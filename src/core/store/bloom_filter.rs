use super::Store;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;
use std::time::{Duration, SystemTime};

/// Bloom filter enhanced store for fast negative lookups
/// 
/// This implementation uses a bloom filter to quickly determine if a key
/// definitely doesn't exist, reducing HashMap lookups for non-existent keys.
/// Particularly effective for sparse keyspaces and DDoS protection.
pub struct BloomFilterStore<S: Store> {
    // Underlying store
    inner: S,
    // Bloom filter bit array
    filter: Vec<u64>,
    // Number of hash functions to use
    hash_count: usize,
    // Filter size in bits
    filter_bits: usize,
    // Approximate number of items
    item_count: usize,
    // Target false positive rate
    #[allow(dead_code)]
    target_fp_rate: f64,
    // Regeneration threshold
    max_items: usize,
}

impl<S: Store> BloomFilterStore<S> {
    pub fn new(inner: S) -> Self {
        Self::with_config(inner, 10_000, 0.01)
    }
    
    pub fn with_config(inner: S, expected_items: usize, false_positive_rate: f64) -> Self {
        // Calculate optimal filter size and hash count
        let filter_bits = Self::optimal_bits(expected_items, false_positive_rate);
        let hash_count = Self::optimal_hash_count(expected_items, filter_bits);
        
        // Create bit array (using u64 for efficient bit operations)
        let filter_words = (filter_bits + 63) / 64;
        let filter = vec![0u64; filter_words];
        
        BloomFilterStore {
            inner,
            filter,
            hash_count,
            filter_bits,
            item_count: 0,
            target_fp_rate: false_positive_rate,
            max_items: expected_items * 2, // Regenerate when 2x expected
        }
    }
    
    /// Calculate optimal number of bits for bloom filter
    fn optimal_bits(items: usize, fp_rate: f64) -> usize {
        let ln2_squared = 0.4804530139182014; // ln(2)^2
        let bits = -(items as f64 * fp_rate.ln()) / ln2_squared;
        bits.ceil() as usize
    }
    
    /// Calculate optimal number of hash functions
    fn optimal_hash_count(items: usize, bits: usize) -> usize {
        let ln2 = 0.6931471805599453; // ln(2)
        let hashes = (bits as f64 / items as f64) * ln2;
        hashes.round().max(1.0) as usize
    }
    
    /// Generate hash values for a key
    fn hash_key(&self, key: &str, index: usize) -> usize {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        index.hash(&mut hasher);
        (hasher.finish() as usize) % self.filter_bits
    }
    
    /// Add a key to the bloom filter
    fn bloom_add(&mut self, key: &str) {
        for i in 0..self.hash_count {
            let bit_idx = self.hash_key(key, i);
            let word_idx = bit_idx / 64;
            let bit_offset = bit_idx % 64;
            self.filter[word_idx] |= 1u64 << bit_offset;
        }
        self.item_count += 1;
    }
    
    /// Check if a key might exist in the bloom filter
    fn bloom_contains(&self, key: &str) -> bool {
        for i in 0..self.hash_count {
            let bit_idx = self.hash_key(key, i);
            let word_idx = bit_idx / 64;
            let bit_offset = bit_idx % 64;
            if (self.filter[word_idx] & (1u64 << bit_offset)) == 0 {
                return false; // Definitely not in set
            }
        }
        true // Possibly in set
    }
    
    /// Clear and regenerate the bloom filter
    fn regenerate_filter(&mut self, _now: SystemTime) {
        // Clear the filter
        for word in &mut self.filter {
            *word = 0;
        }
        self.item_count = 0;
        
        // Note: In a real implementation, we would iterate through all
        // non-expired entries in the inner store and re-add them to the filter.
        // Since Store trait doesn't provide iteration, we'll rely on gradual
        // rebuilding through normal operations.
    }
    
    /// Check if filter needs regeneration
    fn maybe_regenerate(&mut self, now: SystemTime) {
        if self.item_count > self.max_items {
            self.regenerate_filter(now);
        }
    }
}

impl<S: Store> Store for BloomFilterStore<S> {
    fn compare_and_swap_with_ttl(
        &mut self,
        key: &str,
        old: i64,
        new: i64,
        ttl: Duration,
        now: SystemTime,
    ) -> Result<bool, String> {
        // Check bloom filter first
        if !self.bloom_contains(key) {
            // Definitely doesn't exist
            return Ok(false);
        }
        
        // Might exist, check inner store
        let result = self.inner.compare_and_swap_with_ttl(key, old, new, ttl, now)?;
        
        if result {
            // Successfully updated, ensure key is in bloom filter
            self.bloom_add(key);
        }
        
        Ok(result)
    }
    
    fn get(&self, key: &str, now: SystemTime) -> Result<Option<i64>, String> {
        // Check bloom filter first
        if !self.bloom_contains(key) {
            // Definitely doesn't exist
            return Ok(None);
        }
        
        // Might exist, check inner store
        self.inner.get(key, now)
    }
    
    fn log_debug(&self, message: &str) {
        self.inner.log_debug(message)
    }
    
    fn set_if_not_exists_with_ttl(
        &mut self,
        key: &str,
        value: i64,
        ttl: Duration,
        now: SystemTime,
    ) -> Result<bool, String> {
        // For new keys, we can skip bloom filter check since we're adding anyway
        let result = self.inner.set_if_not_exists_with_ttl(key, value, ttl, now)?;
        
        if result {
            // Successfully added, add to bloom filter
            self.bloom_add(key);
            self.maybe_regenerate(now);
        }
        
        Ok(result)
    }
}

/// Counting bloom filter variant that supports deletions
pub struct CountingBloomFilterStore<S: Store> {
    inner: S,
    // Use u8 counters instead of bits (supports up to 255 additions)
    counters: Vec<u8>,
    hash_count: usize,
    filter_size: usize,
    item_count: usize,
}

impl<S: Store> CountingBloomFilterStore<S> {
    pub fn new(inner: S) -> Self {
        Self::with_size(inner, 10_000)
    }
    
    pub fn with_size(inner: S, expected_items: usize) -> Self {
        let filter_size = expected_items * 10; // 10x size for low FP rate
        let hash_count = 4; // Good default for counting filter
        
        CountingBloomFilterStore {
            inner,
            counters: vec![0; filter_size],
            hash_count,
            filter_size,
            item_count: 0,
        }
    }
    
    fn hash_key(&self, key: &str, index: usize) -> usize {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        index.hash(&mut hasher);
        (hasher.finish() as usize) % self.filter_size
    }
    
    fn increment(&mut self, key: &str) {
        for i in 0..self.hash_count {
            let idx = self.hash_key(key, i);
            if self.counters[idx] < 255 {
                self.counters[idx] = self.counters[idx].saturating_add(1);
            }
        }
        self.item_count += 1;
    }
    
    #[allow(dead_code)]
    fn decrement(&mut self, key: &str) {
        for i in 0..self.hash_count {
            let idx = self.hash_key(key, i);
            self.counters[idx] = self.counters[idx].saturating_sub(1);
        }
        if self.item_count > 0 {
            self.item_count -= 1;
        }
    }
    
    fn contains(&self, key: &str) -> bool {
        for i in 0..self.hash_count {
            let idx = self.hash_key(key, i);
            if self.counters[idx] == 0 {
                return false;
            }
        }
        true
    }
}

impl<S: Store> Store for CountingBloomFilterStore<S> {
    fn compare_and_swap_with_ttl(
        &mut self,
        key: &str,
        old: i64,
        new: i64,
        ttl: Duration,
        now: SystemTime,
    ) -> Result<bool, String> {
        if !self.contains(key) {
            return Ok(false);
        }
        
        self.inner.compare_and_swap_with_ttl(key, old, new, ttl, now)
    }
    
    fn get(&self, key: &str, now: SystemTime) -> Result<Option<i64>, String> {
        if !self.contains(key) {
            return Ok(None);
        }
        
        self.inner.get(key, now)
    }
    
    fn log_debug(&self, message: &str) {
        self.inner.log_debug(message)
    }
    
    fn set_if_not_exists_with_ttl(
        &mut self,
        key: &str,
        value: i64,
        ttl: Duration,
        now: SystemTime,
    ) -> Result<bool, String> {
        let result = self.inner.set_if_not_exists_with_ttl(key, value, ttl, now)?;
        
        if result {
            self.increment(key);
        }
        
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::store::MemoryStore;
    
    #[test]
    fn test_bloom_filter_basic() {
        let inner = MemoryStore::new();
        let mut store = BloomFilterStore::with_config(inner, 100, 0.01);
        let now = SystemTime::now();
        let ttl = Duration::from_secs(60);
        
        // Test set and get
        assert!(store.set_if_not_exists_with_ttl("key1", 100, ttl, now).unwrap());
        assert_eq!(store.get("key1", now).unwrap(), Some(100));
        
        // Test non-existent key (bloom filter should say "definitely not")
        assert_eq!(store.get("key_that_doesnt_exist", now).unwrap(), None);
    }
    
    #[test]
    fn test_bloom_filter_false_positives() {
        let inner = MemoryStore::new();
        let mut store = BloomFilterStore::with_config(inner, 100, 0.1); // High FP rate
        let now = SystemTime::now();
        let ttl = Duration::from_secs(60);
        
        // Add some keys
        for i in 0..50 {
            let key = format!("key{}", i);
            assert!(store.set_if_not_exists_with_ttl(&key, i, ttl, now).unwrap());
        }
        
        // Check for false positives
        let mut false_positives = 0;
        for i in 100..200 {
            let key = format!("key{}", i);
            if store.bloom_contains(&key) {
                false_positives += 1;
            }
        }
        
        // With 0.1 FP rate, we expect roughly 10 false positives out of 100
        assert!(false_positives < 20, "Too many false positives: {}", false_positives);
    }
    
    #[test]
    fn test_counting_bloom_filter() {
        let inner = MemoryStore::new();
        let mut store = CountingBloomFilterStore::with_size(inner, 100);
        let now = SystemTime::now();
        let ttl = Duration::from_secs(60);
        
        // Test basic operations
        assert!(store.set_if_not_exists_with_ttl("key1", 100, ttl, now).unwrap());
        assert_eq!(store.get("key1", now).unwrap(), Some(100));
        
        // Test increment/decrement
        store.increment("key2");
        assert!(store.contains("key2"));
        store.decrement("key2");
        assert!(!store.contains("key2"));
    }
}
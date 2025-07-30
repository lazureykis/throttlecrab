use super::Store;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;
use std::time::{Duration, SystemTime};

/// Store using optimized HashMap patterns
/// 
/// Benefits:
/// - Pre-computed hashes stored with entries
/// - Optimized entry API usage
/// - Reduced allocations through careful API use
pub struct RawApiStore {
    data: HashMap<String, (i64, SystemTime)>,
    cleanup_counter: usize,
    cleanup_interval: usize,
}

impl RawApiStore {
    pub fn new() -> Self {
        Self::with_capacity(1000)
    }
    
    pub fn with_capacity(capacity: usize) -> Self {
        RawApiStore {
            data: HashMap::with_capacity(capacity),
            cleanup_counter: 0,
            cleanup_interval: 100,
        }
    }
    
    /// Compute hash for a key
    #[inline]
    #[allow(dead_code)]
    fn hash_key(key: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish()
    }
    
    fn maybe_cleanup(&mut self, now: SystemTime) {
        self.cleanup_counter += 1;
        if self.cleanup_counter >= self.cleanup_interval {
            self.cleanup_counter = 0;
            self.cleanup(now);
        }
    }
    
    fn cleanup(&mut self, now: SystemTime) {
        self.data.retain(|_, (_, expiry)| *expiry > now);
    }
}

impl Store for RawApiStore {
    fn compare_and_swap_with_ttl(
        &mut self,
        key: &str,
        old: i64,
        new: i64,
        ttl: Duration,
        now: SystemTime,
    ) -> Result<bool, String> {
        self.maybe_cleanup(now);
        
        // Use entry API to minimize allocations
        if let Some((value, expiry)) = self.data.get_mut(key) {
            if *expiry > now && *value == old {
                *value = new;
                *expiry = now + ttl;
                Ok(true)
            } else {
                Ok(false)
            }
        } else {
            Ok(false)
        }
    }
    
    fn get(&self, key: &str, now: SystemTime) -> Result<Option<i64>, String> {
        // Note: raw_entry API for immutable access is still unstable
        // Using regular get for now
        match self.data.get(key) {
            Some((value, expiry)) if *expiry > now => Ok(Some(*value)),
            _ => Ok(None),
        }
    }
    
    fn log_debug(&self, message: &str) {
        eprintln!("RawApiStore: {}", message);
    }
    
    fn set_if_not_exists_with_ttl(
        &mut self,
        key: &str,
        value: i64,
        ttl: Duration,
        now: SystemTime,
    ) -> Result<bool, String> {
        self.maybe_cleanup(now);
        
        match self.data.entry(key.to_string()) {
            Entry::Occupied(mut entry) => {
                let (_, expiry) = entry.get();
                if *expiry <= now {
                    // Entry is expired, replace it
                    entry.insert((value, now + ttl));
                    Ok(true)
                } else {
                    // Entry exists and is not expired
                    Ok(false)
                }
            }
            Entry::Vacant(entry) => {
                entry.insert((value, now + ttl));
                Ok(true)
            }
        }
    }
}

/// Store with pre-computed hashes to avoid recomputation
pub struct RawApiStoreV2 {
    // Store hash alongside data to avoid recomputing
    data: HashMap<String, (i64, SystemTime, u64)>,
    // Secondary index for fast hash lookups (if we had many collisions)
    cleanup_counter: usize,
    cleanup_interval: usize,
}

impl RawApiStoreV2 {
    pub fn new() -> Self {
        Self::with_capacity(1000)
    }
    
    pub fn with_capacity(capacity: usize) -> Self {
        RawApiStoreV2 {
            data: HashMap::with_capacity(capacity),
            cleanup_counter: 0,
            cleanup_interval: 100,
        }
    }
    
    #[inline]
    fn hash_key(key: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish()
    }
    
    fn maybe_cleanup(&mut self, now: SystemTime) {
        self.cleanup_counter += 1;
        if self.cleanup_counter >= self.cleanup_interval {
            self.cleanup_counter = 0;
            self.data.retain(|_, (_, expiry, _)| *expiry > now);
        }
    }
}

impl Store for RawApiStoreV2 {
    fn compare_and_swap_with_ttl(
        &mut self,
        key: &str,
        old: i64,
        new: i64,
        ttl: Duration,
        now: SystemTime,
    ) -> Result<bool, String> {
        self.maybe_cleanup(now);
        
        let hash = Self::hash_key(key);
        
        if let Some((value, expiry, stored_hash)) = self.data.get_mut(key) {
            if *expiry > now && *value == old {
                *value = new;
                *expiry = now + ttl;
                *stored_hash = hash;
                Ok(true)
            } else {
                Ok(false)
            }
        } else {
            Ok(false)
        }
    }
    
    fn get(&self, key: &str, now: SystemTime) -> Result<Option<i64>, String> {
        match self.data.get(key) {
            Some((value, expiry, _)) if *expiry > now => Ok(Some(*value)),
            _ => Ok(None),
        }
    }
    
    fn log_debug(&self, message: &str) {
        eprintln!("RawApiStoreV2: {}", message);
    }
    
    fn set_if_not_exists_with_ttl(
        &mut self,
        key: &str,
        value: i64,
        ttl: Duration,
        now: SystemTime,
    ) -> Result<bool, String> {
        self.maybe_cleanup(now);
        
        let hash = Self::hash_key(key);
        
        match self.data.entry(key.to_string()) {
            Entry::Occupied(mut entry) => {
                let (_, expiry, _) = entry.get();
                if *expiry <= now {
                    // Entry is expired, replace it
                    entry.insert((value, now + ttl, hash));
                    Ok(true)
                } else {
                    // Entry exists and is not expired
                    Ok(false)
                }
            }
            Entry::Vacant(entry) => {
                entry.insert((value, now + ttl, hash));
                Ok(true)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_raw_api_store_basic() {
        let mut store = RawApiStore::new();
        let now = SystemTime::now();
        let ttl = Duration::from_secs(60);
        
        // Test set and get
        assert!(store.set_if_not_exists_with_ttl("key1", 100, ttl, now).unwrap());
        assert_eq!(store.get("key1", now).unwrap(), Some(100));
        
        // Test compare and swap
        assert!(store.compare_and_swap_with_ttl("key1", 100, 200, ttl, now).unwrap());
        assert_eq!(store.get("key1", now).unwrap(), Some(200));
    }
    
    #[test]
    fn test_raw_api_v2_store() {
        let mut store = RawApiStoreV2::new();
        let now = SystemTime::now();
        let ttl = Duration::from_secs(60);
        
        // Test basic operations
        assert!(store.set_if_not_exists_with_ttl("key1", 100, ttl, now).unwrap());
        assert_eq!(store.get("key1", now).unwrap(), Some(100));
        
        // Verify hash is stored
        let (_, _, hash) = store.data.get("key1").unwrap();
        assert_eq!(*hash, RawApiStoreV2::hash_key("key1"));
    }
}
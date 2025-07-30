use super::Store;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

#[cfg(feature = "ahash")]
use ahash::AHashMap as HashMap;
#[cfg(not(feature = "ahash"))]
use std::collections::HashMap;

/// Sharded store for improved concurrency
/// 
/// This implementation partitions keys across multiple internal stores,
/// each with its own lock, reducing contention in multi-threaded scenarios.
pub struct ShardedMemoryStore {
    shards: Vec<Shard>,
    shard_count: usize,
}

struct Shard {
    data: Arc<Mutex<HashMap<String, (i64, Option<SystemTime>)>>>,
    next_cleanup: Arc<Mutex<SystemTime>>,
    cleanup_interval: Duration,
}

impl ShardedMemoryStore {
    pub fn new() -> Self {
        // Default to number of CPU cores
        let cpu_count = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        Self::with_shard_count(cpu_count * 4) // 4x CPU cores for better distribution
    }
    
    pub fn with_shard_count(shard_count: usize) -> Self {
        assert!(shard_count > 0, "Shard count must be greater than 0");
        
        let shards = (0..shard_count)
            .map(|_| Shard {
                data: Arc::new(Mutex::new(HashMap::new())),
                next_cleanup: Arc::new(Mutex::new(SystemTime::now() + Duration::from_secs(60))),
                cleanup_interval: Duration::from_secs(60),
            })
            .collect();
            
        ShardedMemoryStore {
            shards,
            shard_count,
        }
    }
    
    /// Determine which shard a key belongs to
    fn get_shard_index(&self, key: &str) -> usize {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        (hasher.finish() as usize) % self.shard_count
    }
    
    /// Get the shard for a specific key
    fn get_shard(&self, key: &str) -> &Shard {
        let index = self.get_shard_index(key);
        &self.shards[index]
    }
}

impl Shard {
    fn clean_expired(&self, now: SystemTime) {
        let mut data = self.data.lock().unwrap();
        data.retain(|_, (_, expiry)| {
            if let Some(exp) = expiry {
                *exp > now
            } else {
                true
            }
        });
        
        let mut next_cleanup = self.next_cleanup.lock().unwrap();
        *next_cleanup = now + self.cleanup_interval;
    }
    
    fn maybe_clean_expired(&self, now: SystemTime) {
        let next_cleanup = *self.next_cleanup.lock().unwrap();
        if now >= next_cleanup {
            self.clean_expired(now);
        }
    }
}

impl Default for ShardedMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl Store for ShardedMemoryStore {
    fn compare_and_swap_with_ttl(
        &mut self,
        key: &str,
        old: i64,
        new: i64,
        ttl: Duration,
        now: SystemTime,
    ) -> Result<bool, String> {
        let shard = self.get_shard(key);
        shard.maybe_clean_expired(now);
        
        let mut data = shard.data.lock().unwrap();
        match data.get(key) {
            Some((current, _)) if *current == old => {
                let expiry = now + ttl;
                data.insert(key.to_string(), (new, Some(expiry)));
                Ok(true)
            }
            Some(_) => Ok(false),
            None => Ok(false),
        }
    }
    
    fn get(&self, key: &str, now: SystemTime) -> Result<Option<i64>, String> {
        let shard = self.get_shard(key);
        let data = shard.data.lock().unwrap();
        
        match data.get(key) {
            Some((value, Some(expiry))) if *expiry > now => Ok(Some(*value)),
            Some((value, None)) => Ok(Some(*value)),
            _ => Ok(None),
        }
    }
    
    fn log_debug(&self, _message: &str) {
        // No-op
    }
    
    fn set_if_not_exists_with_ttl(
        &mut self,
        key: &str,
        value: i64,
        ttl: Duration,
        now: SystemTime,
    ) -> Result<bool, String> {
        let shard = self.get_shard(key);
        
        // Check if key exists and is not expired
        {
            let data = shard.data.lock().unwrap();
            match data.get(key) {
                Some((_, Some(expiry))) if *expiry > now => return Ok(false),
                Some((_, None)) => return Ok(false),
                _ => {}
            }
        }
        
        // Clean and insert
        shard.maybe_clean_expired(now);
        
        let mut data = shard.data.lock().unwrap();
        let expiry = now + ttl;
        data.insert(key.to_string(), (value, Some(expiry)));
        Ok(true)
    }
}

/// Thread-safe wrapper for concurrent access
/// 
/// This wrapper allows the ShardedMemoryStore to be used safely
/// across multiple threads without external synchronization.
pub struct ConcurrentShardedStore {
    inner: Arc<Mutex<ShardedMemoryStore>>,
}

impl ConcurrentShardedStore {
    pub fn new() -> Self {
        ConcurrentShardedStore {
            inner: Arc::new(Mutex::new(ShardedMemoryStore::new())),
        }
    }
    
    pub fn with_shard_count(shard_count: usize) -> Self {
        ConcurrentShardedStore {
            inner: Arc::new(Mutex::new(ShardedMemoryStore::with_shard_count(shard_count))),
        }
    }
}

impl Clone for ConcurrentShardedStore {
    fn clone(&self) -> Self {
        ConcurrentShardedStore {
            inner: self.inner.clone(),
        }
    }
}

impl Default for ConcurrentShardedStore {
    fn default() -> Self {
        Self::new()
    }
}

impl Store for ConcurrentShardedStore {
    fn compare_and_swap_with_ttl(
        &mut self,
        key: &str,
        old: i64,
        new: i64,
        ttl: Duration,
        now: SystemTime,
    ) -> Result<bool, String> {
        self.inner.lock().unwrap().compare_and_swap_with_ttl(key, old, new, ttl, now)
    }
    
    fn get(&self, key: &str, now: SystemTime) -> Result<Option<i64>, String> {
        self.inner.lock().unwrap().get(key, now)
    }
    
    fn log_debug(&self, message: &str) {
        self.inner.lock().unwrap().log_debug(message)
    }
    
    fn set_if_not_exists_with_ttl(
        &mut self,
        key: &str,
        value: i64,
        ttl: Duration,
        now: SystemTime,
    ) -> Result<bool, String> {
        self.inner.lock().unwrap().set_if_not_exists_with_ttl(key, value, ttl, now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sharded_basic_operations() {
        let mut store = ShardedMemoryStore::with_shard_count(4);
        let now = SystemTime::now();
        let ttl = Duration::from_secs(60);
        
        // Test set and get
        assert!(store.set_if_not_exists_with_ttl("key1", 100, ttl, now).unwrap());
        assert_eq!(store.get("key1", now).unwrap(), Some(100));
        
        // Test compare and swap
        assert!(store.compare_and_swap_with_ttl("key1", 100, 200, ttl, now).unwrap());
        assert_eq!(store.get("key1", now).unwrap(), Some(200));
        
        // Test non-existent key
        assert_eq!(store.get("key2", now).unwrap(), None);
    }
    
    #[test]
    fn test_sharded_distribution() {
        let store = ShardedMemoryStore::with_shard_count(4);
        
        // Test that different keys map to different shards
        let keys = vec!["key1", "key2", "key3", "key4", "key5", "key6", "key7", "key8"];
        let mut shard_counts = vec![0; 4];
        
        for key in &keys {
            let index = store.get_shard_index(key);
            shard_counts[index] += 1;
        }
        
        // Verify keys are distributed (at least 2 shards should be used)
        let used_shards = shard_counts.iter().filter(|&&count| count > 0).count();
        assert!(used_shards >= 2);
    }
    
    #[test]
    fn test_concurrent_access() {
        let store = ConcurrentShardedStore::with_shard_count(4);
        let now = SystemTime::now();
        let ttl = Duration::from_secs(60);
        
        // Clone for concurrent access
        let mut store1 = store.clone();
        let mut store2 = store.clone();
        
        // Both can access
        assert!(store1.set_if_not_exists_with_ttl("key1", 100, ttl, now).unwrap());
        assert_eq!(store2.get("key1", now).unwrap(), Some(100));
    }
    
    #[test]
    fn test_expiry_handling() {
        let mut store = ShardedMemoryStore::with_shard_count(2);
        let now = SystemTime::now();
        let short_ttl = Duration::from_millis(100);
        
        // Set with short TTL
        assert!(store.set_if_not_exists_with_ttl("key1", 100, short_ttl, now).unwrap());
        
        // Should exist now
        assert_eq!(store.get("key1", now).unwrap(), Some(100));
        
        // Should be expired later
        let later = now + Duration::from_millis(200);
        assert_eq!(store.get("key1", later).unwrap(), None);
        
        // Should be able to set again
        assert!(store.set_if_not_exists_with_ttl("key1", 200, Duration::from_secs(60), later).unwrap());
        assert_eq!(store.get("key1", later).unwrap(), Some(200));
    }
}
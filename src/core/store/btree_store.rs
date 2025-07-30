use super::Store;
use std::collections::BTreeMap;
use std::time::{Duration, SystemTime};

/// BTreeMap-based store for ordered key access
/// 
/// Benefits:
/// - Keys are always sorted, enabling efficient range queries
/// - Better cache locality for sequential access patterns
/// - Predictable O(log n) performance
/// - No hash collisions
pub struct BTreeStore {
    data: BTreeMap<String, (i64, SystemTime)>,
    cleanup_counter: usize,
    cleanup_interval: usize,
}

impl BTreeStore {
    pub fn new() -> Self {
        Self::with_capacity(1000)
    }
    
    pub fn with_capacity(_capacity: usize) -> Self {
        // BTreeMap doesn't support pre-allocation
        BTreeStore {
            data: BTreeMap::new(),
            cleanup_counter: 0,
            cleanup_interval: 100,
        }
    }
    
    fn maybe_cleanup(&mut self, now: SystemTime) {
        self.cleanup_counter += 1;
        if self.cleanup_counter >= self.cleanup_interval {
            self.cleanup_counter = 0;
            self.cleanup(now);
        }
    }
    
    fn cleanup(&mut self, now: SystemTime) {
        // BTreeMap maintains order, so we can efficiently remove expired entries
        let expired_keys: Vec<String> = self.data
            .iter()
            .filter(|(_, (_, expiry))| *expiry <= now)
            .map(|(k, _)| k.clone())
            .collect();
            
        for key in expired_keys {
            self.data.remove(&key);
        }
    }
}

impl Store for BTreeStore {
    fn compare_and_swap_with_ttl(
        &mut self,
        key: &str,
        old: i64,
        new: i64,
        ttl: Duration,
        now: SystemTime,
    ) -> Result<bool, String> {
        self.maybe_cleanup(now);
        
        match self.data.get(key) {
            Some((value, expiry)) if *expiry > now => {
                if *value == old {
                    let new_expiry = now + ttl;
                    self.data.insert(key.to_string(), (new, new_expiry));
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            _ => Ok(false),
        }
    }
    
    fn get(&self, key: &str, now: SystemTime) -> Result<Option<i64>, String> {
        match self.data.get(key) {
            Some((value, expiry)) if *expiry > now => Ok(Some(*value)),
            _ => Ok(None),
        }
    }
    
    fn log_debug(&self, message: &str) {
        eprintln!("BTreeStore: {}", message);
    }
    
    fn set_if_not_exists_with_ttl(
        &mut self,
        key: &str,
        value: i64,
        ttl: Duration,
        now: SystemTime,
    ) -> Result<bool, String> {
        self.maybe_cleanup(now);
        
        match self.data.get(key) {
            Some((_, expiry)) if *expiry > now => Ok(false),
            _ => {
                let expiry = now + ttl;
                self.data.insert(key.to_string(), (value, expiry));
                Ok(true)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_btree_store_basic() {
        let mut store = BTreeStore::new();
        let now = SystemTime::now();
        let ttl = Duration::from_secs(60);
        
        // Test set and get
        assert!(store.set_if_not_exists_with_ttl("key1", 100, ttl, now).unwrap());
        assert_eq!(store.get("key1", now).unwrap(), Some(100));
        
        // Test compare and swap
        assert!(store.compare_and_swap_with_ttl("key1", 100, 200, ttl, now).unwrap());
        assert_eq!(store.get("key1", now).unwrap(), Some(200));
        
        // Test expiry
        let expired_time = now + Duration::from_secs(61);
        assert_eq!(store.get("key1", expired_time).unwrap(), None);
    }
    
    #[test]
    fn test_btree_ordered_iteration() {
        let mut store = BTreeStore::new();
        let now = SystemTime::now();
        let ttl = Duration::from_secs(60);
        
        // Insert keys in random order
        store.set_if_not_exists_with_ttl("key3", 3, ttl, now).unwrap();
        store.set_if_not_exists_with_ttl("key1", 1, ttl, now).unwrap();
        store.set_if_not_exists_with_ttl("key2", 2, ttl, now).unwrap();
        
        // Keys should be in sorted order in BTreeMap
        let keys: Vec<String> = store.data.keys().cloned().collect();
        assert_eq!(keys, vec!["key1", "key2", "key3"]);
    }
}
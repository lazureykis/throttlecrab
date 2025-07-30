use super::Store;
use std::collections::{BinaryHeap, HashMap};
use std::cmp::Ordering;
use std::time::{Duration, SystemTime};

/// Entry in the expiry heap
#[derive(Debug, Clone, Eq, PartialEq)]
struct ExpiryEntry {
    expiry: SystemTime,
    key: String,
}

// Implement ordering for min-heap (earliest expiry first)
impl Ord for ExpiryEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse order for min-heap
        other.expiry.cmp(&self.expiry)
    }
}

impl PartialOrd for ExpiryEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// BinaryHeap-based store for efficient TTL management
/// 
/// Benefits:
/// - O(1) access to next expiring key
/// - O(log n) insertion and removal
/// - Efficient bulk expiry operations
/// - Perfect for time-based cleanup strategies
pub struct HeapStore {
    data: HashMap<String, (i64, SystemTime)>,
    expiry_heap: BinaryHeap<ExpiryEntry>,
    cleanup_batch_size: usize,
}

impl HeapStore {
    pub fn new() -> Self {
        Self::with_capacity(1000)
    }
    
    pub fn with_capacity(capacity: usize) -> Self {
        HeapStore {
            data: HashMap::with_capacity(capacity),
            expiry_heap: BinaryHeap::with_capacity(capacity),
            cleanup_batch_size: 100,
        }
    }
    
    /// Clean up expired entries using the heap
    fn cleanup_expired(&mut self, now: SystemTime) {
        let mut cleaned = 0;
        
        while cleaned < self.cleanup_batch_size {
            match self.expiry_heap.peek() {
                Some(entry) if entry.expiry <= now => {
                    let entry = self.expiry_heap.pop().unwrap();
                    
                    // Check if the key still exists and has the same expiry
                    if let Some((_, stored_expiry)) = self.data.get(&entry.key) {
                        if *stored_expiry == entry.expiry {
                            self.data.remove(&entry.key);
                            cleaned += 1;
                        }
                        // If expiry doesn't match, this is a stale heap entry
                    }
                }
                _ => break, // No more expired entries
            }
        }
    }
}

impl Store for HeapStore {
    fn compare_and_swap_with_ttl(
        &mut self,
        key: &str,
        old: i64,
        new: i64,
        ttl: Duration,
        now: SystemTime,
    ) -> Result<bool, String> {
        self.cleanup_expired(now);
        
        match self.data.get(key) {
            Some((value, expiry)) if *expiry > now => {
                if *value == old {
                    let new_expiry = now + ttl;
                    self.data.insert(key.to_string(), (new, new_expiry));
                    
                    // Add new expiry to heap
                    self.expiry_heap.push(ExpiryEntry {
                        expiry: new_expiry,
                        key: key.to_string(),
                    });
                    
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
        eprintln!("HeapStore: {}", message);
    }
    
    fn set_if_not_exists_with_ttl(
        &mut self,
        key: &str,
        value: i64,
        ttl: Duration,
        now: SystemTime,
    ) -> Result<bool, String> {
        self.cleanup_expired(now);
        
        match self.data.get(key) {
            Some((_, expiry)) if *expiry > now => Ok(false),
            _ => {
                let expiry = now + ttl;
                self.data.insert(key.to_string(), (value, expiry));
                
                // Add to expiry heap
                self.expiry_heap.push(ExpiryEntry {
                    expiry,
                    key: key.to_string(),
                });
                
                Ok(true)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_heap_store_basic() {
        let mut store = HeapStore::new();
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
    fn test_heap_expiry_order() {
        let mut store = HeapStore::new();
        let now = SystemTime::now();
        
        // Insert with different TTLs
        store.set_if_not_exists_with_ttl("key1", 1, Duration::from_secs(10), now).unwrap();
        store.set_if_not_exists_with_ttl("key2", 2, Duration::from_secs(5), now).unwrap();
        store.set_if_not_exists_with_ttl("key3", 3, Duration::from_secs(15), now).unwrap();
        
        // Verify heap has correct order (key2 should expire first)
        let first = store.expiry_heap.peek().unwrap();
        assert_eq!(first.key, "key2");
    }
    
    #[test]
    fn test_heap_cleanup() {
        let mut store = HeapStore::new();
        let now = SystemTime::now();
        
        // Insert entries with short TTL
        for i in 0..10 {
            store.set_if_not_exists_with_ttl(
                &format!("key{}", i),
                i,
                Duration::from_secs(1),
                now
            ).unwrap();
        }
        
        // All should be present
        assert_eq!(store.data.len(), 10);
        
        // After expiry, cleanup should remove them
        let expired_time = now + Duration::from_secs(2);
        store.cleanup_expired(expired_time);
        
        // All should be removed
        assert_eq!(store.data.len(), 0);
    }
}
#[cfg(test)]
mod tests {
    use super::super::optimized::OptimizedMemoryStore;
    use super::super::Store;
    use std::time::{Duration, SystemTime};

    #[test]
    fn test_cleanup_actually_happens() {
        let mut store = OptimizedMemoryStore::with_capacity(100);
        let now = SystemTime::now();
        
        // Add 1000 entries with 1 second TTL
        for i in 0..1000 {
            let key = format!("key_{}", i);
            store.set_if_not_exists_with_ttl(
                &key,
                i,
                Duration::from_secs(1),
                now
            ).unwrap();
        }
        
        // Verify all entries exist
        assert_eq!(store.len(), 1000);
        
        // Move time forward by 2 seconds (past TTL)
        let future = now + Duration::from_secs(2);
        
        // Access some keys to trigger cleanup check
        for i in 0..10 {
            let key = format!("key_{}", i);
            let _ = store.get(&key, future);
        }
        
        // The first get won't trigger cleanup (no operations yet)
        // But after some operations...
        
        // Do many operations to ensure we hit a cleanup trigger
        for i in 0..200 {
            let key = format!("new_key_{}", i);
            store.set_if_not_exists_with_ttl(
                &key,
                i,
                Duration::from_secs(60),
                future
            ).unwrap();
        }
        
        // Move time forward past cleanup interval
        let much_later = future + Duration::from_secs(61);
        
        // This should trigger cleanup
        store.get("trigger_cleanup", much_later).unwrap();
        
        // Verify expired entries were removed
        assert!(store.len() < 1000, "Cleanup didn't remove expired entries. Size: {}", store.len());
        
        // Verify new entries still exist
        for i in 0..200 {
            let key = format!("new_key_{}", i);
            assert!(store.get(&key, much_later).unwrap().is_some());
        }
    }
    
    #[test]
    fn test_cleanup_with_memory_pressure() {
        let mut store = OptimizedMemoryStore::with_capacity(100);
        let now = SystemTime::now();
        
        // Fill store with mixed TTL entries
        for i in 0..500 {
            let key = format!("key_{}", i);
            let ttl = if i % 2 == 0 {
                Duration::from_secs(1) // Half expire quickly
            } else {
                Duration::from_secs(3600) // Half have long TTL
            };
            
            store.set_if_not_exists_with_ttl(&key, i, ttl, now).unwrap();
        }
        
        // Move time forward
        let later = now + Duration::from_secs(2);
        
        // Mark many as expired by checking them
        for i in (0..500).step_by(2) {
            let key = format!("key_{}", i);
            let _ = store.get(&key, later);
        }
        
        // Store should have tracked ~250 expired entries
        assert!(store.expired_count() > 200);
        
        // Trigger cleanup by doing more operations
        let much_later = later + Duration::from_secs(58);
        
        // This should trigger cleanup due to expired count
        store.set_if_not_exists_with_ttl("trigger", 999, Duration::from_secs(60), much_later).unwrap();
        
        // Verify cleanup happened
        assert!(store.len() < 300, "Expected ~250 entries after cleanup, got {}", store.len());
    }
    
    #[test] 
    fn test_no_cleanup_without_triggers() {
        let mut store = OptimizedMemoryStore::with_capacity(100);
        let now = SystemTime::now();
        
        // Add entries with long TTL
        for i in 0..100 {
            let key = format!("key_{}", i);
            store.set_if_not_exists_with_ttl(
                &key,
                i,
                Duration::from_secs(3600),
                now
            ).unwrap();
        }
        
        // Do operations but not enough to trigger cleanup
        for i in 0..10 {
            let key = format!("key_{}", i);
            let _ = store.get(&key, now);
        }
        
        // Verify no cleanup happened (all entries still valid)
        assert_eq!(store.len(), 100);
        assert_eq!(store.expired_count(), 0);
    }
}
#[cfg(test)]
mod tests {
    use super::super::store::{MemoryStore, Store};
    use std::time::{Duration, SystemTime};

    #[test]
    fn test_memory_store_set_and_get() {
        let mut store = MemoryStore::new();
        
        // Set a value
        let success = store
            .set_if_not_exists_with_ttl("key1", 42, Duration::from_secs(60))
            .unwrap();
        assert!(success);

        // Get the value
        let (value, _time) = store.get_with_time("key1").unwrap();
        assert_eq!(value, Some(42));

        // Try to set again - should fail
        let success = store
            .set_if_not_exists_with_ttl("key1", 100, Duration::from_secs(60))
            .unwrap();
        assert!(!success);

        // Value should still be 42
        let (value, _time) = store.get_with_time("key1").unwrap();
        assert_eq!(value, Some(42));
    }

    #[test]
    fn test_memory_store_compare_and_swap() {
        let mut store = MemoryStore::new();

        // Set initial value
        store
            .set_if_not_exists_with_ttl("key1", 10, Duration::from_secs(60))
            .unwrap();

        // Successful CAS
        let success = store
            .compare_and_swap_with_ttl("key1", 10, 20, Duration::from_secs(60))
            .unwrap();
        assert!(success);

        let (value, _) = store.get_with_time("key1").unwrap();
        assert_eq!(value, Some(20));

        // Failed CAS - old value doesn't match
        let success = store
            .compare_and_swap_with_ttl("key1", 10, 30, Duration::from_secs(60))
            .unwrap();
        assert!(!success);

        let (value, _) = store.get_with_time("key1").unwrap();
        assert_eq!(value, Some(20)); // Still 20
    }

    #[test]
    fn test_memory_store_ttl() {
        let mut store = MemoryStore::new();

        // Set with very short TTL
        store
            .set_if_not_exists_with_ttl("key1", 42, Duration::from_millis(1))
            .unwrap();

        // Value should exist immediately
        let (value, _) = store.get_with_time("key1").unwrap();
        assert_eq!(value, Some(42));

        // Wait for expiry
        std::thread::sleep(Duration::from_millis(10));

        // Trigger cleanup by trying to set a new value
        store
            .set_if_not_exists_with_ttl("key2", 100, Duration::from_secs(60))
            .unwrap();

        // Original key should be gone after cleanup
        let (value, _) = store.get_with_time("key1").unwrap();
        assert_eq!(value, None);
    }

    #[test]
    fn test_memory_store_get_nonexistent() {
        let store = MemoryStore::new();

        let (value, time) = store.get_with_time("nonexistent").unwrap();
        assert_eq!(value, None);
        
        // Time should be close to now
        let now = SystemTime::now();
        let diff = now.duration_since(time).unwrap_or_else(|_| time.duration_since(now).unwrap());
        assert!(diff < Duration::from_millis(100));
    }

    #[test]
    fn test_memory_store_multiple_keys() {
        let mut store = MemoryStore::new();

        // Set multiple keys
        for i in 0..10 {
            let key = format!("key{}", i);
            store
                .set_if_not_exists_with_ttl(&key, i * 10, Duration::from_secs(60))
                .unwrap();
        }

        // Verify all keys
        for i in 0..10 {
            let key = format!("key{}", i);
            let (value, _) = store.get_with_time(&key).unwrap();
            assert_eq!(value, Some(i * 10));
        }
    }
}
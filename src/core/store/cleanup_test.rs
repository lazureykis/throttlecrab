#[cfg(test)]
mod tests {
    use super::super::Store;
    use super::super::optimized::OptimizedMemoryStore;
    use std::time::{Duration, SystemTime};

    #[test]
    fn test_cleanup_actually_happens() {
        let mut store = OptimizedMemoryStore::with_capacity(100);
        let now = SystemTime::now();

        // Add 1000 entries with 1 second TTL
        for i in 0..1000 {
            let key = format!("key_{i}");
            store
                .set_if_not_exists_with_ttl(&key, i, Duration::from_secs(1), now)
                .unwrap();
        }

        // Verify all entries exist
        assert_eq!(store.len(), 1000);

        // Move time forward by 61 seconds (past TTL and cleanup interval)
        let future = now + Duration::from_secs(61);

        // Trigger cleanup by performing an operation after the cleanup interval
        store
            .set_if_not_exists_with_ttl("trigger", 999, Duration::from_secs(60), future)
            .unwrap();

        // Verify expired entries were removed
        // Should only have the trigger entry
        assert!(
            store.len() < 50,
            "Cleanup didn't remove expired entries. Size: {}",
            store.len()
        );

        // Verify the trigger entry exists
        assert!(store.get("trigger", future).unwrap().is_some());
    }

    #[test]
    fn test_cleanup_with_memory_pressure() {
        let mut store = OptimizedMemoryStore::with_capacity(100);
        let now = SystemTime::now();

        // Fill store with mixed TTL entries
        for i in 0..500 {
            let key = format!("key_{i}");
            let ttl = if i % 2 == 0 {
                Duration::from_secs(1) // Half expire quickly
            } else {
                Duration::from_secs(3600) // Half have long TTL
            };

            store.set_if_not_exists_with_ttl(&key, i, ttl, now).unwrap();
        }

        // Move time forward past cleanup interval
        let later = now + Duration::from_secs(61);

        // Trigger cleanup by performing an operation after the cleanup interval
        store
            .set_if_not_exists_with_ttl("trigger", 999, Duration::from_secs(60), later)
            .unwrap();

        // Verify cleanup happened - should have ~250 long-TTL entries + trigger
        assert!(
            store.len() < 300 && store.len() > 200,
            "Expected ~251 entries after cleanup, got {}",
            store.len()
        );

        // Verify that long-TTL entries still exist
        for i in (1..100).step_by(2) {
            let key = format!("key_{i}");
            assert!(
                store.get(&key, later).unwrap().is_some(),
                "Long-TTL entry {i} should still exist"
            );
        }
    }

    #[test]
    fn test_no_cleanup_without_triggers() {
        let mut store = OptimizedMemoryStore::with_capacity(100);
        let now = SystemTime::now();

        // Add entries with long TTL
        for i in 0..100 {
            let key = format!("key_{i}");
            store
                .set_if_not_exists_with_ttl(&key, i, Duration::from_secs(3600), now)
                .unwrap();
        }

        // Do operations but not enough to trigger cleanup
        for i in 0..10 {
            let key = format!("key_{i}");
            let _ = store.get(&key, now);
        }

        // Verify no cleanup happened (all entries still valid)
        assert_eq!(store.len(), 100);
        assert_eq!(store.expired_count(), 0);
    }
}

#[cfg(test)]
mod tests {
    use crate::core::store::{AdaptiveStore, PeriodicStore, ProbabilisticStore, Store};
    use std::time::{Duration, SystemTime};

    #[test]
    fn test_periodic_store_builder() {
        let mut store = PeriodicStore::builder()
            .capacity(50_000)
            .cleanup_interval(Duration::from_secs(120))
            .build();

        // Test basic functionality
        let now = SystemTime::now();
        let ttl = Duration::from_secs(60);

        // Should be able to set a value
        assert!(
            store
                .set_if_not_exists_with_ttl("test_key", 100, ttl, now)
                .unwrap()
        );

        // Should retrieve the value
        assert_eq!(store.get("test_key", now).unwrap(), Some(100));
    }

    #[test]
    fn test_periodic_store_builder_defaults() {
        let mut store = PeriodicStore::builder().build();

        // Should work with default values
        let now = SystemTime::now();
        let ttl = Duration::from_secs(60);

        assert!(
            store
                .set_if_not_exists_with_ttl("key1", 42, ttl, now)
                .unwrap()
        );
        assert_eq!(store.get("key1", now).unwrap(), Some(42));
    }

    #[test]
    fn test_probabilistic_store_builder() {
        let mut store = ProbabilisticStore::builder()
            .capacity(75_000)
            .cleanup_probability(5_000)
            .build();

        // Test basic functionality
        let now = SystemTime::now();
        let ttl = Duration::from_secs(60);

        assert!(
            store
                .set_if_not_exists_with_ttl("prob_key", 200, ttl, now)
                .unwrap()
        );
        assert_eq!(store.get("prob_key", now).unwrap(), Some(200));
    }

    #[test]
    fn test_probabilistic_store_builder_defaults() {
        let mut store = ProbabilisticStore::builder().build();

        // Should work with default values
        let now = SystemTime::now();
        let ttl = Duration::from_secs(60);

        assert!(
            store
                .set_if_not_exists_with_ttl("key2", 84, ttl, now)
                .unwrap()
        );
        assert_eq!(store.get("key2", now).unwrap(), Some(84));
    }

    #[test]
    fn test_adaptive_store_builder() {
        let mut store = AdaptiveStore::builder()
            .capacity(100_000)
            .min_interval(Duration::from_secs(2))
            .max_interval(Duration::from_secs(600))
            .max_operations(500_000)
            .build();

        // Test basic functionality
        let now = SystemTime::now();
        let ttl = Duration::from_secs(60);

        assert!(
            store
                .set_if_not_exists_with_ttl("adaptive_key", 300, ttl, now)
                .unwrap()
        );
        assert_eq!(store.get("adaptive_key", now).unwrap(), Some(300));
    }

    #[test]
    fn test_adaptive_store_builder_defaults() {
        let mut store = AdaptiveStore::builder().build();

        // Should work with default values
        let now = SystemTime::now();
        let ttl = Duration::from_secs(60);

        assert!(
            store
                .set_if_not_exists_with_ttl("key3", 126, ttl, now)
                .unwrap()
        );
        assert_eq!(store.get("key3", now).unwrap(), Some(126));
    }

    #[test]
    fn test_adaptive_store_builder_partial_config() {
        // Test builder with only some fields set
        let mut store = AdaptiveStore::builder()
            .min_interval(Duration::from_secs(10))
            .max_operations(1_000_000)
            .build();

        let now = SystemTime::now();
        let ttl = Duration::from_secs(60);

        assert!(
            store
                .set_if_not_exists_with_ttl("key4", 168, ttl, now)
                .unwrap()
        );
        assert_eq!(store.get("key4", now).unwrap(), Some(168));
    }

    #[test]
    fn test_store_builder_large_capacity() {
        // Test that builders handle large capacities correctly
        let mut periodic = PeriodicStore::builder().capacity(1_000_000).build();

        let mut probabilistic = ProbabilisticStore::builder().capacity(1_000_000).build();

        let mut adaptive = AdaptiveStore::builder().capacity(1_000_000).build();

        let now = SystemTime::now();
        let ttl = Duration::from_secs(60);

        // All stores should handle operations correctly
        assert!(
            periodic
                .set_if_not_exists_with_ttl("p_key", 1, ttl, now)
                .unwrap()
        );
        assert!(
            probabilistic
                .set_if_not_exists_with_ttl("pr_key", 2, ttl, now)
                .unwrap()
        );
        assert!(
            adaptive
                .set_if_not_exists_with_ttl("a_key", 3, ttl, now)
                .unwrap()
        );
    }
}

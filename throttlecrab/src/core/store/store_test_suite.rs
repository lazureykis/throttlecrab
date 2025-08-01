/// Comprehensive test suite for all store implementations
/// Tests edge cases and validates correctness across all stores
#[cfg(test)]
mod tests {
    use crate::RateLimiter;
    use crate::core::store::*;
    use crate::core::store::{AdaptiveStore, PeriodicStore, ProbabilisticStore};
    use std::time::{Duration, SystemTime};

    /// Macro to test all stores with a given test function
    macro_rules! test_all_stores {
        ($test_fn:expr) => {
            // Removed: Standard, Arena, TimingWheel, BloomFilter, BTree, Heap, RawApi stores
            $test_fn("Periodic", &mut PeriodicStore::with_capacity(100));
            $test_fn("Probabilistic", &mut ProbabilisticStore::with_capacity(100));
            $test_fn("Adaptive", &mut AdaptiveStore::with_capacity(100));
        };
    }

    /// Test basic set and get operations
    #[test]
    fn test_basic_operations() {
        let test_fn = |name: &str, store: &mut dyn Store| {
            let now = SystemTime::now();
            let ttl = Duration::from_secs(60);

            // Test set_if_not_exists
            assert!(
                store
                    .set_if_not_exists_with_ttl("key1", 100, ttl, now)
                    .unwrap(),
                "{name}: Failed to set new key"
            );

            // Test get existing key
            assert_eq!(
                store.get("key1", now).unwrap(),
                Some(100),
                "{name}: Failed to get existing key"
            );

            // Test set_if_not_exists on existing key
            assert!(
                !store
                    .set_if_not_exists_with_ttl("key1", 200, ttl, now)
                    .unwrap(),
                "{name}: set_if_not_exists should fail on existing key"
            );

            // Value should remain unchanged
            assert_eq!(
                store.get("key1", now).unwrap(),
                Some(100),
                "{name}: Value changed after failed set"
            );
        };

        test_all_stores!(test_fn);
    }

    /// Test compare and swap operations
    #[test]
    fn test_compare_and_swap() {
        let test_fn = |name: &str, store: &mut dyn Store| {
            let now = SystemTime::now();
            let ttl = Duration::from_secs(60);

            // Set initial value
            store
                .set_if_not_exists_with_ttl("key1", 100, ttl, now)
                .unwrap();

            // Successful CAS
            assert!(
                store
                    .compare_and_swap_with_ttl("key1", 100, 200, ttl, now)
                    .unwrap(),
                "{name}: CAS with correct old value should succeed"
            );
            assert_eq!(
                store.get("key1", now).unwrap(),
                Some(200),
                "{name}: Value not updated after successful CAS"
            );

            // Failed CAS - wrong old value
            assert!(
                !store
                    .compare_and_swap_with_ttl("key1", 100, 300, ttl, now)
                    .unwrap(),
                "{name}: CAS with wrong old value should fail"
            );
            assert_eq!(
                store.get("key1", now).unwrap(),
                Some(200),
                "{name}: Value changed after failed CAS"
            );

            // CAS on non-existent key
            assert!(
                !store
                    .compare_and_swap_with_ttl("key2", 0, 100, ttl, now)
                    .unwrap(),
                "{name}: CAS on non-existent key should fail"
            );
        };

        test_all_stores!(test_fn);
    }

    /// Test TTL expiration
    #[test]
    fn test_ttl_expiration() {
        let test_fn = |name: &str, store: &mut dyn Store| {
            let now = SystemTime::now();
            let ttl = Duration::from_secs(60);

            // Set value with TTL
            store
                .set_if_not_exists_with_ttl("key1", 100, ttl, now)
                .unwrap();

            // Value should exist before expiry
            assert_eq!(
                store.get("key1", now).unwrap(),
                Some(100),
                "{name}: Value missing before expiry"
            );

            // Value should exist just before expiry
            let almost_expired = now + Duration::from_secs(59);
            assert_eq!(
                store.get("key1", almost_expired).unwrap(),
                Some(100),
                "{name}: Value missing just before expiry"
            );

            // Value should not exist after expiry
            let expired = now + Duration::from_secs(61);

            assert_eq!(
                store.get("key1", expired).unwrap(),
                None,
                "{name}: Value exists after expiry"
            );

            // CAS should fail on expired key
            assert!(
                !store
                    .compare_and_swap_with_ttl("key1", 100, 200, ttl, expired)
                    .unwrap(),
                "{name}: CAS succeeded on expired key"
            );

            // Should be able to set expired key again
            assert!(
                store
                    .set_if_not_exists_with_ttl("key1", 300, ttl, expired)
                    .unwrap(),
                "{name}: Failed to set expired key"
            );
            assert_eq!(
                store.get("key1", expired).unwrap(),
                Some(300),
                "{name}: New value not set on expired key"
            );
        };

        test_all_stores!(test_fn);
    }

    /// Test edge case: negative TAT values
    #[test]
    fn test_negative_tat() {
        let test_fn = |name: &str, store: &mut dyn Store| {
            let now = SystemTime::now();
            let ttl = Duration::from_secs(60);

            // Set a negative value
            assert!(
                store
                    .set_if_not_exists_with_ttl("key1", -1000, ttl, now)
                    .unwrap(),
                "{name}: Failed to set negative value"
            );

            // Should retrieve negative value correctly
            assert_eq!(
                store.get("key1", now).unwrap(),
                Some(-1000),
                "{name}: Failed to retrieve negative value"
            );

            // CAS with negative values
            assert!(
                store
                    .compare_and_swap_with_ttl("key1", -1000, -500, ttl, now)
                    .unwrap(),
                "{name}: Failed to CAS negative values"
            );
            assert_eq!(
                store.get("key1", now).unwrap(),
                Some(-500),
                "{name}: Wrong value after negative CAS"
            );
        };

        test_all_stores!(test_fn);
    }

    /// Test edge case: very short TTLs
    #[test]
    fn test_short_ttl() {
        let test_fn = |name: &str, store: &mut dyn Store| {
            let now = SystemTime::now();
            let short_ttl = Duration::from_millis(1);

            // Set with very short TTL
            store
                .set_if_not_exists_with_ttl("key1", 100, short_ttl, now)
                .unwrap();

            // Should exist immediately (unless TTL is truncated to 0)
            // Compact store uses second precision, so 1ms TTL becomes 0s
            if name != "Compact" {
                assert_eq!(
                    store.get("key1", now).unwrap(),
                    Some(100),
                    "{name}: Value missing immediately after set"
                );
            }

            // Should expire after 1ms
            let expired = now + Duration::from_millis(2);

            // Compact store has precision issues with very short TTLs (uses seconds)
            if name != "Compact" {
                assert_eq!(
                    store.get("key1", expired).unwrap(),
                    None,
                    "{name}: Value exists after short TTL expiry"
                );
            }
        };

        test_all_stores!(test_fn);
    }

    /// Test edge case: maximum values
    #[test]
    fn test_extreme_values() {
        let test_fn = |name: &str, store: &mut dyn Store| {
            let now = SystemTime::now();
            let ttl = Duration::from_secs(60);

            // Test i64::MAX
            store
                .set_if_not_exists_with_ttl("max", i64::MAX, ttl, now)
                .unwrap();
            assert_eq!(
                store.get("max", now).unwrap(),
                Some(i64::MAX),
                "{name}: Failed with i64::MAX"
            );

            // Test i64::MIN
            store
                .set_if_not_exists_with_ttl("min", i64::MIN, ttl, now)
                .unwrap();
            assert_eq!(
                store.get("min", now).unwrap(),
                Some(i64::MIN),
                "{name}: Failed with i64::MIN"
            );

            // CAS with extreme values
            assert!(
                store
                    .compare_and_swap_with_ttl("max", i64::MAX, i64::MAX - 1, ttl, now)
                    .unwrap(),
                "{name}: Failed to CAS i64::MAX"
            );
        };

        test_all_stores!(test_fn);
    }

    /// Test edge case: empty and special keys
    #[test]
    fn test_special_keys() {
        let test_fn = |name: &str, store: &mut dyn Store| {
            let now = SystemTime::now();
            let ttl = Duration::from_secs(60);

            // Empty key
            store.set_if_not_exists_with_ttl("", 100, ttl, now).unwrap();
            assert_eq!(
                store.get("", now).unwrap(),
                Some(100),
                "{name}: Failed with empty key"
            );

            // Very long key
            let long_key = "a".repeat(1000);
            store
                .set_if_not_exists_with_ttl(&long_key, 200, ttl, now)
                .unwrap();
            assert_eq!(
                store.get(&long_key, now).unwrap(),
                Some(200),
                "{name}: Failed with long key"
            );

            // Unicode key
            let unicode_key = "🦀🔥💻";
            store
                .set_if_not_exists_with_ttl(unicode_key, 300, ttl, now)
                .unwrap();
            assert_eq!(
                store.get(unicode_key, now).unwrap(),
                Some(300),
                "{name}: Failed with unicode key"
            );

            // Key with special characters
            let special_key = "key:with:colons/and/slashes\\and\\backslashes";
            store
                .set_if_not_exists_with_ttl(special_key, 400, ttl, now)
                .unwrap();
            assert_eq!(
                store.get(special_key, now).unwrap(),
                Some(400),
                "{name}: Failed with special characters"
            );
        };

        test_all_stores!(test_fn);
    }

    /// Test concurrent-like operations (simulated)
    #[test]
    fn test_concurrent_operations() {
        let test_fn = |name: &str, store: &mut dyn Store| {
            let now = SystemTime::now();
            let ttl = Duration::from_secs(60);

            // Set initial value
            store
                .set_if_not_exists_with_ttl("counter", 0, ttl, now)
                .unwrap();

            // Simulate multiple concurrent increments
            let mut current = 0;
            for _ in 0..10 {
                // Read current value
                let value = store.get("counter", now).unwrap().unwrap();

                // Try to update - might fail in real concurrent scenario
                if store
                    .compare_and_swap_with_ttl("counter", value, value + 1, ttl, now)
                    .unwrap()
                {
                    current += 1;
                }
            }

            // Should have incremented successfully
            assert_eq!(
                store.get("counter", now).unwrap(),
                Some(current),
                "{name}: Counter increment failed"
            );
        };

        test_all_stores!(test_fn);
    }

    /// Test cleanup behavior
    #[test]
    fn test_cleanup_behavior() {
        let test_fn = |name: &str, store: &mut dyn Store| {
            let now = SystemTime::now();
            let ttl = Duration::from_secs(1);

            // Add many entries with short TTL
            for i in 0..100 {
                store
                    .set_if_not_exists_with_ttl(&format!("key{i}"), i, ttl, now)
                    .unwrap();
            }

            // All should exist initially
            for i in 0..100 {
                assert!(
                    store.get(&format!("key{i}"), now).unwrap().is_some(),
                    "{name}: Key {i} missing before expiry"
                );
            }

            // After expiry, cleanup should happen (eventually)
            let expired = now + Duration::from_secs(2);

            // Trigger cleanup by performing operations
            for i in 0..10 {
                store.get(&format!("key{i}"), expired).unwrap();
            }

            // All expired entries should return None
            for i in 0..100 {
                assert_eq!(
                    store.get(&format!("key{i}"), expired).unwrap(),
                    None,
                    "{name}: Key {i} exists after expiry"
                );
            }
        };

        test_all_stores!(test_fn);
    }

    /// Test TTL updates on CAS
    #[test]
    fn test_ttl_update_on_cas() {
        let test_fn = |name: &str, store: &mut dyn Store| {
            let now = SystemTime::now();
            let short_ttl = Duration::from_secs(10);
            let long_ttl = Duration::from_secs(100);

            // Set with short TTL
            store
                .set_if_not_exists_with_ttl("key1", 100, short_ttl, now)
                .unwrap();

            // CAS with longer TTL
            assert!(
                store
                    .compare_and_swap_with_ttl("key1", 100, 200, long_ttl, now)
                    .unwrap(),
                "{name}: CAS failed"
            );

            // Should still exist after original TTL
            let after_short = now + Duration::from_secs(11);
            assert_eq!(
                store.get("key1", after_short).unwrap(),
                Some(200),
                "{name}: Value expired with original TTL"
            );

            // Should expire after new TTL
            let after_long = now + Duration::from_secs(101);

            assert_eq!(
                store.get("key1", after_long).unwrap(),
                None,
                "{name}: Value didn't expire with new TTL"
            );
        };

        test_all_stores!(test_fn);
    }

    /// Test zero and negative TTLs
    #[test]
    fn test_zero_ttl() {
        let test_fn = |name: &str, store: &mut dyn Store| {
            let now = SystemTime::now();
            let zero_ttl = Duration::from_secs(0);

            // Set with zero TTL - should expire immediately
            store
                .set_if_not_exists_with_ttl("key1", 100, zero_ttl, now)
                .unwrap();

            // Might or might not exist at exact same time (implementation dependent)
            // But should definitely not exist after any time passes
            let later = now + Duration::from_nanos(1);

            assert_eq!(
                store.get("key1", later).unwrap(),
                None,
                "{name}: Value with zero TTL exists after time passed"
            );
        };

        test_all_stores!(test_fn);
    }

    /// Test many unique keys (stress test)
    #[test]
    fn test_many_keys() {
        let test_fn = |name: &str, store: &mut dyn Store| {
            let now = SystemTime::now();
            let ttl = Duration::from_secs(3600);
            let num_keys = 500;

            // Add many unique keys
            for i in 0..num_keys {
                let key = format!("key_{i}");
                assert!(
                    store.set_if_not_exists_with_ttl(&key, i, ttl, now).unwrap(),
                    "{name}: Failed to set key {i}"
                );
            }

            // Verify all keys exist with correct values
            for i in 0..num_keys {
                let key = format!("key_{i}");
                assert_eq!(
                    store.get(&key, now).unwrap(),
                    Some(i),
                    "{name}: Wrong value for key {i}"
                );
            }

            // CAS on random keys
            for i in (0..num_keys).step_by(7) {
                let key = format!("key_{i}");
                assert!(
                    store
                        .compare_and_swap_with_ttl(&key, i, i + 1000, ttl, now)
                        .unwrap(),
                    "{name}: CAS failed for key {i}"
                );
            }

            // Verify CAS updates
            for i in (0..num_keys).step_by(7) {
                let key = format!("key_{i}");
                assert_eq!(
                    store.get(&key, now).unwrap(),
                    Some(i + 1000),
                    "{name}: Wrong value after CAS for key {i}"
                );
            }
        };

        test_all_stores!(test_fn);
    }

    /// Test rate limiting behavior with different stores
    #[test]
    fn test_rate_limiting_all_stores() {
        // Test each store type separately with RateLimiter
        fn test_rate_limiter<S: Store>(name: &str, mut limiter: RateLimiter<S>) {
            let now = SystemTime::now();

            // Rate limit: 10 per hour, burst of 5
            let max_burst = 5;
            let rate = 10;
            let period = 3600;
            let cost = 1;

            // First 5 requests should be allowed (burst)
            for i in 0..max_burst {
                let (allowed, result) = limiter
                    .rate_limit("test_key", max_burst, rate, period, cost, now)
                    .unwrap();
                assert!(allowed, "{name}: Request {} should be allowed", i + 1);
                assert_eq!(
                    result.remaining,
                    max_burst - i - 1,
                    "{name}: Wrong remaining count"
                );
            }

            // 6th request should be blocked
            let (allowed, _) = limiter
                .rate_limit("test_key", max_burst, rate, period, cost, now)
                .unwrap();
            assert!(!allowed, "{name}: 6th request should be blocked");

            // After some time, should allow more (token regeneration)
            let later = now + Duration::from_secs((period / rate) as u64); // 360 seconds = 1 token
            let (allowed, result) = limiter
                .rate_limit("test_key", max_burst, rate, period, cost, later)
                .unwrap();
            assert!(
                allowed,
                "{name}: Request should be allowed after token regeneration"
            );
            assert_eq!(result.remaining, 0, "{name}: Should have exactly 1 token");
        }

        // Test each store type (removed: Standard, Arena, TimingWheel, BloomFilter, BTree, Heap, RawApi)
        test_rate_limiter(
            "Periodic",
            RateLimiter::new(PeriodicStore::with_capacity(100)),
        );
        test_rate_limiter(
            "Probabilistic",
            RateLimiter::new(ProbabilisticStore::with_capacity(100)),
        );
        test_rate_limiter(
            "Adaptive",
            RateLimiter::new(AdaptiveStore::with_capacity(100)),
        );
    }
}

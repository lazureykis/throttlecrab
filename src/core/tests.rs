use super::{MemoryStore, RateLimiter};
use std::thread;
use std::time::{Duration, SystemTime};

#[test]
fn test_basic_rate_limiting() {
    let mut store = MemoryStore::new();
    let mut limiter = RateLimiter::new_from_parameters(
        &mut store, 5,  // max_burst
        10, // count_per_period
        60, // period
    )
    .unwrap();

    // First request should succeed
    let now = SystemTime::now();
    let (allowed, result) = limiter.rate_limit("test", 1, now).unwrap();
    assert!(allowed);
    assert_eq!(result.limit, 5);
    assert_eq!(result.remaining, 4);
}

#[test]
fn test_burst_capacity() {
    let mut store = MemoryStore::new();
    let mut limiter = RateLimiter::new_from_parameters(
        &mut store, 5,  // max_burst
        10, // count_per_period
        60, // period
    )
    .unwrap();

    // Should allow burst capacity requests
    let now = SystemTime::now();
    for i in 0..5 {
        let (allowed, result) = limiter.rate_limit("burst_test", 1, now).unwrap();
        assert!(allowed, "Request {} should be allowed", i + 1);
        assert_eq!(result.remaining, 5 - (i + 1) as i64);
    }

    // 6th request should be blocked
    let (allowed, result) = limiter.rate_limit("burst_test", 1, now).unwrap();
    assert!(!allowed);
    assert_eq!(result.remaining, 0);
    assert!(result.retry_after.as_secs() > 0);
}

#[test]
fn test_rate_replenishment() {
    let mut store = MemoryStore::new();
    let mut limiter = RateLimiter::new_from_parameters(
        &mut store, 2,  // max_burst
        60, // count_per_period (1 per second)
        60, // period
    )
    .unwrap();

    // Use all burst capacity
    let now = SystemTime::now();
    let (allowed1, _) = limiter.rate_limit("replenish_test", 1, now).unwrap();
    let (allowed2, _) = limiter.rate_limit("replenish_test", 1, now).unwrap();
    assert!(allowed1);
    assert!(allowed2);

    // Should be blocked
    let (allowed3, _result) = limiter.rate_limit("replenish_test", 1, now).unwrap();
    assert!(!allowed3);

    // Wait for replenishment
    thread::sleep(Duration::from_secs(2));

    // Should allow one more
    let later = SystemTime::now();
    let (allowed4, _) = limiter.rate_limit("replenish_test", 1, later).unwrap();
    assert!(allowed4);
}

#[test]
fn test_different_keys() {
    let mut store = MemoryStore::new();

    // Use a burst of 2 to make the test clearer
    let mut limiter = RateLimiter::new_from_parameters(
        &mut store, 2,  // max_burst
        2,  // count_per_period
        60, // period
    )
    .unwrap();

    // Different keys should have independent limits
    let now = SystemTime::now();
    let (allowed1, _) = limiter.rate_limit("key1", 1, now).unwrap();
    let (allowed2, _) = limiter.rate_limit("key2", 1, now).unwrap();
    assert!(allowed1);
    assert!(allowed2);

    // Use up remaining burst for key1
    let (allowed3, _) = limiter.rate_limit("key1", 1, now).unwrap();
    assert!(allowed3);

    // Third request for key1 should be blocked
    let (allowed4, _) = limiter.rate_limit("key1", 1, now).unwrap();
    assert!(!allowed4);

    // But key2 should still have one more
    let (allowed5, _) = limiter.rate_limit("key2", 1, now).unwrap();
    assert!(allowed5);

    // Now key2 should also be blocked
    let (allowed6, _) = limiter.rate_limit("key2", 1, now).unwrap();
    assert!(!allowed6);
}

#[test]
fn test_quantity_parameter() {
    let mut store = MemoryStore::new();
    let mut limiter = RateLimiter::new_from_parameters(
        &mut store, 10, // max_burst
        10, // count_per_period
        60, // period
    )
    .unwrap();

    // Request with quantity 5
    let now = SystemTime::now();
    let (allowed1, result1) = limiter.rate_limit("quantity_test", 5, now).unwrap();
    assert!(allowed1);
    assert_eq!(result1.remaining, 5);

    // Request with quantity 6 should be blocked
    let (allowed2, result2) = limiter.rate_limit("quantity_test", 6, now).unwrap();
    assert!(!allowed2);
    assert_eq!(result2.remaining, 5);

    // Request with quantity 5 should succeed
    let (allowed3, result3) = limiter.rate_limit("quantity_test", 5, now).unwrap();
    assert!(allowed3);
    assert_eq!(result3.remaining, 0);
}

#[test]
fn test_negative_quantity_error() {
    let mut store = MemoryStore::new();
    let mut limiter = RateLimiter::new_from_parameters(
        &mut store, 10, // max_burst
        10, // count_per_period
        60, // period
    )
    .unwrap();

    let now = SystemTime::now();
    let result = limiter.rate_limit("negative_test", -1, now);
    assert!(result.is_err());
}

#[test]
fn test_invalid_parameters() {
    let mut store = MemoryStore::new();

    // Test invalid burst
    let limiter = RateLimiter::new_from_parameters(
        &mut store, 0,  // invalid
        10, // count_per_period
        60, // period
    );
    assert!(limiter.is_err());

    // Test invalid count
    let limiter = RateLimiter::new_from_parameters(
        &mut store, 10, // max_burst
        0,  // invalid
        60, // period
    );
    assert!(limiter.is_err());

    // Test invalid period
    let limiter = RateLimiter::new_from_parameters(
        &mut store, 10, // max_burst
        10, // count_per_period
        0,  // invalid
    );
    assert!(limiter.is_err());
}

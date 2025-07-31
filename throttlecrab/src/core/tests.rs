use super::{PeriodicStore, RateLimiter};
use std::time::{Duration, SystemTime};

#[test]
fn test_basic_rate_limiting() {
    let mut limiter = RateLimiter::new(PeriodicStore::new());

    // First request should succeed
    let now = SystemTime::now();
    let (allowed, result) = limiter.rate_limit("test", 5, 10, 60, 1, now).unwrap();
    assert!(allowed);
    assert_eq!(result.limit, 5);
    assert_eq!(result.remaining, 4);
}

#[test]
fn test_burst_capacity() {
    let mut limiter = RateLimiter::new(PeriodicStore::new());

    // Should allow burst capacity requests
    let now = SystemTime::now();
    for i in 0..5 {
        let (allowed, result) = limiter.rate_limit("burst_test", 5, 10, 60, 1, now).unwrap();
        assert!(allowed, "Request {} should be allowed", i + 1);
        assert_eq!(result.remaining, 5 - (i + 1) as i64);
    }

    // 6th request should be blocked
    let (allowed, result) = limiter.rate_limit("burst_test", 5, 10, 60, 1, now).unwrap();
    assert!(!allowed);
    assert_eq!(result.remaining, 0);
    assert!(result.retry_after.as_secs() > 0);
}

#[test]
fn test_rate_replenishment() {
    let mut limiter = RateLimiter::new(PeriodicStore::new());

    // Use all burst capacity
    let now = SystemTime::now();
    let (allowed1, _) = limiter
        .rate_limit("replenish_test", 2, 60, 60, 1, now)
        .unwrap();
    let (allowed2, _) = limiter
        .rate_limit("replenish_test", 2, 60, 60, 1, now)
        .unwrap();
    assert!(allowed1);
    assert!(allowed2);

    // Should be blocked
    let (allowed3, _result) = limiter
        .rate_limit("replenish_test", 2, 60, 60, 1, now)
        .unwrap();
    assert!(!allowed3);

    // Should allow one more
    let later = now + Duration::from_secs(1);
    let (allowed4, _) = limiter
        .rate_limit("replenish_test", 2, 60, 60, 1, later)
        .unwrap();
    assert!(allowed4);
}

#[test]
fn test_different_keys() {
    let mut limiter = RateLimiter::new(PeriodicStore::new());

    // Use a burst of 2 to make the test clearer
    // Different keys should have independent limits
    let now = SystemTime::now();
    let (allowed1, _) = limiter.rate_limit("key1", 2, 2, 60, 1, now).unwrap();
    let (allowed2, _) = limiter.rate_limit("key2", 2, 2, 60, 1, now).unwrap();
    assert!(allowed1);
    assert!(allowed2);

    // Use up remaining burst for key1
    let (allowed3, _) = limiter.rate_limit("key1", 2, 2, 60, 1, now).unwrap();
    assert!(allowed3);

    // Third request for key1 should be blocked
    let (allowed4, _) = limiter.rate_limit("key1", 2, 2, 60, 1, now).unwrap();
    assert!(!allowed4);

    // But key2 should still have one more
    let (allowed5, _) = limiter.rate_limit("key2", 2, 2, 60, 1, now).unwrap();
    assert!(allowed5);

    // Now key2 should also be blocked
    let (allowed6, _) = limiter.rate_limit("key2", 2, 2, 60, 1, now).unwrap();
    assert!(!allowed6);
}

#[test]
fn test_quantity_parameter() {
    let mut limiter = RateLimiter::new(PeriodicStore::new());

    // Request with quantity 5
    let now = SystemTime::now();
    let (allowed1, result1) = limiter
        .rate_limit("quantity_test", 10, 10, 60, 5, now)
        .unwrap();
    assert!(allowed1);
    assert_eq!(result1.remaining, 5);

    // Request with quantity 6 should be blocked
    let (allowed2, result2) = limiter
        .rate_limit("quantity_test", 10, 10, 60, 6, now)
        .unwrap();
    assert!(!allowed2);
    assert_eq!(result2.remaining, 5);

    // Request with quantity 5 should succeed
    let (allowed3, result3) = limiter
        .rate_limit("quantity_test", 10, 10, 60, 5, now)
        .unwrap();
    assert!(allowed3);
    assert_eq!(result3.remaining, 0);
}

#[test]
fn test_negative_quantity_error() {
    let mut limiter = RateLimiter::new(PeriodicStore::new());

    let now = SystemTime::now();
    let result = limiter.rate_limit("negative_test", 10, 10, 60, -1, now);
    assert!(result.is_err());
}

#[test]
fn test_invalid_parameters() {
    let mut limiter = RateLimiter::new(PeriodicStore::new());
    let now = SystemTime::now();

    // Test invalid burst
    let result = limiter.rate_limit("test", 0, 10, 60, 1, now);
    assert!(result.is_err());

    // Test invalid count
    let result = limiter.rate_limit("test", 10, 0, 60, 1, now);
    assert!(result.is_err());

    // Test invalid period
    let result = limiter.rate_limit("test", 10, 10, 0, 1, now);
    assert!(result.is_err());
}

#[test]
fn test_large_quantity_overflow_protection() {
    let mut limiter = RateLimiter::new(PeriodicStore::new());

    // Test with very large quantity that could cause overflow
    let now = SystemTime::now();
    let result = limiter.rate_limit("overflow_test", 10, 10, 60, i64::MAX / 2, now);

    // Should not panic, should handle gracefully
    assert!(result.is_ok());
    let (allowed, _) = result.unwrap();
    // With such a large quantity, it should be rejected
    assert!(!allowed);
}

#[test]
fn test_saturating_arithmetic() {
    let mut limiter = RateLimiter::new(PeriodicStore::new());

    // Test edge cases that could cause overflow
    let now = SystemTime::now();

    // Large burst capacity
    let result = limiter.rate_limit("saturate_test", i64::MAX / 1000, 100, 60, 1, now);
    assert!(result.is_ok());

    // Large count per period
    let result = limiter.rate_limit("saturate_test2", 10, i64::MAX / 1000, 60, 1, now);
    assert!(result.is_ok());
}

#[test]
fn test_daylight_saving_time_transitions() {
    let mut limiter = RateLimiter::new(PeriodicStore::new());

    // Simulate DST scenarios where time can jump forward or backward by 1 hour
    let base_time = SystemTime::now();

    // Test 1: Spring forward - clock jumps ahead 1 hour
    // Make some requests before the jump
    let (allowed1, result1) = limiter
        .rate_limit("dst_test", 5, 10, 60, 1, base_time)
        .unwrap();
    assert!(allowed1);
    assert_eq!(result1.remaining, 4);

    let (allowed2, result2) = limiter
        .rate_limit("dst_test", 5, 10, 60, 1, base_time)
        .unwrap();
    assert!(allowed2);
    assert_eq!(result2.remaining, 3);

    // Jump forward 1 hour (spring DST transition)
    let spring_forward = base_time + Duration::from_secs(3600);
    let (allowed3, result3) = limiter
        .rate_limit("dst_test", 5, 10, 60, 1, spring_forward)
        .unwrap();
    assert!(allowed3);
    // After an hour, we should have replenished tokens
    // With 10 requests per 60 seconds, after 3600 seconds we should have all tokens back
    assert_eq!(result3.remaining, 4);

    // Test 2: Fall back - clock jumps back 1 hour
    // This is the tricky case where time appears to go backwards
    let fall_back = base_time - Duration::from_secs(3600);

    // When time goes backwards, our implementation should handle it gracefully
    let result4 = limiter.rate_limit("dst_test2", 5, 10, 60, 1, fall_back);
    assert!(result4.is_ok());
    let (allowed4, _) = result4.unwrap();
    // Should still allow requests as we handle time going backwards gracefully
    assert!(allowed4);
}

#[test]
fn test_rapid_time_changes() {
    let mut limiter = RateLimiter::new(PeriodicStore::new());

    // Test rapid time changes that might occur during system clock adjustments
    let base_time = SystemTime::now();

    // Make a request at base time
    let (allowed1, _) = limiter
        .rate_limit("time_jump", 3, 10, 60, 1, base_time)
        .unwrap();
    assert!(allowed1);

    // Jump backward 5 seconds
    let time_back = base_time - Duration::from_secs(5);
    let result_back = limiter.rate_limit("time_jump", 3, 10, 60, 1, time_back);
    assert!(result_back.is_ok());

    // Jump forward 10 seconds from original
    let time_forward = base_time + Duration::from_secs(10);
    let (allowed2, _) = limiter
        .rate_limit("time_jump", 3, 10, 60, 1, time_forward)
        .unwrap();
    assert!(allowed2);

    // Multiple rapid changes
    for i in 0..5 {
        let jittered_time = if i % 2 == 0 {
            base_time + Duration::from_secs(i)
        } else {
            base_time - Duration::from_secs(i)
        };

        let result = limiter.rate_limit("time_jitter", 10, 10, 60, 1, jittered_time);
        // Should handle all time changes without panicking
        assert!(result.is_ok());
    }
}

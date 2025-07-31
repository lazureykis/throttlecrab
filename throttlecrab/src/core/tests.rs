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
fn test_remaining_count_accuracy() {
    let mut limiter = RateLimiter::new(PeriodicStore::new());
    let start_time = SystemTime::now();
    
    // Test parameters: burst=5, rate=10/60s (1 token per 6 seconds)
    let burst = 5;
    let rate = 10;
    let period = 60;
    
    // Test 1: First request should have burst-1 remaining
    let (allowed, result) = limiter
        .rate_limit("remaining_test", burst, rate, period, 1, start_time)
        .unwrap();
    assert!(allowed);
    assert_eq!(result.remaining, 4, "First request should leave 4 remaining");
    
    // Test 2: Consume rest of burst capacity
    for i in 2..=5 {
        let (allowed, result) = limiter
            .rate_limit("remaining_test", burst, rate, period, 1, start_time)
            .unwrap();
        assert!(allowed, "Request {} should be allowed", i);
        assert_eq!(
            result.remaining, 
            (5 - i) as i64, 
            "Request {} should leave {} remaining", 
            i, 
            5 - i
        );
    }
    
    // Test 3: Next request should be blocked with 0 remaining
    let (allowed, result) = limiter
        .rate_limit("remaining_test", burst, rate, period, 1, start_time)
        .unwrap();
    assert!(!allowed, "Should be rate limited after burst");
    assert_eq!(result.remaining, 0, "Should have 0 remaining when blocked");
    assert!(result.retry_after.as_secs() > 0, "Should have positive retry_after");
    
    // Test 4: After token replenishment, should allow with 0 remaining
    // Wait for one token to replenish (6 seconds in this case)
    let after_replenish = start_time + Duration::from_secs(6);
    let (allowed, result) = limiter
        .rate_limit("remaining_test", burst, rate, period, 1, after_replenish)
        .unwrap();
    assert!(allowed, "Should allow after replenishment");
    assert_eq!(result.remaining, 0, "Should have 0 remaining after using replenished token");
    
    // Test 5: Immediate next request should be blocked again
    let (allowed, result) = limiter
        .rate_limit("remaining_test", burst, rate, period, 1, after_replenish)
        .unwrap();
    assert!(!allowed, "Should be blocked again");
    assert_eq!(result.remaining, 0);
    
    // Test 6: Test with larger quantity
    let (allowed, result) = limiter
        .rate_limit("quantity_remaining", burst, rate, period, 3, start_time)
        .unwrap();
    assert!(allowed);
    assert_eq!(result.remaining, 2, "Using quantity=3 should leave 2 remaining");
    
    // Test 7: Quantity larger than remaining should be blocked
    let (allowed, result) = limiter
        .rate_limit("quantity_remaining", burst, rate, period, 3, start_time)
        .unwrap();
    assert!(!allowed, "Quantity larger than remaining should be blocked");
    assert_eq!(result.remaining, 2, "Remaining should not change on blocked request");
    
    // Test 8: But smaller quantity should work
    let (allowed, result) = limiter
        .rate_limit("quantity_remaining", burst, rate, period, 2, start_time)
        .unwrap();
    assert!(allowed, "Quantity equal to remaining should be allowed");
    assert_eq!(result.remaining, 0);
    
    // Test 9: Edge case - very high rate (multiple tokens per second)
    let (allowed, result) = limiter
        .rate_limit("high_rate", 10, 600, 60, 1, start_time)
        .unwrap();
    assert!(allowed);
    assert_eq!(result.remaining, 9);
    
    // After 1 second, should have replenished ~10 tokens
    let one_sec_later = start_time + Duration::from_secs(1);
    
    // Use up the burst
    for _ in 0..9 {
        limiter.rate_limit("high_rate", 10, 600, 60, 1, start_time).unwrap();
    }
    
    // Should have replenished some tokens after 1 second
    let (allowed, result) = limiter
        .rate_limit("high_rate", 10, 600, 60, 1, one_sec_later)
        .unwrap();
    assert!(allowed, "Should have replenished tokens");
    assert!(result.remaining < 10, "Should not be at full capacity immediately");
}

#[test]
fn test_remaining_count_all_stores() {
    use super::{AdaptiveStore, ProbabilisticStore};
    
    // Test the same scenario with all store types
    fn test_scenario<S: super::Store>(mut limiter: RateLimiter<S>) {
        let now = SystemTime::now();
        
        // Use burst of 3 for simpler testing
        let burst = 3;
        let rate = 6;
        let period = 60;
        
        // Consume all burst
        for i in 1..=3 {
            let (allowed, result) = limiter
                .rate_limit("test_key", burst, rate, period, 1, now)
                .unwrap();
            assert!(allowed, "Request {} should be allowed", i);
            assert_eq!(result.remaining, (3 - i) as i64);
        }
        
        // Next should be blocked
        let (allowed, result) = limiter
            .rate_limit("test_key", burst, rate, period, 1, now)
            .unwrap();
        assert!(!allowed);
        assert_eq!(result.remaining, 0);
        
        // After 10 seconds (1 token replenished)
        let later = now + Duration::from_secs(10);
        let (allowed, result) = limiter
            .rate_limit("test_key", burst, rate, period, 1, later)
            .unwrap();
        assert!(allowed);
        assert_eq!(result.remaining, 0, "Should use the replenished token immediately");
    }
    
    // Test with PeriodicStore
    test_scenario(RateLimiter::new(PeriodicStore::new()));
    
    // Test with AdaptiveStore
    test_scenario(RateLimiter::new(AdaptiveStore::new()));
    
    // Test with ProbabilisticStore
    test_scenario(RateLimiter::new(ProbabilisticStore::new()));
}

#[test]
fn test_edge_cases_zero_remaining() {
    let mut limiter = RateLimiter::new(PeriodicStore::new());
    let now = SystemTime::now();
    
    // Edge case 1: Exact token replenishment timing
    // burst=2, rate=120/60s = 2 per second
    let (allowed, result) = limiter
        .rate_limit("exact_timing", 2, 120, 60, 1, now)
        .unwrap();
    assert!(allowed);
    assert_eq!(result.remaining, 1);
    
    let (allowed, result) = limiter
        .rate_limit("exact_timing", 2, 120, 60, 1, now)
        .unwrap();
    assert!(allowed);
    assert_eq!(result.remaining, 0);
    
    // Exactly 0.5 seconds later - should have 1 token
    let half_sec = now + Duration::from_millis(500);
    let (allowed, result) = limiter
        .rate_limit("exact_timing", 2, 120, 60, 1, half_sec)
        .unwrap();
    assert!(allowed);
    assert_eq!(result.remaining, 0);
    
    // Edge case 2: Division by zero protection
    // This would cause emission_interval to be 0
    let result = limiter.rate_limit("zero_period", 10, 10, 0, 1, now);
    assert!(result.is_err(), "Zero period should error");
    
    // Edge case 3: Fractional tokens
    // burst=3, rate=7/60s means ~8.57 seconds per token
    let (allowed, result) = limiter
        .rate_limit("fractional", 3, 7, 60, 1, now)
        .unwrap();
    assert!(allowed);
    assert_eq!(result.remaining, 2);
    
    // Use all burst
    limiter.rate_limit("fractional", 3, 7, 60, 1, now).unwrap();
    limiter.rate_limit("fractional", 3, 7, 60, 1, now).unwrap();
    
    // After 8 seconds, should still not have a token
    let eight_sec = now + Duration::from_secs(8);
    let (allowed, _) = limiter
        .rate_limit("fractional", 3, 7, 60, 1, eight_sec)
        .unwrap();
    assert!(!allowed, "Should not have token after 8 seconds");
    
    // After 9 seconds, should have a token
    let nine_sec = now + Duration::from_secs(9);
    let (allowed, result) = limiter
        .rate_limit("fractional", 3, 7, 60, 1, nine_sec)
        .unwrap();
    assert!(allowed, "Should have token after 9 seconds");
    assert_eq!(result.remaining, 0);
    
    // Edge case 4: Maximum values
    let (allowed, result) = limiter
        .rate_limit("max_burst", i64::MAX / 1000, 100, 60, 1, now)
        .unwrap();
    assert!(allowed);
    assert!(result.remaining > 0, "Should handle large burst values");
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

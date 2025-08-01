use std::time::SystemTime;
use throttlecrab::{PeriodicStore, RateLimiter};

fn main() {
    // Create a rate limiter with periodic cleanup store
    let mut limiter = RateLimiter::new(PeriodicStore::with_capacity(1000));

    // Simulate API requests with rate limiting
    // Allow 10 requests per minute with burst of 5
    let key = "user_123";
    let max_burst = 5;
    let count_per_period = 10;
    let period = 60; // seconds

    println!("Rate limit: {count_per_period} requests per {period} seconds (burst: {max_burst})");
    println!();

    // Make some requests
    for i in 1..=12 {
        let (allowed, result) = limiter
            .rate_limit(
                key,
                max_burst,
                count_per_period,
                period,
                1,
                SystemTime::now(),
            )
            .unwrap();

        if allowed {
            println!("Request #{i}: ✓ Allowed (remaining: {})", result.remaining);
        } else {
            println!(
                "Request #{i}: ✗ Denied (retry after: {:.1}s)",
                result.retry_after.as_secs_f64()
            );
        }
    }
}

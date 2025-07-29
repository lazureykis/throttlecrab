use throttlecrab::{MemoryStore, RateLimiter};
use std::time::SystemTime;

fn main() {
    // Create a rate limiter with memory store
    let mut limiter = RateLimiter::new(MemoryStore::new());

    // Simulate API requests with rate limiting
    // Allow 10 requests per minute with burst of 5
    let key = "user_123";
    let max_burst = 5;
    let count_per_period = 10;
    let period = 60; // seconds

    println!("Rate limit: {} requests per {} seconds (burst: {})", 
             count_per_period, period, max_burst);
    println!();

    // Make some requests
    for i in 1..=12 {
        let (allowed, result) = limiter
            .rate_limit(key, max_burst, count_per_period, period, 1, SystemTime::now())
            .unwrap();

        if allowed {
            println!(
                "Request #{}: ✓ Allowed (remaining: {})",
                i, result.remaining
            );
        } else {
            println!(
                "Request #{}: ✗ Denied (retry after: {:.1}s)",
                i,
                result.retry_after.as_secs_f64()
            );
        }
    }
}
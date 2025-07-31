use std::time::{Instant, SystemTime};
use throttlecrab::{AdaptiveStore, PeriodicStore, ProbabilisticStore, RateLimiter};

fn benchmark_store<S: throttlecrab::Store>(
    name: &str,
    mut limiter: RateLimiter<S>,
    num_keys: usize,
    iterations: usize,
) {
    let start = Instant::now();
    let mut allowed_count = 0;
    let mut blocked_count = 0;

    for i in 0..iterations {
        let key = format!("key_{}", i % num_keys);
        let (allowed, _result) = limiter
            .rate_limit(&key, 100, 1000, 3600, 1, SystemTime::now())
            .unwrap();

        if allowed {
            allowed_count += 1;
        } else {
            blocked_count += 1;
        }
    }

    let elapsed = start.elapsed();
    let ops_per_sec = iterations as f64 / elapsed.as_secs_f64();

    println!(
        "{:<25} | {:>10} ops/s | Allowed: {:>7} | Blocked: {:>7} | Time: {:?}",
        name, ops_per_sec as u64, allowed_count, blocked_count, elapsed
    );
}

fn main() {
    println!("ThrottleCrab Store Performance Comparison");
    println!("=========================================");
    println!();

    let num_keys = 2000;
    let iterations = 400_000;

    println!("Configuration:");
    println!("  Unique keys: {num_keys}");
    println!("  Total operations: {iterations}");
    println!("  Rate limit: 1000 requests per 3600 seconds (1 hour)");
    println!("  Burst: 100");
    println!();

    println!(
        "Store Implementation      | Throughput   | Allowed         | Blocked         | Total Time"
    );
    println!("{}", "-".repeat(90));

    // PeriodicStore
    benchmark_store(
        "Periodic Store",
        RateLimiter::new(PeriodicStore::with_capacity(num_keys)),
        num_keys,
        iterations,
    );

    // ProbabilisticStore
    benchmark_store(
        "Probabilistic Store",
        RateLimiter::new(ProbabilisticStore::with_capacity(num_keys)),
        num_keys,
        iterations,
    );

    // AdaptiveStore
    benchmark_store(
        "Adaptive Store",
        RateLimiter::new(AdaptiveStore::with_capacity(num_keys)),
        num_keys,
        iterations,
    );

    println!();
    println!("Note: Results may vary based on system load and CPU characteristics.");
}

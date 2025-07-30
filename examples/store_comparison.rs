use std::time::{Instant, SystemTime};
use throttlecrab::RateLimiter;
use throttlecrab::core::store::{
    adaptive_cleanup::AdaptiveMemoryStore,
    amortized::{AmortizedMemoryStore, ProbabilisticMemoryStore},
    optimized::{InternedMemoryStore, OptimizedMemoryStore},
};

fn benchmark_store<S: throttlecrab::core::store::Store>(
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

    // OptimizedMemoryStore
    benchmark_store(
        "Optimized MemoryStore",
        RateLimiter::new(OptimizedMemoryStore::with_capacity(num_keys)),
        num_keys,
        iterations,
    );

    // InternedMemoryStore
    benchmark_store(
        "Interned MemoryStore",
        RateLimiter::new(InternedMemoryStore::with_capacity(num_keys)),
        num_keys,
        iterations,
    );

    // AmortizedMemoryStore
    benchmark_store(
        "Amortized MemoryStore",
        RateLimiter::new(AmortizedMemoryStore::with_capacity(num_keys)),
        num_keys,
        iterations,
    );

    // ProbabilisticMemoryStore
    benchmark_store(
        "Probabilistic MemoryStore",
        RateLimiter::new(ProbabilisticMemoryStore::with_capacity(num_keys)),
        num_keys,
        iterations,
    );

    // AdaptiveMemoryStore
    benchmark_store(
        "Adaptive MemoryStore",
        RateLimiter::new(AdaptiveMemoryStore::with_capacity(num_keys)),
        num_keys,
        iterations,
    );

    println!();
    println!("Note: Results may vary based on system load and CPU characteristics.");
}

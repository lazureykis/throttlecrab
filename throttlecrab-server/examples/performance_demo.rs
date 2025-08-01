use std::time::{Instant, SystemTime};
use throttlecrab::{PeriodicStore, RateLimiter};

fn benchmark_store(name: &str, mut limiter: RateLimiter<impl throttlecrab::Store>) {
    println!("\n{name} Benchmark");
    println!("{}", "-".repeat(40));

    // Warmup with 10,000 unique keys
    println!("Warming up with 10,000 unique keys...");
    for i in 0..10_000 {
        let key = format!("warmup_key_{i}");
        limiter
            .rate_limit(&key, 100, 1000, 60, 1, SystemTime::now())
            .unwrap();
    }

    // Benchmark with rotating keys
    let num_iterations = 100_000;
    let num_keys = 1_000;

    println!("Running {num_iterations} iterations with {num_keys} rotating keys...");
    let start = Instant::now();

    for i in 0..num_iterations {
        let key = format!("bench_key_{}", i % num_keys);
        limiter
            .rate_limit(&key, 100, 1000, 60, 1, SystemTime::now())
            .unwrap();
    }

    let duration = start.elapsed();
    let throughput = num_iterations as f64 / duration.as_secs_f64();

    println!("Duration: {duration:?}");
    println!("Throughput: {throughput:.0} req/s");
    println!(
        "Average latency: {:.2} µs/req",
        duration.as_micros() as f64 / num_iterations as f64
    );
}

fn main() {
    println!("ThrottleCrab Performance Demo");
    println!("=============================");

    // Note: The throttlecrab library uses AHash by default for fast hashing

    // Benchmark optimized store
    let periodic_limiter = RateLimiter::new(PeriodicStore::with_capacity(10_000));
    benchmark_store("Periodic Store", periodic_limiter);

    // Show improvement
    println!("\n📊 Performance Summary");
    println!("{}", "=".repeat(40));
    println!("The PeriodicStore with AHash provides:");
    println!("- Deferred cleanup (only every 60s or when 20% expired)");
    println!("- Pre-allocated capacity to avoid rehashing");
    println!("- Fast AHash hashing (SIMD-optimized)");
}

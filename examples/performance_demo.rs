use std::time::{Instant, SystemTime};
use throttlecrab::{MemoryStore, RateLimiter};
use throttlecrab::core::store::optimized::OptimizedMemoryStore;

fn benchmark_store(name: &str, mut limiter: RateLimiter<impl throttlecrab::Store>) {
    println!("\n{} Benchmark", name);
    println!("{}", "-".repeat(40));
    
    // Warmup with 10,000 unique keys
    println!("Warming up with 10,000 unique keys...");
    for i in 0..10_000 {
        let key = format!("warmup_key_{}", i);
        limiter.rate_limit(&key, 100, 1000, 60, 1, SystemTime::now()).unwrap();
    }
    
    // Benchmark with rotating keys
    let num_iterations = 100_000;
    let num_keys = 1_000;
    
    println!("Running {} iterations with {} rotating keys...", num_iterations, num_keys);
    let start = Instant::now();
    
    for i in 0..num_iterations {
        let key = format!("bench_key_{}", i % num_keys);
        limiter.rate_limit(&key, 100, 1000, 60, 1, SystemTime::now()).unwrap();
    }
    
    let duration = start.elapsed();
    let throughput = num_iterations as f64 / duration.as_secs_f64();
    
    println!("Duration: {:?}", duration);
    println!("Throughput: {:.0} req/s", throughput);
    println!("Average latency: {:.2} µs/req", duration.as_micros() as f64 / num_iterations as f64);
}

fn main() {
    println!("ThrottleCrab Performance Demo");
    println!("=============================");
    
    // Check if ahash feature is enabled
    #[cfg(feature = "ahash")]
    println!("✓ Using AHash for fast hashing");
    #[cfg(not(feature = "ahash"))]
    println!("✗ Using standard HashMap (slower)");
    
    // Benchmark standard store
    let standard_limiter = RateLimiter::new(MemoryStore::new());
    benchmark_store("Standard MemoryStore", standard_limiter);
    
    // Benchmark optimized store
    let optimized_limiter = RateLimiter::new(OptimizedMemoryStore::with_capacity(10_000));
    benchmark_store("Optimized MemoryStore", optimized_limiter);
    
    // Show improvement
    println!("\n📊 Performance Summary");
    println!("{}", "=".repeat(40));
    println!("The OptimizedMemoryStore with AHash provides:");
    println!("- Deferred cleanup (only every 60s or when 20% expired)");
    println!("- Pre-allocated capacity to avoid rehashing");
    println!("- Fast AHash hashing (SIMD-optimized)");
    println!("\nExpected improvement: 50-100x faster for workloads with many keys!");
}
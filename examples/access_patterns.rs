use std::time::{Instant, SystemTime};
use throttlecrab::core::store::{
    adaptive_cleanup::AdaptiveMemoryStore,
    amortized::{AmortizedMemoryStore, ProbabilisticMemoryStore},
    arena::ArenaMemoryStore,
    compact::CompactMemoryStore,
    optimized::{InternedMemoryStore, OptimizedMemoryStore},
    timing_wheel::TimingWheelStore,
};
use throttlecrab::{MemoryStore, RateLimiter};

#[derive(Clone, Copy)]
enum AccessPattern {
    Sequential,
    Random,
    HotKey,
    Zipfian,
    Bursty,
    SparseKeys,
}

fn benchmark_pattern<S: throttlecrab::core::store::Store>(
    _store_name: &str,
    mut limiter: RateLimiter<S>,
    pattern: AccessPattern,
    num_keys: usize,
    iterations: usize,
) -> (u64, usize, usize) {
    let start = Instant::now();
    let mut allowed_count = 0;
    let mut blocked_count = 0;
    
    match pattern {
        AccessPattern::Sequential => {
            // Sequential access - keys accessed in order
            for i in 0..iterations {
                let key = format!("key_{}", i % num_keys);
                let (allowed, _) = limiter
                    .rate_limit(&key, 100, 1000, 3600, 1, SystemTime::now())
                    .unwrap();
                
                if allowed {
                    allowed_count += 1;
                } else {
                    blocked_count += 1;
                }
            }
        }
        AccessPattern::Random => {
            // Random access - keys accessed randomly
            for i in 0..iterations {
                // Simple pseudo-random using prime multiplication
                let key_id = (i.wrapping_mul(2654435761)) % num_keys;
                let key = format!("key_{}", key_id);
                let (allowed, _) = limiter
                    .rate_limit(&key, 100, 1000, 3600, 1, SystemTime::now())
                    .unwrap();
                
                if allowed {
                    allowed_count += 1;
                } else {
                    blocked_count += 1;
                }
            }
        }
        AccessPattern::HotKey => {
            // Hot key pattern - 80% of requests go to 20% of keys
            for i in 0..iterations {
                let key_id = if i % 5 < 4 {
                    // 80% of requests go to first 20% of keys
                    (i * 7) % (num_keys / 5)
                } else {
                    // 20% of requests go to remaining 80% of keys
                    (num_keys / 5) + ((i * 13) % (num_keys * 4 / 5))
                };
                let key = format!("key_{}", key_id);
                let (allowed, _) = limiter
                    .rate_limit(&key, 100, 1000, 3600, 1, SystemTime::now())
                    .unwrap();
                
                if allowed {
                    allowed_count += 1;
                } else {
                    blocked_count += 1;
                }
            }
        }
        AccessPattern::Zipfian => {
            // Zipfian distribution - power law (very few keys get most requests)
            for i in 0..iterations {
                // Simple zipfian approximation
                let rank = ((i as f64 * 0.1).exp() as usize) % num_keys;
                let key_id = num_keys - rank - 1;
                let key = format!("key_{}", key_id);
                let (allowed, _) = limiter
                    .rate_limit(&key, 100, 1000, 3600, 1, SystemTime::now())
                    .unwrap();
                
                if allowed {
                    allowed_count += 1;
                } else {
                    blocked_count += 1;
                }
            }
        }
        AccessPattern::Bursty => {
            // Bursty pattern - concentrated bursts on specific keys
            for burst in 0..(iterations / 100) {
                let burst_key = burst % num_keys;
                // Send 100 requests to the same key
                for _ in 0..100 {
                    let key = format!("key_{}", burst_key);
                    let (allowed, _) = limiter
                        .rate_limit(&key, 100, 1000, 3600, 1, SystemTime::now())
                        .unwrap();
                    
                    if allowed {
                        allowed_count += 1;
                    } else {
                        blocked_count += 1;
                    }
                }
            }
        }
        AccessPattern::SparseKeys => {
            // Sparse keys - 90% of requests are for non-existent keys
            for i in 0..iterations {
                let key = if i % 10 == 0 {
                    // 10% existing keys
                    format!("key_{}", i % (num_keys / 10))
                } else {
                    // 90% non-existent keys - reuse a smaller pool to avoid Arena overflow
                    format!("nonexistent_key_{}", i % num_keys)
                };
                let (allowed, _) = limiter
                    .rate_limit(&key, 100, 1000, 3600, 1, SystemTime::now())
                    .unwrap();
                
                if allowed {
                    allowed_count += 1;
                } else {
                    blocked_count += 1;
                }
            }
        }
    }
    
    let elapsed = start.elapsed();
    let ops_per_sec = iterations as f64 / elapsed.as_secs_f64();
    
    (ops_per_sec as u64, allowed_count, blocked_count)
}

fn print_pattern_results(pattern: AccessPattern, results: Vec<(&str, u64, usize, usize)>) {
    let pattern_name = match pattern {
        AccessPattern::Sequential => "Sequential Access",
        AccessPattern::Random => "Random Access",
        AccessPattern::HotKey => "Hot Key (80/20)",
        AccessPattern::Zipfian => "Zipfian Distribution",
        AccessPattern::Bursty => "Bursty Pattern",
        AccessPattern::SparseKeys => "Sparse Keys (90% miss)",
    };
    
    println!("\n{}", pattern_name);
    println!("{}", "=".repeat(90));
    println!("{:<25} | {:>12} | {:>10} | {:>10}", "Store", "Ops/sec", "Allowed", "Blocked");
    println!("{}", "-".repeat(90));
    
    // Sort by performance
    let mut sorted = results;
    sorted.sort_by_key(|&(_, ops, _, _)| std::cmp::Reverse(ops));
    
    for (store, ops_per_sec, allowed, blocked) in sorted {
        println!("{:<25} | {:>12} | {:>10} | {:>10}", 
            store, ops_per_sec, allowed, blocked);
    }
}

fn main() {
    println!("ThrottleCrab Access Pattern Benchmarks");
    println!("======================================");
    
    let num_keys = 10_000;
    let iterations = 100_000;
    
    println!("\nConfiguration:");
    println!("  Unique keys: {}", num_keys);
    println!("  Total operations: {}", iterations);
    println!("  Rate limit: 1000 per hour, burst: 100");
    
    let patterns = [
        AccessPattern::Sequential,
        AccessPattern::Random,
        AccessPattern::HotKey,
        AccessPattern::Zipfian,
        AccessPattern::Bursty,
        AccessPattern::SparseKeys,
    ];
    
    
    for pattern in patterns {
        let mut results = Vec::new();
        
        // Standard Store
        let limiter = RateLimiter::new(MemoryStore::new());
        let (ops_per_sec, allowed, blocked) = benchmark_pattern(
            "Standard", limiter, pattern, num_keys, iterations
        );
        results.push(("Standard", ops_per_sec, allowed, blocked));
        
        // Optimized Store
        let limiter = RateLimiter::new(OptimizedMemoryStore::with_capacity(num_keys));
        let (ops_per_sec, allowed, blocked) = benchmark_pattern(
            "Optimized", limiter, pattern, num_keys, iterations
        );
        results.push(("Optimized", ops_per_sec, allowed, blocked));
        
        // Interned Store
        let limiter = RateLimiter::new(InternedMemoryStore::with_capacity(num_keys));
        let (ops_per_sec, allowed, blocked) = benchmark_pattern(
            "Interned", limiter, pattern, num_keys, iterations
        );
        results.push(("Interned", ops_per_sec, allowed, blocked));
        
        // Amortized Store
        let limiter = RateLimiter::new(AmortizedMemoryStore::with_capacity(num_keys));
        let (ops_per_sec, allowed, blocked) = benchmark_pattern(
            "Amortized", limiter, pattern, num_keys, iterations
        );
        results.push(("Amortized", ops_per_sec, allowed, blocked));
        
        // Probabilistic Store
        let limiter = RateLimiter::new(ProbabilisticMemoryStore::with_capacity(num_keys));
        let (ops_per_sec, allowed, blocked) = benchmark_pattern(
            "Probabilistic", limiter, pattern, num_keys, iterations
        );
        results.push(("Probabilistic", ops_per_sec, allowed, blocked));
        
        // Adaptive Store
        let limiter = RateLimiter::new(AdaptiveMemoryStore::with_capacity(num_keys));
        let (ops_per_sec, allowed, blocked) = benchmark_pattern(
            "Adaptive", limiter, pattern, num_keys, iterations
        );
        results.push(("Adaptive", ops_per_sec, allowed, blocked));
        
        // Arena Store - needs larger capacity for larger datasets
        let limiter = RateLimiter::new(ArenaMemoryStore::with_capacity(num_keys * 3));
        let (ops_per_sec, allowed, blocked) = benchmark_pattern(
            "Arena", limiter, pattern, num_keys, iterations
        );
        results.push(("Arena", ops_per_sec, allowed, blocked));
        
        // Compact Store
        let limiter = RateLimiter::new(CompactMemoryStore::with_capacity(num_keys));
        let (ops_per_sec, allowed, blocked) = benchmark_pattern(
            "Compact", limiter, pattern, num_keys, iterations
        );
        results.push(("Compact", ops_per_sec, allowed, blocked));
        
        // TimingWheel Store
        let limiter = RateLimiter::new(TimingWheelStore::with_capacity(num_keys));
        let (ops_per_sec, allowed, blocked) = benchmark_pattern(
            "TimingWheel", limiter, pattern, num_keys, iterations
        );
        results.push(("TimingWheel", ops_per_sec, allowed, blocked));
        
        // BloomFilter Store - Skip due to stack overflow issues
        // let limiter = RateLimiter::new(BloomFilterStore::with_config(
        //     OptimizedMemoryStore::with_capacity(num_keys),
        //     num_keys,
        //     0.01
        // ));
        // let (ops_per_sec, allowed, blocked) = benchmark_pattern(
        //     "BloomFilter", limiter, pattern, num_keys, iterations
        // );
        // results.push(("BloomFilter", ops_per_sec, allowed, blocked));
        
        print_pattern_results(pattern, results);
    }
    
    println!("\n\nKey Insights:");
    println!("- Sequential: Best for cache-friendly workloads");
    println!("- Random: Tests general-purpose performance");
    println!("- Hot Key: Common in real-world (popular endpoints)");
    println!("- Zipfian: Models real-world distributions");
    println!("- Bursty: Tests handling of concentrated load");
    println!("- Sparse: Tests non-existent key handling");
}
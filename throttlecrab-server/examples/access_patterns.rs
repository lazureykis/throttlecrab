use std::time::{Instant, SystemTime};
use throttlecrab::{AdaptiveStore, PeriodicStore, ProbabilisticStore, RateLimiter};

#[derive(Clone, Copy)]
enum AccessPattern {
    Sequential,
    Random,
    HotKey,
    Zipfian,
}

impl AccessPattern {
    fn name(&self) -> &'static str {
        match self {
            AccessPattern::Sequential => "Sequential",
            AccessPattern::Random => "Random",
            AccessPattern::HotKey => "Hot Key (90/10)",
            AccessPattern::Zipfian => "Zipfian",
        }
    }

    fn generate_key(&self, iteration: usize, num_keys: usize) -> String {
        match self {
            AccessPattern::Sequential => {
                // Keys accessed in order
                format!("key_{}", iteration % num_keys)
            }
            AccessPattern::Random => {
                // Keys accessed randomly using prime multiplication
                let key_id = (iteration.wrapping_mul(2654435761)) % num_keys;
                format!("key_{key_id}")
            }
            AccessPattern::HotKey => {
                // 90% of requests go to 10% of keys
                if iteration % 10 < 9 {
                    let hot_key_id = (iteration / 10) % (num_keys / 10);
                    format!("hot_key_{hot_key_id}")
                } else {
                    let cold_key_id = (iteration.wrapping_mul(2654435761)) % num_keys;
                    format!("cold_key_{cold_key_id}")
                }
            }
            AccessPattern::Zipfian => {
                // Zipfian distribution - most requests go to a few keys
                let rank =
                    ((iteration as f64).ln() / (num_keys as f64).ln() * num_keys as f64) as usize;
                format!("key_{}", rank.min(num_keys - 1))
            }
        }
    }
}

fn benchmark_pattern<S: throttlecrab::Store>(
    store_name: &str,
    mut limiter: RateLimiter<S>,
    pattern: AccessPattern,
    num_keys: usize,
    iterations: usize,
) -> (u64, usize, usize) {
    let start = Instant::now();
    let mut allowed_count = 0;
    let mut blocked_count = 0;

    for i in 0..iterations {
        let key = pattern.generate_key(i, num_keys);
        let (allowed, _) = limiter
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
        "{:<20} {:<20} {:>12} ops/s | allowed: {:>7} | blocked: {:>7}",
        store_name,
        pattern.name(),
        ops_per_sec as u64,
        allowed_count,
        blocked_count
    );

    (ops_per_sec as u64, allowed_count, blocked_count)
}

fn main() {
    println!("ThrottleCrab Access Pattern Performance Comparison");
    println!("==================================================\n");

    println!("This example demonstrates how different access patterns affect");
    println!("the performance of various store implementations.\n");

    let num_keys = 5000;
    let iterations = 500_000;

    println!("Configuration:");
    println!("  - Number of unique keys: {num_keys}");
    println!("  - Total iterations: {iterations}");
    println!("  - Rate limit: 100 burst, 1000 requests per hour\n");

    println!(
        "{:<20} {:<20} {:>12} {:>10} {:>10}",
        "Store Type", "Access Pattern", "Throughput", "Allowed", "Blocked"
    );
    println!("{}", "-".repeat(80));

    let patterns = vec![
        AccessPattern::Sequential,
        AccessPattern::Random,
        AccessPattern::HotKey,
        AccessPattern::Zipfian,
    ];

    // Test each store with each pattern
    for pattern in &patterns {
        // Adaptive Store
        let adaptive = RateLimiter::new(AdaptiveStore::new());
        benchmark_pattern("Adaptive", adaptive, *pattern, num_keys, iterations);

        // Periodic Store
        let periodic = RateLimiter::new(PeriodicStore::new());
        benchmark_pattern("Periodic", periodic, *pattern, num_keys, iterations);

        // Probabilistic Store
        let probabilistic = RateLimiter::new(ProbabilisticStore::new());
        benchmark_pattern(
            "Probabilistic",
            probabilistic,
            *pattern,
            num_keys,
            iterations,
        );

        println!();
    }

    println!("\nKey Insights:");
    println!("- Sequential access benefits from cache locality");
    println!("- Hot key patterns show the importance of efficient key lookup");
    println!("- Random access tests the overall performance of the store");
    println!("- Zipfian distribution simulates real-world usage patterns");
}

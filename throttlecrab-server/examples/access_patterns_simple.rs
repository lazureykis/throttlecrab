use std::time::{Instant, SystemTime};
use throttlecrab::{AdaptiveStore, PeriodicStore, ProbabilisticStore, RateLimiter};

fn benchmark_store<S: throttlecrab::Store>(
    name: &str,
    mut limiter: RateLimiter<S>,
    _pattern: &str,
    test_fn: impl Fn(usize) -> String,
    num_keys: usize,
    iterations: usize,
) -> (u64, usize, usize) {
    let start = Instant::now();
    let mut allowed = 0;
    let mut blocked = 0;

    for i in 0..iterations {
        let key = test_fn(i % num_keys);
        let (is_allowed, _) = limiter
            .rate_limit(&key, 100, 1000, 3600, 1, SystemTime::now())
            .unwrap();

        if is_allowed {
            allowed += 1;
        } else {
            blocked += 1;
        }
    }

    let elapsed = start.elapsed();
    let ops_per_sec = iterations as f64 / elapsed.as_secs_f64();

    println!(
        "{:<20} {:>12} ops/s | allowed: {:>6} | blocked: {:>6}",
        name, ops_per_sec as u64, allowed, blocked
    );

    (ops_per_sec as u64, allowed, blocked)
}

fn main() {
    println!("ThrottleCrab Access Pattern Performance");
    println!("=======================================\n");

    let num_keys = 5000;
    let iterations = 500_000;

    // Test patterns
    let patterns = vec![
        (
            "Sequential",
            Box::new(|i: usize| format!("key_{i}")) as Box<dyn Fn(usize) -> String>,
        ),
        (
            "Random",
            Box::new(|i: usize| format!("key_{}", (i * 2654435761_usize) % 1000)),
        ),
        (
            "Hot Keys",
            Box::new(|i: usize| {
                if i % 5 < 4 {
                    format!("key_{}", i % 20) // 80% on 20 keys
                } else {
                    format!("key_{}", 20 + (i % 980)) // 20% on other keys
                }
            }),
        ),
        (
            "Sparse",
            Box::new(|i: usize| {
                if i % 10 == 0 {
                    format!("existing_{}", i % 100)
                } else {
                    format!("missing_{i}")
                }
            }),
        ),
    ];

    for (pattern_name, test_fn) in patterns {
        println!("\n{pattern_name} Access Pattern");
        println!("{}", "-".repeat(70));

        // Benchmark each store
        benchmark_store(
            "Periodic",
            RateLimiter::new(PeriodicStore::with_capacity(num_keys)),
            pattern_name,
            &*test_fn,
            num_keys,
            iterations,
        );

        benchmark_store(
            "Adaptive",
            RateLimiter::new(AdaptiveStore::with_capacity(num_keys)),
            pattern_name,
            &*test_fn,
            num_keys,
            iterations,
        );

        benchmark_store(
            "Probabilistic",
            RateLimiter::new(ProbabilisticStore::with_capacity(num_keys)),
            pattern_name,
            &*test_fn,
            num_keys,
            iterations,
        );
    }

    println!("\n\nKey Findings:");
    println!("- Sequential access benefits from cache locality");
    println!("- Random access tests general-purpose performance");
    println!("- Hot keys show real-world behavior (popular endpoints)");
    println!("- Sparse pattern tests non-existent key handling");
}

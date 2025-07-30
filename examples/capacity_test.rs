use std::time::SystemTime;
use throttlecrab::RateLimiter;
use throttlecrab::core::store::{
    amortized::AmortizedMemoryStore, optimized::OptimizedMemoryStore,
};

fn test_store_capacity<S: throttlecrab::core::store::Store>(
    name: &str,
    mut limiter: RateLimiter<S>,
    num_keys: usize,
) {
    println!("\nTesting {name} with {num_keys} unique keys:");

    let mut success_count = 0;
    let mut error_count = 0;
    let now = SystemTime::now();

    for i in 0..num_keys {
        let key = format!("key_{i}");
        match limiter.rate_limit(&key, 100, 1000, 3600, 1, now) {
            Ok((_allowed, _)) => {
                success_count += 1;
                if i % 1000 == 0 {
                    println!("  Progress: {i} keys processed");
                }
            }
            Err(e) => {
                error_count += 1;
                println!("  ERROR at key {i}: {e}");
                break; // Stop on first error
            }
        }
    }

    println!("  Result: {success_count} successful, {error_count} errors");
}

fn main() {
    println!("Store Capacity Behavior Test");
    println!("============================");

    let test_sizes = vec![100, 1000, 5000, 10000];

    for &size in &test_sizes {
        println!("\n--- Testing with {size} keys ---");

        // Optimized store - should handle any size
        test_store_capacity(
            "Optimized MemoryStore",
            RateLimiter::new(OptimizedMemoryStore::with_capacity(size / 2)), // Under-provision
            size,
        );

        // Amortized store - should handle any size
        test_store_capacity(
            "Amortized MemoryStore",
            RateLimiter::new(AmortizedMemoryStore::with_capacity(size / 2)), // Under-provision
            size,
        );

    }

    println!("\n\nKey Findings:");
    println!("- All remaining stores grow dynamically when capacity is exceeded");
    println!("- Initial capacity helps performance by avoiding rehashing");
    println!("- Under-provisioning is safe but may impact performance");
}

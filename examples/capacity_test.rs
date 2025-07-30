use std::time::SystemTime;
use throttlecrab::core::store::{
    arena::ArenaMemoryStore,
    optimized::OptimizedMemoryStore,
};
use throttlecrab::{MemoryStore, RateLimiter};

fn test_store_capacity<S: throttlecrab::core::store::Store>(
    name: &str,
    mut limiter: RateLimiter<S>,
    num_keys: usize,
) {
    println!("\nTesting {} with {} unique keys:", name, num_keys);
    
    let mut success_count = 0;
    let mut error_count = 0;
    let now = SystemTime::now();
    
    for i in 0..num_keys {
        let key = format!("key_{}", i);
        match limiter.rate_limit(&key, 100, 1000, 3600, 1, now) {
            Ok((allowed, _)) => {
                success_count += 1;
                if i % 1000 == 0 {
                    println!("  Progress: {} keys processed", i);
                }
            }
            Err(e) => {
                error_count += 1;
                println!("  ERROR at key {}: {}", i, e);
                break; // Stop on first error
            }
        }
    }
    
    println!("  Result: {} successful, {} errors", success_count, error_count);
}

fn main() {
    println!("Store Capacity Behavior Test");
    println!("============================");
    
    let test_sizes = vec![100, 1000, 5000, 10000];
    
    for &size in &test_sizes {
        println!("\n--- Testing with {} keys ---", size);
        
        // Standard store - should handle any size
        test_store_capacity(
            "Standard MemoryStore",
            RateLimiter::new(MemoryStore::new()),
            size,
        );
        
        // Optimized store - should handle any size
        test_store_capacity(
            "Optimized MemoryStore",
            RateLimiter::new(OptimizedMemoryStore::with_capacity(size / 2)), // Under-provision
            size,
        );
        
        // Arena store - will fail if capacity exceeded
        test_store_capacity(
            "Arena MemoryStore (capacity = size/2)",
            RateLimiter::new(ArenaMemoryStore::with_capacity(size / 2)), // Under-provision
            size,
        );
        
        // Arena store with adequate capacity
        test_store_capacity(
            "Arena MemoryStore (capacity = size*2)",
            RateLimiter::new(ArenaMemoryStore::with_capacity(size * 2)), // Over-provision
            size,
        );
    }
    
    println!("\n\nKey Findings:");
    println!("- Standard and Optimized stores grow dynamically");
    println!("- Arena store fails with 'Arena capacity exceeded' when full");
    println!("- Proper capacity planning is critical for Arena store");
}
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::hint::black_box;
use std::time::{Duration, SystemTime};
use throttlecrab::{MemoryStore, RateLimiter};
use throttlecrab::core::store::optimized::{OptimizedMemoryStore, InternedMemoryStore};
use throttlecrab::core::store::fast_hasher::{FastHashMemoryStore, SimpleHashMemoryStore};

mod ahash_store;
use ahash_store::AHashMemoryStore;

/// Benchmark all store implementations across different key counts
fn benchmark_store_shootout(c: &mut Criterion) {
    let mut group = c.benchmark_group("store_shootout");
    
    // Test with different numbers of unique keys
    for &num_keys in &[10u64, 100, 1_000, 10_000, 100_000] {
        group.throughput(Throughput::Elements(1));
        group.measurement_time(Duration::from_secs(10));
        
        // Standard MemoryStore
        group.bench_with_input(
            BenchmarkId::new("standard", num_keys),
            &num_keys,
            |b, &num_keys| {
                let mut limiter = RateLimiter::new(MemoryStore::new());
                let mut counter = 0u64;
                
                b.iter(|| {
                    let key = format!("key_{}", counter % num_keys);
                    counter += 1;
                    
                    let (allowed, _result) = limiter
                        .rate_limit(
                            black_box(&key),
                            black_box(100),
                            black_box(1000),
                            black_box(60),
                            black_box(1),
                            black_box(SystemTime::now()),
                        )
                        .unwrap();
                    
                    black_box(allowed)
                });
            },
        );
        
        // OptimizedMemoryStore
        group.bench_with_input(
            BenchmarkId::new("optimized", num_keys),
            &num_keys,
            |b, &num_keys| {
                let mut limiter = RateLimiter::new(OptimizedMemoryStore::with_capacity(num_keys as usize));
                let mut counter = 0u64;
                
                b.iter(|| {
                    let key = format!("key_{}", counter % num_keys);
                    counter += 1;
                    
                    let (allowed, _result) = limiter
                        .rate_limit(
                            black_box(&key),
                            black_box(100),
                            black_box(1000),
                            black_box(60),
                            black_box(1),
                            black_box(SystemTime::now()),
                        )
                        .unwrap();
                    
                    black_box(allowed)
                });
            },
        );
        
        // InternedMemoryStore
        group.bench_with_input(
            BenchmarkId::new("interned", num_keys),
            &num_keys,
            |b, &num_keys| {
                let mut limiter = RateLimiter::new(InternedMemoryStore::with_capacity(num_keys as usize));
                let mut counter = 0u64;
                
                b.iter(|| {
                    let key = format!("key_{}", counter % num_keys);
                    counter += 1;
                    
                    let (allowed, _result) = limiter
                        .rate_limit(
                            black_box(&key),
                            black_box(100),
                            black_box(1000),
                            black_box(60),
                            black_box(1),
                            black_box(SystemTime::now()),
                        )
                        .unwrap();
                    
                    black_box(allowed)
                });
            },
        );
        
        // FastHashMemoryStore
        group.bench_with_input(
            BenchmarkId::new("fast_hash", num_keys),
            &num_keys,
            |b, &num_keys| {
                let mut limiter = RateLimiter::new(FastHashMemoryStore::with_capacity(num_keys as usize));
                let mut counter = 0u64;
                
                b.iter(|| {
                    let key = format!("key_{}", counter % num_keys);
                    counter += 1;
                    
                    let (allowed, _result) = limiter
                        .rate_limit(
                            black_box(&key),
                            black_box(100),
                            black_box(1000),
                            black_box(60),
                            black_box(1),
                            black_box(SystemTime::now()),
                        )
                        .unwrap();
                    
                    black_box(allowed)
                });
            },
        );
        
        // SimpleHashMemoryStore
        group.bench_with_input(
            BenchmarkId::new("simple_hash", num_keys),
            &num_keys,
            |b, &num_keys| {
                let mut limiter = RateLimiter::new(SimpleHashMemoryStore::with_capacity(num_keys as usize));
                let mut counter = 0u64;
                
                b.iter(|| {
                    let key = format!("key_{}", counter % num_keys);
                    counter += 1;
                    
                    let (allowed, _result) = limiter
                        .rate_limit(
                            black_box(&key),
                            black_box(100),
                            black_box(1000),
                            black_box(60),
                            black_box(1),
                            black_box(SystemTime::now()),
                        )
                        .unwrap();
                    
                    black_box(allowed)
                });
            },
        );
        
        // AHashMemoryStore
        group.bench_with_input(
            BenchmarkId::new("ahash", num_keys),
            &num_keys,
            |b, &num_keys| {
                let mut limiter = RateLimiter::new(AHashMemoryStore::with_capacity(num_keys as usize));
                let mut counter = 0u64;
                
                b.iter(|| {
                    let key = format!("key_{}", counter % num_keys);
                    counter += 1;
                    
                    let (allowed, _result) = limiter
                        .rate_limit(
                            black_box(&key),
                            black_box(100),
                            black_box(1000),
                            black_box(60),
                            black_box(1),
                            black_box(SystemTime::now()),
                        )
                        .unwrap();
                    
                    black_box(allowed)
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark with different access patterns
fn benchmark_access_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("access_patterns");
    group.throughput(Throughput::Elements(1));
    
    // Sequential access pattern
    group.bench_function("sequential_standard", |b| {
        let mut limiter = RateLimiter::new(MemoryStore::new());
        let mut counter = 0u64;
        
        b.iter(|| {
            let key = format!("key_{}", counter);
            counter += 1;
            
            let (allowed, _result) = limiter
                .rate_limit(
                    black_box(&key),
                    black_box(100),
                    black_box(1000),
                    black_box(60),
                    black_box(1),
                    black_box(SystemTime::now()),
                )
                .unwrap();
            
            black_box(allowed)
        });
    });
    
    group.bench_function("sequential_ahash", |b| {
        let mut limiter = RateLimiter::new(AHashMemoryStore::with_capacity(100_000));
        let mut counter = 0u64;
        
        b.iter(|| {
            let key = format!("key_{}", counter);
            counter += 1;
            
            let (allowed, _result) = limiter
                .rate_limit(
                    black_box(&key),
                    black_box(100),
                    black_box(1000),
                    black_box(60),
                    black_box(1),
                    black_box(SystemTime::now()),
                )
                .unwrap();
            
            black_box(allowed)
        });
    });
    
    // Random access pattern
    group.bench_function("random_standard", |b| {
        let mut limiter = RateLimiter::new(MemoryStore::new());
        let mut counter = 0u64;
        
        b.iter(|| {
            // Simple pseudo-random using prime multiplication
            let key_id = (counter.wrapping_mul(2654435761)) % 10_000;
            let key = format!("key_{}", key_id);
            counter += 1;
            
            let (allowed, _result) = limiter
                .rate_limit(
                    black_box(&key),
                    black_box(100),
                    black_box(1000),
                    black_box(60),
                    black_box(1),
                    black_box(SystemTime::now()),
                )
                .unwrap();
            
            black_box(allowed)
        });
    });
    
    group.bench_function("random_ahash", |b| {
        let mut limiter = RateLimiter::new(AHashMemoryStore::with_capacity(10_000));
        let mut counter = 0u64;
        
        b.iter(|| {
            // Simple pseudo-random using prime multiplication
            let key_id = (counter.wrapping_mul(2654435761)) % 10_000;
            let key = format!("key_{}", key_id);
            counter += 1;
            
            let (allowed, _result) = limiter
                .rate_limit(
                    black_box(&key),
                    black_box(100),
                    black_box(1000),
                    black_box(60),
                    black_box(1),
                    black_box(SystemTime::now()),
                )
                .unwrap();
            
            black_box(allowed)
        });
    });
    
    // Hot key pattern (80% of requests go to 20% of keys)
    group.bench_function("hotkey_standard", |b| {
        let mut limiter = RateLimiter::new(MemoryStore::new());
        let mut counter = 0u64;
        
        b.iter(|| {
            let key_id = if counter % 5 == 0 {
                // 20% of requests are spread across all keys
                counter % 10_000
            } else {
                // 80% of requests go to first 2000 keys
                counter % 2_000
            };
            let key = format!("key_{}", key_id);
            counter += 1;
            
            let (allowed, _result) = limiter
                .rate_limit(
                    black_box(&key),
                    black_box(100),
                    black_box(1000),
                    black_box(60),
                    black_box(1),
                    black_box(SystemTime::now()),
                )
                .unwrap();
            
            black_box(allowed)
        });
    });
    
    group.bench_function("hotkey_ahash", |b| {
        let mut limiter = RateLimiter::new(AHashMemoryStore::with_capacity(10_000));
        let mut counter = 0u64;
        
        b.iter(|| {
            let key_id = if counter % 5 == 0 {
                // 20% of requests are spread across all keys
                counter % 10_000
            } else {
                // 80% of requests go to first 2000 keys
                counter % 2_000
            };
            let key = format!("key_{}", key_id);
            counter += 1;
            
            let (allowed, _result) = limiter
                .rate_limit(
                    black_box(&key),
                    black_box(100),
                    black_box(1000),
                    black_box(60),
                    black_box(1),
                    black_box(SystemTime::now()),
                )
                .unwrap();
            
            black_box(allowed)
        });
    });
    
    group.finish();
}

/// Benchmark memory usage patterns
fn benchmark_memory_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_patterns");
    group.throughput(Throughput::Elements(1));
    
    // Benchmark with short TTL (causes more cleanup)
    group.bench_function("short_ttl_standard", |b| {
        let mut limiter = RateLimiter::new(MemoryStore::new());
        let mut counter = 0u64;
        
        b.iter(|| {
            let key = format!("key_{}", counter % 1000);
            counter += 1;
            
            let (allowed, _result) = limiter
                .rate_limit(
                    black_box(&key),
                    black_box(10),
                    black_box(100),
                    black_box(1), // 1 second TTL
                    black_box(1),
                    black_box(SystemTime::now()),
                )
                .unwrap();
            
            black_box(allowed)
        });
    });
    
    group.bench_function("short_ttl_optimized", |b| {
        let mut limiter = RateLimiter::new(OptimizedMemoryStore::with_capacity(1000));
        let mut counter = 0u64;
        
        b.iter(|| {
            let key = format!("key_{}", counter % 1000);
            counter += 1;
            
            let (allowed, _result) = limiter
                .rate_limit(
                    black_box(&key),
                    black_box(10),
                    black_box(100),
                    black_box(1), // 1 second TTL
                    black_box(1),
                    black_box(SystemTime::now()),
                )
                .unwrap();
            
            black_box(allowed)
        });
    });
    
    // Benchmark with varying key lengths
    group.bench_function("long_keys_standard", |b| {
        let mut limiter = RateLimiter::new(MemoryStore::new());
        let mut counter = 0u64;
        
        b.iter(|| {
            let key = format!("very_long_key_prefix_that_simulates_real_world_usage_pattern_{}", counter % 1000);
            counter += 1;
            
            let (allowed, _result) = limiter
                .rate_limit(
                    black_box(&key),
                    black_box(100),
                    black_box(1000),
                    black_box(60),
                    black_box(1),
                    black_box(SystemTime::now()),
                )
                .unwrap();
            
            black_box(allowed)
        });
    });
    
    group.bench_function("long_keys_ahash", |b| {
        let mut limiter = RateLimiter::new(AHashMemoryStore::with_capacity(1000));
        let mut counter = 0u64;
        
        b.iter(|| {
            let key = format!("very_long_key_prefix_that_simulates_real_world_usage_pattern_{}", counter % 1000);
            counter += 1;
            
            let (allowed, _result) = limiter
                .rate_limit(
                    black_box(&key),
                    black_box(100),
                    black_box(1000),
                    black_box(60),
                    black_box(1),
                    black_box(SystemTime::now()),
                )
                .unwrap();
            
            black_box(allowed)
        });
    });
    
    group.finish();
}

criterion_group!(benches, benchmark_store_shootout, benchmark_access_patterns, benchmark_memory_patterns);
criterion_main!(benches);
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::hint::black_box;
use std::time::{Duration, SystemTime};
use throttlecrab::{MemoryStore, RateLimiter};
use throttlecrab::core::store::optimized::OptimizedMemoryStore;
use throttlecrab::core::store::adaptive_cleanup::AdaptiveMemoryStore;
use throttlecrab::core::store::amortized::{AmortizedMemoryStore, ProbabilisticMemoryStore};

/// Benchmark cleanup strategies with expired entries
fn benchmark_with_expiration(c: &mut Criterion) {
    let mut group = c.benchmark_group("cleanup_with_expiration");
    group.throughput(Throughput::Elements(1));
    
    // Test with 50% expired entries
    group.bench_function("standard_50pct_expired", |b| {
        let mut limiter = RateLimiter::new(MemoryStore::new());
        let start_time = SystemTime::now();
        let mut counter = 0u64;
        
        // Pre-populate with mixed TTLs
        for i in 0..10_000 {
            let key = format!("key_{}", i);
            let ttl = if i % 2 == 0 { 1 } else { 3600 }; // Half expire after 1 second
            limiter.rate_limit(&key, 100, 1000, ttl, 1, start_time).unwrap();
        }
        
        // Benchmark after entries have expired
        let bench_time = start_time + Duration::from_secs(2);
        
        b.iter(|| {
            let key = format!("bench_key_{}", counter % 1000);
            counter += 1;
            
            let (allowed, _result) = limiter
                .rate_limit(
                    black_box(&key),
                    black_box(100),
                    black_box(1000),
                    black_box(60),
                    black_box(1),
                    black_box(bench_time),
                )
                .unwrap();
            
            black_box(allowed)
        });
    });
    
    group.bench_function("optimized_50pct_expired", |b| {
        let mut limiter = RateLimiter::new(OptimizedMemoryStore::with_capacity(10_000));
        let start_time = SystemTime::now();
        let mut counter = 0u64;
        
        // Pre-populate with mixed TTLs
        for i in 0..10_000 {
            let key = format!("key_{}", i);
            let ttl = if i % 2 == 0 { 1 } else { 3600 };
            limiter.rate_limit(&key, 100, 1000, ttl, 1, start_time).unwrap();
        }
        
        let bench_time = start_time + Duration::from_secs(2);
        
        b.iter(|| {
            let key = format!("bench_key_{}", counter % 1000);
            counter += 1;
            
            let (allowed, _result) = limiter
                .rate_limit(
                    black_box(&key),
                    black_box(100),
                    black_box(1000),
                    black_box(60),
                    black_box(1),
                    black_box(bench_time),
                )
                .unwrap();
            
            black_box(allowed)
        });
    });
    
    group.bench_function("adaptive_50pct_expired", |b| {
        let mut limiter = RateLimiter::new(AdaptiveMemoryStore::with_capacity(10_000));
        let start_time = SystemTime::now();
        let mut counter = 0u64;
        
        // Pre-populate with mixed TTLs
        for i in 0..10_000 {
            let key = format!("key_{}", i);
            let ttl = if i % 2 == 0 { 1 } else { 3600 };
            limiter.rate_limit(&key, 100, 1000, ttl, 1, start_time).unwrap();
        }
        
        let bench_time = start_time + Duration::from_secs(2);
        
        b.iter(|| {
            let key = format!("bench_key_{}", counter % 1000);
            counter += 1;
            
            let (allowed, _result) = limiter
                .rate_limit(
                    black_box(&key),
                    black_box(100),
                    black_box(1000),
                    black_box(60),
                    black_box(1),
                    black_box(bench_time),
                )
                .unwrap();
            
            black_box(allowed)
        });
    });
    
    group.bench_function("amortized_50pct_expired", |b| {
        let mut limiter = RateLimiter::new(AmortizedMemoryStore::with_capacity(10_000));
        let start_time = SystemTime::now();
        let mut counter = 0u64;
        
        // Pre-populate with mixed TTLs
        for i in 0..10_000 {
            let key = format!("key_{}", i);
            let ttl = if i % 2 == 0 { 1 } else { 3600 };
            limiter.rate_limit(&key, 100, 1000, ttl, 1, start_time).unwrap();
        }
        
        let bench_time = start_time + Duration::from_secs(2);
        
        b.iter(|| {
            let key = format!("bench_key_{}", counter % 1000);
            counter += 1;
            
            let (allowed, _result) = limiter
                .rate_limit(
                    black_box(&key),
                    black_box(100),
                    black_box(1000),
                    black_box(60),
                    black_box(1),
                    black_box(bench_time),
                )
                .unwrap();
            
            black_box(allowed)
        });
    });
    
    group.bench_function("probabilistic_50pct_expired", |b| {
        let mut limiter = RateLimiter::new(ProbabilisticMemoryStore::with_capacity(10_000));
        let start_time = SystemTime::now();
        let mut counter = 0u64;
        
        // Pre-populate with mixed TTLs
        for i in 0..10_000 {
            let key = format!("key_{}", i);
            let ttl = if i % 2 == 0 { 1 } else { 3600 };
            limiter.rate_limit(&key, 100, 1000, ttl, 1, start_time).unwrap();
        }
        
        let bench_time = start_time + Duration::from_secs(2);
        
        b.iter(|| {
            let key = format!("bench_key_{}", counter % 1000);
            counter += 1;
            
            let (allowed, _result) = limiter
                .rate_limit(
                    black_box(&key),
                    black_box(100),
                    black_box(1000),
                    black_box(60),
                    black_box(1),
                    black_box(bench_time),
                )
                .unwrap();
            
            black_box(allowed)
        });
    });
    
    group.finish();
}

/// Benchmark latency distribution
fn benchmark_latency_percentiles(c: &mut Criterion) {
    let mut group = c.benchmark_group("cleanup_latency_p99");
    group.throughput(Throughput::Elements(1));
    group.sample_size(1000); // More samples for percentile accuracy
    
    // Standard cleanup - expect periodic spikes
    group.bench_function("standard_cleanup_spikes", |b| {
        let mut limiter = RateLimiter::new(MemoryStore::new());
        let mut counter = 0u64;
        
        b.iter(|| {
            let key = format!("key_{}", counter);
            counter += 1;
            
            // Add with short TTL to force cleanups
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
    
    // Amortized - should have consistent latency
    group.bench_function("amortized_consistent", |b| {
        let mut limiter = RateLimiter::new(AmortizedMemoryStore::new());
        let mut counter = 0u64;
        
        b.iter(|| {
            let key = format!("key_{}", counter);
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
    
    group.finish();
}

criterion_group!(benches, benchmark_with_expiration, benchmark_latency_percentiles);
criterion_main!(benches);
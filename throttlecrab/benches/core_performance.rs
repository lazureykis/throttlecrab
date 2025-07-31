use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use std::time::{Duration, SystemTime};
use throttlecrab::{PeriodicStore, RateLimiter};

mod ahash_store;
use ahash_store::AHashStore;

fn benchmark_core_rate_limiter(c: &mut Criterion) {
    let mut group = c.benchmark_group("core_rate_limiter");
    group.throughput(Throughput::Elements(1));
    group.measurement_time(Duration::from_secs(10));

    // Test with a simple configuration
    group.bench_function("single_key_allowed", |b| {
        let mut limiter = RateLimiter::new(PeriodicStore::new());
        let mut counter = 0u64;

        b.iter(|| {
            let key = "test_key";
            counter += 1;

            let (allowed, _result) = limiter
                .rate_limit(
                    black_box(key),
                    black_box(1000),  // max_burst
                    black_box(10000), // count_per_period
                    black_box(60),    // period
                    black_box(1),     // quantity
                    black_box(SystemTime::now()),
                )
                .unwrap();

            black_box(allowed)
        });
    });

    // Test with multiple keys to simulate real-world usage
    group.bench_function("rotating_keys_100", |b| {
        let mut limiter = RateLimiter::new(PeriodicStore::new());
        let mut counter = 0u64;

        b.iter(|| {
            let key = format!("key_{}", counter % 100);
            counter += 1;

            let (allowed, _result) = limiter
                .rate_limit(
                    black_box(&key),
                    black_box(100),  // max_burst
                    black_box(1000), // count_per_period
                    black_box(60),   // period
                    black_box(1),    // quantity
                    black_box(SystemTime::now()),
                )
                .unwrap();

            black_box(allowed)
        });
    });

    // Test when rate limit is exceeded (worst case)
    group.bench_function("single_key_denied", |b| {
        let mut limiter = RateLimiter::new(PeriodicStore::new());

        // Exhaust the rate limit first
        let key = "exhausted_key";
        for _ in 0..10 {
            limiter
                .rate_limit(key, 5, 10, 60, 1, SystemTime::now())
                .unwrap();
        }

        b.iter(|| {
            let (allowed, _result) = limiter
                .rate_limit(
                    black_box(key),
                    black_box(5),  // max_burst
                    black_box(10), // count_per_period
                    black_box(60), // period
                    black_box(1),  // quantity
                    black_box(SystemTime::now()),
                )
                .unwrap();

            black_box(allowed)
        });
    });

    // Test with high burst values
    group.bench_function("high_burst_single_key", |b| {
        let mut limiter = RateLimiter::new(PeriodicStore::new());
        let mut counter = 0u64;

        b.iter(|| {
            let key = "high_burst_key";
            counter += 1;

            let (allowed, _result) = limiter
                .rate_limit(
                    black_box(key),
                    black_box(100000),  // max_burst
                    black_box(1000000), // count_per_period
                    black_box(60),      // period
                    black_box(1),       // quantity
                    black_box(SystemTime::now()),
                )
                .unwrap();

            black_box(allowed)
        });
    });

    // Test with varying quantities
    group.bench_function("varying_quantities", |b| {
        let mut limiter = RateLimiter::new(PeriodicStore::new());
        let mut counter = 0u64;

        b.iter(|| {
            let key = "quantity_key";
            counter += 1;
            let quantity = (counter % 10) + 1;

            let (allowed, _result) = limiter
                .rate_limit(
                    black_box(key),
                    black_box(1000),  // max_burst
                    black_box(10000), // count_per_period
                    black_box(60),    // period
                    black_box(quantity as i64),
                    black_box(SystemTime::now()),
                )
                .unwrap();

            black_box(allowed)
        });
    });

    group.finish();
}

fn benchmark_memory_store_growth(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_store_growth");
    group.throughput(Throughput::Elements(1));

    // Test with growing number of unique keys
    for num_keys in [10, 100, 1000, 10000] {
        group.bench_with_input(
            format!("unique_keys_{num_keys}"),
            &num_keys,
            |b, &num_keys| {
                let mut limiter = RateLimiter::new(PeriodicStore::new());
                let mut counter = 0u64;

                b.iter(|| {
                    let key = format!("key_{}", counter % num_keys);
                    counter += 1;

                    let (allowed, _result) = limiter
                        .rate_limit(
                            black_box(&key),
                            black_box(100),  // max_burst
                            black_box(1000), // count_per_period
                            black_box(60),   // period
                            black_box(1),    // quantity
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

fn benchmark_store_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("store_comparison");
    group.throughput(Throughput::Elements(1));

    // Test with 10,000 unique keys - where optimization matters most
    let num_keys = 10000u64;

    group.bench_function("standard_memory_store", |b| {
        let mut limiter = RateLimiter::new(PeriodicStore::new());
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
    });

    group.bench_function("ahash_memory_store", |b| {
        let mut limiter = RateLimiter::new(AHashStore::with_capacity(num_keys as usize));
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
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_core_rate_limiter,
    benchmark_memory_store_growth,
    benchmark_store_comparison
);
criterion_main!(benches);

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use throttlecrab::{AdaptiveStore, PeriodicStore, ProbabilisticStore, RateLimiter};

fn benchmark_store_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("store_operations");
    group.measurement_time(Duration::from_secs(1));
    group.warm_up_time(Duration::from_millis(100));

    // Test different operations
    let store = Arc::new(parking_lot::Mutex::new(RateLimiter::new(
        PeriodicStore::new(),
    )));

    // Benchmark single throttle operation
    group.bench_function("single_throttle", |b| {
        let mut counter = 0u64;
        b.iter(|| {
            let key = format!("bench_key_{counter}");
            counter += 1;
            let mut limiter = store.lock();
            let result = limiter.rate_limit(
                black_box(&key),
                black_box(100),  // max_burst
                black_box(1000), // count_per_period
                black_box(60),   // period
                black_box(1),    // quantity
                black_box(SystemTime::now()),
            );
            let _ = black_box(result);
        });
    });

    // Benchmark with hot keys (repeated access to same keys)
    group.bench_function("hot_keys", |b| {
        let keys: Vec<String> = (0..100).map(|i| format!("hot_key_{i}")).collect();
        let mut idx = 0;
        b.iter(|| {
            let key = &keys[idx % keys.len()];
            idx += 1;
            let mut limiter = store.lock();
            let result = limiter.rate_limit(
                black_box(key),
                black_box(100),
                black_box(1000),
                black_box(60),
                black_box(1),
                black_box(SystemTime::now()),
            );
            let _ = black_box(result);
        });
    });

    // Benchmark with cold keys (always new keys)
    group.bench_function("cold_keys", |b| {
        let mut counter = 0u64;
        b.iter(|| {
            let key = format!("cold_key_{counter}");
            counter += 1;
            let mut limiter = store.lock();
            let result = limiter.rate_limit(
                black_box(&key),
                black_box(100),
                black_box(1000),
                black_box(60),
                black_box(1),
                black_box(SystemTime::now()),
            );
            let _ = black_box(result);
        });
    });

    group.finish();
}

fn benchmark_concurrent_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_access");
    group.measurement_time(Duration::from_secs(1));
    group.warm_up_time(Duration::from_millis(100));

    let store = Arc::new(parking_lot::Mutex::new(RateLimiter::new(
        PeriodicStore::new(),
    )));

    for num_threads in [1, 2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("threads_{num_threads}")),
            num_threads,
            |b, &num_threads| {
                let runtime = tokio::runtime::Runtime::new().unwrap();
                let mut counter = 0u64;

                b.iter(|| {
                    runtime.block_on(async {
                        let mut handles = vec![];

                        for thread_id in 0..num_threads {
                            let store = store.clone();
                            let key = format!("concurrent_key_{thread_id}_{counter}");
                            counter += 1;

                            let handle = tokio::spawn(async move {
                                let mut limiter = store.lock();
                                let result =
                                    limiter.rate_limit(&key, 100, 1000, 60, 1, SystemTime::now());
                                let _ = black_box(result);
                            });

                            handles.push(handle);
                        }

                        for handle in handles {
                            handle.await.unwrap();
                        }
                    });
                });
            },
        );
    }

    group.finish();
}

fn benchmark_memory_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_patterns");
    group.measurement_time(Duration::from_secs(1));
    group.warm_up_time(Duration::from_millis(100));

    // Zipfian distribution (hotspot pattern)
    group.bench_function("zipfian_distribution", |b| {
        let store = Arc::new(parking_lot::Mutex::new(RateLimiter::new(
            PeriodicStore::new(),
        )));
        let keys: Vec<String> = (0..1000).map(|i| format!("zipf_key_{i}")).collect();
        let mut rng = fastrand::Rng::new();

        b.iter(|| {
            // Simple zipfian-like distribution: lower indices more likely
            let idx = (rng.f64().powf(2.0) * keys.len() as f64) as usize;
            let key = &keys[idx.min(keys.len() - 1)];

            let mut limiter = store.lock();
            let result = limiter.rate_limit(
                black_box(key),
                black_box(100),
                black_box(1000),
                black_box(60),
                black_box(1),
                black_box(SystemTime::now()),
            );
            let _ = black_box(result);
        });
    });

    // Sequential access pattern
    group.bench_function("sequential_access", |b| {
        let store = Arc::new(parking_lot::Mutex::new(RateLimiter::new(
            PeriodicStore::new(),
        )));
        let mut counter = 0u64;

        b.iter(|| {
            let key = format!("seq_key_{}", counter % 1000);
            counter += 1;

            let mut limiter = store.lock();
            let result = limiter.rate_limit(
                black_box(&key),
                black_box(100),
                black_box(1000),
                black_box(60),
                black_box(1),
                black_box(SystemTime::now()),
            );
            let _ = black_box(result);
        });
    });

    group.finish();
}

fn benchmark_store_types(c: &mut Criterion) {
    let mut group = c.benchmark_group("store_type_comparison");
    group.measurement_time(Duration::from_secs(1));
    group.warm_up_time(Duration::from_millis(100));

    // Compare different store types with mixed workload
    let workload_keys: Vec<String> = (0..1000).map(|i| format!("workload_key_{i}")).collect();

    group.bench_function("periodic_store", |b| {
        let store = Arc::new(parking_lot::Mutex::new(RateLimiter::new(
            PeriodicStore::new(),
        )));
        let mut idx = 0;

        b.iter(|| {
            let key = &workload_keys[idx % workload_keys.len()];
            idx += 1;
            let mut limiter = store.lock();
            let result = limiter.rate_limit(
                black_box(key),
                black_box(100),
                black_box(1000),
                black_box(60),
                black_box(1),
                black_box(SystemTime::now()),
            );
            let _ = black_box(result);
        });
    });

    group.bench_function("probabilistic_store", |b| {
        let store = Arc::new(parking_lot::Mutex::new(RateLimiter::new(
            ProbabilisticStore::new(),
        )));
        let mut idx = 0;

        b.iter(|| {
            let key = &workload_keys[idx % workload_keys.len()];
            idx += 1;
            let mut limiter = store.lock();
            let result = limiter.rate_limit(
                black_box(key),
                black_box(100),
                black_box(1000),
                black_box(60),
                black_box(1),
                black_box(SystemTime::now()),
            );
            let _ = black_box(result);
        });
    });

    group.bench_function("adaptive_store", |b| {
        let store = Arc::new(parking_lot::Mutex::new(RateLimiter::new(
            AdaptiveStore::new(),
        )));
        let mut idx = 0;

        b.iter(|| {
            let key = &workload_keys[idx % workload_keys.len()];
            idx += 1;
            let mut limiter = store.lock();
            let result = limiter.rate_limit(
                black_box(key),
                black_box(100),
                black_box(1000),
                black_box(60),
                black_box(1),
                black_box(SystemTime::now()),
            );
            let _ = black_box(result);
        });
    });

    group.finish();
}

fn benchmark_workload_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("workload_patterns");
    group.measurement_time(Duration::from_secs(1));
    group.warm_up_time(Duration::from_millis(100));

    // Test with different request rates
    for request_rate in [100, 1000, 10_000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("rps_{request_rate}")),
            &request_rate,
            |b, &request_rate| {
                let store = Arc::new(parking_lot::Mutex::new(RateLimiter::new(
                    PeriodicStore::new(),
                )));
                let keys: Vec<String> = (0..100).map(|i| format!("workload_key_{i}")).collect();
                let mut idx = 0;

                b.iter(|| {
                    let key = &keys[idx % keys.len()];
                    idx += 1;

                    let mut limiter = store.lock();
                    let result = limiter.rate_limit(
                        black_box(key),
                        black_box(100),
                        black_box(request_rate),
                        black_box(60),
                        black_box(1),
                        black_box(SystemTime::now()),
                    );
                    let _ = black_box(result);
                });
            },
        );
    }

    // Test with burst patterns
    group.bench_function("burst_pattern", |b| {
        let store = Arc::new(parking_lot::Mutex::new(RateLimiter::new(
            PeriodicStore::new(),
        )));
        let mut counter = 0u64;

        b.iter(|| {
            let burst_size = if counter % 100 < 10 { 10 } else { 1 };
            let key = format!("burst_key_{}", counter / 100);
            counter += 1;

            let mut limiter = store.lock();
            let result = limiter.rate_limit(
                black_box(&key),
                black_box(100),
                black_box(1000),
                black_box(60),
                black_box(burst_size),
                black_box(SystemTime::now()),
            );
            let _ = black_box(result);
        });
    });

    group.finish();
}

fn benchmark_high_cardinality(c: &mut Criterion) {
    let mut group = c.benchmark_group("high_cardinality");
    group.measurement_time(Duration::from_secs(2));
    group.warm_up_time(Duration::from_millis(200));
    group.sample_size(10);

    // Test with increasing number of unique keys
    for num_keys in [1_000, 10_000, 100_000] {
        group.throughput(Throughput::Elements(num_keys as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("keys_{num_keys}")),
            &num_keys,
            |b, &num_keys| {
                let store = Arc::new(parking_lot::Mutex::new(RateLimiter::new(
                    PeriodicStore::new(),
                )));

                b.iter(|| {
                    // Fill store with unique keys
                    for i in 0..num_keys {
                        let key = format!("high_card_key_{i}");
                        let mut limiter = store.lock();
                        let _ = limiter.rate_limit(&key, 100, 1000, 60, 1, SystemTime::now());
                    }
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    benchmark_store_operations,
    benchmark_concurrent_access,
    benchmark_memory_patterns,
    benchmark_store_types,
    benchmark_workload_patterns,
    benchmark_high_cardinality
);
criterion_main!(benches);

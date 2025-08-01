use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;

fn benchmark_connection_pool_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("connection_pool_sizes");
    group.measurement_time(Duration::from_secs(10));

    let runtime = Arc::new(Runtime::new().unwrap());
    let url = "http://127.0.0.1:9091/throttle";

    // Test different connection pool sizes
    for pool_size in [1, 5, 10, 20, 50].iter() {
        group.throughput(Throughput::Elements(100)); // 100 requests per iteration
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("pool_{pool_size}")),
            pool_size,
            |b, &pool_size| {
                let client = runtime.block_on(async {
                    reqwest::Client::builder()
                        .pool_max_idle_per_host(pool_size)
                        .pool_idle_timeout(None)
                        .build()
                        .unwrap()
                });

                let mut counter = 0u64;

                b.iter(|| {
                    runtime.block_on(async {
                        let mut tasks = Vec::with_capacity(100);

                        for _ in 0..100 {
                            let key = format!("bench_key_{counter}");
                            counter += 1;

                            let client = client.clone();
                            let url = url.to_string();

                            let task = tokio::spawn(async move {
                                let resp = client
                                    .post(&url)
                                    .json(&serde_json::json!({
                                        "key": key,
                                        "max_burst": 100,
                                        "count_per_period": 10000,
                                        "period": 60,
                                        "quantity": 1
                                    }))
                                    .send()
                                    .await
                                    .unwrap();

                                let json: serde_json::Value = resp.json().await.unwrap();
                                json["allowed"].as_bool().unwrap()
                            });

                            tasks.push(task);
                        }

                        // Wait for all requests to complete
                        for task in tasks {
                            task.await.unwrap();
                        }
                    });
                });
            },
        );
    }

    group.finish();
}

fn benchmark_concurrent_connections(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_connections");
    group.measurement_time(Duration::from_secs(10));

    let runtime = Arc::new(Runtime::new().unwrap());
    let url = "http://127.0.0.1:9091/throttle";

    // Test different concurrency levels
    for concurrency in [1, 10, 50, 100, 200].iter() {
        group.throughput(Throughput::Elements(*concurrency as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("concurrent_{concurrency}")),
            concurrency,
            |b, &concurrency| {
                let client = runtime.block_on(async {
                    reqwest::Client::builder()
                        .pool_max_idle_per_host(concurrency)
                        .build()
                        .unwrap()
                });

                let mut counter = 0u64;

                b.iter(|| {
                    runtime.block_on(async {
                        let mut tasks = Vec::with_capacity(concurrency);

                        for _ in 0..concurrency {
                            let key = format!("bench_key_{counter}");
                            counter += 1;

                            let client = client.clone();
                            let url = url.to_string();

                            let task = tokio::spawn(async move {
                                let resp = client
                                    .post(&url)
                                    .json(&serde_json::json!({
                                        "key": key,
                                        "max_burst": 100,
                                        "count_per_period": 10000,
                                        "period": 60,
                                        "quantity": 1
                                    }))
                                    .send()
                                    .await
                                    .unwrap();

                                let json: serde_json::Value = resp.json().await.unwrap();
                                json["allowed"].as_bool().unwrap()
                            });

                            tasks.push(task);
                        }

                        // Wait for all requests to complete
                        for task in tasks {
                            task.await.unwrap();
                        }
                    });
                });
            },
        );
    }

    group.finish();
}

fn connection_pool_benchmarks(c: &mut Criterion) {
    println!("Make sure server is running with: cargo run --release -- --http --http-port 9091");
    println!("Waiting for server to start...");
    std::thread::sleep(Duration::from_secs(2));

    // Test connection
    let runtime = Runtime::new().unwrap();
    let check_result =
        runtime.block_on(async { reqwest::get("http://127.0.0.1:9091/health").await });
    match check_result {
        Ok(_) => println!("Connected to HTTP server on port 9091"),
        Err(e) => {
            eprintln!("\n❌ ERROR: Server is not running on port 9091");
            eprintln!("   Error: {e}");
            eprintln!("\n📝 To run benchmarks, you need to start the server first:");
            eprintln!("\n   Option 1 - All transports (recommended):");
            eprintln!("   cargo run --release -- --http --http-port 9091 --grpc --grpc-port 9093 --redis --redis-port 9092");
            eprintln!("\n   Option 2 - HTTP only:");
            eprintln!("   cargo run --release -- --http --http-port 9091");
            eprintln!("\n   Then in another terminal, run:");
            eprintln!("   cargo bench");
            eprintln!("\n⚠️  Note: The server must be running for benchmarks to work.");
            std::process::exit(1);
        }
    }

    benchmark_connection_pool_sizes(c);
    benchmark_concurrent_connections(c);
}

criterion_group!(benches, connection_pool_benchmarks);
criterion_main!(benches);

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::time::Duration;
use tokio::runtime::Runtime;

fn benchmark_http_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("http_throughput");
    group.measurement_time(Duration::from_secs(10));

    // Create runtime and client once
    let runtime = Runtime::new().unwrap();
    let client = runtime.block_on(async {
        reqwest::Client::builder()
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(None)
            .build()
            .unwrap()
    });

    let url = "http://127.0.0.1:9091/throttle";
    let mut counter = 0u64;

    // Test different request batch sizes
    for batch_size in [1, 10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &batch_size| {
                b.iter(|| {
                    runtime.block_on(async {
                        let mut tasks = Vec::with_capacity(batch_size);

                        for _ in 0..batch_size {
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

fn benchmark_http_connection_reuse(c: &mut Criterion) {
    let mut group = c.benchmark_group("http_connection_reuse");
    group.throughput(Throughput::Elements(1));
    group.measurement_time(Duration::from_secs(10));

    let runtime = Runtime::new().unwrap();
    let url = "http://127.0.0.1:9091/throttle";
    let mut counter = 0u64;

    // Benchmark with connection pooling
    group.bench_function("with_pooling", |b| {
        let client = runtime.block_on(async {
            reqwest::Client::builder()
                .pool_max_idle_per_host(10)
                .build()
                .unwrap()
        });

        b.iter(|| {
            let key = format!("bench_key_{counter}");
            counter += 1;

            runtime.block_on(async {
                let resp = client
                    .post(url)
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
            })
        });
    });

    // Benchmark without connection pooling
    group.bench_function("without_pooling", |b| {
        b.iter(|| {
            let key = format!("bench_key_{counter}");
            counter += 1;

            runtime.block_on(async {
                let client = reqwest::Client::new();
                let resp = client
                    .post(url)
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
            })
        });
    });

    group.finish();
}

fn http_throughput_benchmarks(c: &mut Criterion) {
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

    benchmark_http_throughput(c);
    benchmark_http_connection_reuse(c);
}

criterion_group!(benches, http_throughput_benchmarks);
criterion_main!(benches);

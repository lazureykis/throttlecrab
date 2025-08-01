use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;

fn benchmark_shared_connection_pool(c: &mut Criterion) {
    let mut group = c.benchmark_group("shared_connection_pool");
    group.measurement_time(Duration::from_secs(1));
    group.warm_up_time(Duration::from_millis(100));

    let runtime = Runtime::new().unwrap();

    // Create a single shared client with a large connection pool
    let client = Arc::new(
        reqwest::Client::builder()
            .pool_max_idle_per_host(100)
            .pool_idle_timeout(None)
            .build()
            .unwrap(),
    );

    let url = "http://127.0.0.1:9091/throttle";

    // Test with different numbers of concurrent threads
    for num_threads in [1, 2, 4, 8, 16, 32] {
        group.throughput(Throughput::Elements(num_threads as u64));
        group.bench_function(&format!("threads_{}", num_threads), |b| {
            let mut counter = 0u64;

            b.iter(|| {
                runtime.block_on(async {
                    let mut handles = vec![];

                    for _ in 0..num_threads {
                        let client = client.clone();
                        let url = url.to_string();
                        let key = format!("bench_key_{}", counter);
                        counter += 1;

                        let handle = tokio::spawn(async move {
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

                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.await.unwrap();
                    }
                });
            });
        });
    }

    group.finish();
}

fn connection_pool_benchmarks(c: &mut Criterion) {
    println!("Make sure server is running with: cargo run --release -- --http --http-port 9091");

    // Test connection
    let runtime = Runtime::new().unwrap();
    let check_result =
        runtime.block_on(async { reqwest::get("http://127.0.0.1:9091/health").await });

    match check_result {
        Ok(_) => println!("Connected to HTTP server on port 9091"),
        Err(e) => {
            eprintln!("\n❌ ERROR: Server is not running on port 9091");
            eprintln!("   Error: {e}");
            eprintln!("\n📝 To run benchmarks, start the server:");
            eprintln!("   cargo run --release -- --http --http-port 9091");
            eprintln!("\n   Then in another terminal, run:");
            eprintln!("   cargo bench connection_pool");
            std::process::exit(1);
        }
    }

    benchmark_shared_connection_pool(c);
}

criterion_group!(benches, connection_pool_benchmarks);
criterion_main!(benches);

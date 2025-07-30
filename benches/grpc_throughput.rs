use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::runtime::Runtime;
use tokio::sync::Semaphore;

// Include the generated protobuf code
pub mod throttlecrab {
    tonic::include_proto!("throttlecrab");
}

use throttlecrab::ThrottleRequest;
use throttlecrab::rate_limiter_client::RateLimiterClient;

async fn make_grpc_request(
    client: &mut RateLimiterClient<tonic::transport::Channel>,
    key: &str,
) -> Result<bool, tonic::Status> {
    let now = SystemTime::now();
    let duration = now.duration_since(UNIX_EPOCH).unwrap();

    let request = tonic::Request::new(ThrottleRequest {
        key: key.to_string(),
        max_burst: 1000,
        count_per_period: 10000,
        period: 60,
        quantity: 1,
        timestamp_secs: duration.as_secs() as i64,
        timestamp_nanos: duration.subsec_nanos() as i32,
    });

    let response = client.throttle(request).await?;
    Ok(response.into_inner().allowed)
}

fn grpc_throughput(c: &mut Criterion) {
    println!("Make sure to run the gRPC server:");
    println!("  cargo run --features bin -- --server --port 9093 --grpc");
    println!("Waiting for server to start...");
    std::thread::sleep(Duration::from_secs(2));

    let mut group = c.benchmark_group("grpc_throughput");
    group.measurement_time(Duration::from_secs(10));
    group.warm_up_time(Duration::from_secs(2));

    let runtime = Runtime::new().unwrap();

    // Test connection
    match runtime.block_on(RateLimiterClient::connect("http://127.0.0.1:9093")) {
        Ok(_) => println!("Connected to gRPC server"),
        Err(e) => {
            eprintln!("Failed to connect to gRPC server: {e}");
            eprintln!(
                "Please start the server with: cargo run --features bin -- --server --port 9093 --grpc"
            );
            return;
        }
    }

    // Single client sequential requests
    group.throughput(Throughput::Elements(1));
    group.bench_function("sequential", |b| {
        let client = runtime.block_on(async {
            RateLimiterClient::connect("http://127.0.0.1:9093")
                .await
                .unwrap()
        });
        let mut client = client;
        let mut counter = 0u64;

        b.iter(|| {
            runtime.block_on(async {
                let key = format!("bench_key_{counter}");
                counter += 1;
                make_grpc_request(&mut client, &key).await.unwrap()
            })
        });
    });

    // Concurrent requests with different client counts
    for num_clients in [1, 10, 50, 100] {
        group.throughput(Throughput::Elements(num_clients as u64));
        group.bench_with_input(
            BenchmarkId::new("concurrent_clients", num_clients),
            &num_clients,
            |b, &num_clients| {
                b.to_async(&runtime).iter(|| async move {
                    let semaphore = Arc::new(Semaphore::new(num_clients));
                    let mut handles = vec![];

                    for i in 0..num_clients {
                        let permit = semaphore.clone().acquire_owned().await.unwrap();
                        let handle = tokio::spawn(async move {
                            let mut client = RateLimiterClient::connect("http://127.0.0.1:9093")
                                .await
                                .unwrap();
                            let key = format!("bench_key_client_{i}");
                            let result = make_grpc_request(&mut client, &key).await.unwrap();
                            drop(permit);
                            result
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.await.unwrap();
                    }
                });
            },
        );
    }

    // Batch requests on single connection
    for batch_size in [10, 100, 1000] {
        group.throughput(Throughput::Elements(batch_size as u64));
        group.bench_with_input(
            BenchmarkId::new("batch_requests", batch_size),
            &batch_size,
            |b, &batch_size| {
                let client = runtime.block_on(async {
                    RateLimiterClient::connect("http://127.0.0.1:9093")
                        .await
                        .unwrap()
                });
                let mut client = client;
                let mut counter = 0u64;

                b.iter(|| {
                    runtime.block_on(async {
                        for _ in 0..batch_size {
                            let key = format!("bench_key_{counter}");
                            counter += 1;
                            make_grpc_request(&mut client, &key).await.unwrap();
                        }
                    });
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, grpc_throughput);
criterion_main!(benches);

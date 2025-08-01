use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use std::time::Duration;
use tokio::runtime::Runtime;

fn benchmark_http_protocol(c: &mut Criterion, port: u16) {
    let mut group = c.benchmark_group("protocol_http");
    group.throughput(Throughput::Elements(1));
    group.measurement_time(Duration::from_secs(10));

    let runtime = Runtime::new().unwrap();
    let client = runtime.block_on(async {
        reqwest::Client::builder()
            .pool_max_idle_per_host(1)
            .build()
            .unwrap()
    });

    let url = format!("http://127.0.0.1:{port}/throttle");
    let mut counter = 0u64;

    group.bench_function("single_request", |b| {
        b.iter(|| {
            let key = format!("bench_key_{counter}");
            counter += 1;

            runtime.block_on(async {
                let resp = client
                    .post(&url)
                    .json(&serde_json::json!({
                        "key": key,
                        "max_burst": 100,
                        "count_per_period": 1000,
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

fn benchmark_grpc_protocol(c: &mut Criterion, port: u16) {
    use throttlecrab_server::grpc::ThrottleRequest;
    use throttlecrab_server::grpc::rate_limiter_client::RateLimiterClient;

    let mut group = c.benchmark_group("protocol_grpc");
    group.throughput(Throughput::Elements(1));
    group.measurement_time(Duration::from_secs(10));

    let runtime = Runtime::new().unwrap();
    let mut counter = 0u64;

    let mut client = runtime.block_on(async {
        RateLimiterClient::connect(format!("http://127.0.0.1:{port}"))
            .await
            .expect("Failed to connect to gRPC server")
    });

    group.bench_function("single_request", |b| {
        b.iter(|| {
            let key = format!("bench_key_{counter}");
            counter += 1;

            runtime.block_on(async {
                let request = tonic::Request::new(ThrottleRequest {
                    key: key.clone(),
                    max_burst: 100,
                    count_per_period: 1000,
                    period: 60,
                    quantity: 1,
                });

                let response = client.throttle(request).await.unwrap();
                response.into_inner().allowed
            })
        });
    });

    group.finish();
}

fn protocol_comparison(c: &mut Criterion) {
    println!("Make sure to run two server instances:");
    println!("  1. cargo run --release -- --http --http-port 9091");
    println!("  2. cargo run --release -- --grpc --grpc-port 9093");
    println!("Waiting for servers to start...");
    std::thread::sleep(Duration::from_secs(2));

    // Test HTTP connection
    let runtime = Runtime::new().unwrap();
    let check_result =
        runtime.block_on(async { reqwest::get("http://127.0.0.1:9091/health").await });
    match check_result {
        Ok(_) => println!("Connected to HTTP server on port 9091"),
        Err(e) => {
            eprintln!("Failed to connect to HTTP server on port 9091: {e}");
            eprintln!(
                "Please start the server with: cargo run --release -- --http --http-port 9091"
            );
            return;
        }
    }

    // Test gRPC connection
    match std::net::TcpStream::connect("127.0.0.1:9093") {
        Ok(_) => println!("Connected to gRPC server on port 9093"),
        Err(e) => {
            eprintln!("Failed to connect to gRPC server on port 9093: {e}");
            eprintln!(
                "Please start the server with: cargo run --release -- --grpc --grpc-port 9093"
            );
            return;
        }
    }

    // Run benchmarks
    benchmark_http_protocol(c, 9091);
    benchmark_grpc_protocol(c, 9093);
}

criterion_group!(benches, protocol_comparison);
criterion_main!(benches);

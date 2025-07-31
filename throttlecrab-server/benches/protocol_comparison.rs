use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::runtime::Runtime;

fn make_native_request(stream: &mut TcpStream, key: &str) -> std::io::Result<bool> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as i64;

    // Write fixed header
    stream.write_all(&[1u8])?; // cmd
    stream.write_all(&[key.len() as u8])?; // key_len
    stream.write_all(&100i64.to_le_bytes())?; // burst
    stream.write_all(&1000i64.to_le_bytes())?; // rate
    stream.write_all(&60i64.to_le_bytes())?; // period
    stream.write_all(&1i64.to_le_bytes())?; // quantity
    stream.write_all(&timestamp.to_le_bytes())?; // timestamp
    stream.write_all(key.as_bytes())?; // key

    // Read response (33 bytes)
    let mut response = [0u8; 33];
    stream.read_exact(&mut response)?;

    Ok(response[1] == 1) // allowed field
}

// Include the generated protobuf code
pub mod throttlecrab_proto {
    tonic::include_proto!("throttlecrab");
}

use throttlecrab_proto::ThrottleRequest;
use throttlecrab_proto::rate_limiter_client::RateLimiterClient;

async fn make_grpc_request(
    client: &mut RateLimiterClient<tonic::transport::Channel>,
    key: &str,
) -> Result<bool, tonic::Status> {
    let now = SystemTime::now();
    let duration = now.duration_since(UNIX_EPOCH).unwrap();

    let request = tonic::Request::new(ThrottleRequest {
        key: key.to_string(),
        max_burst: 100,
        count_per_period: 1000,
        period: 60,
        quantity: 1,
        timestamp: duration.as_nanos() as i64,
    });

    let response = client.throttle(request).await?;
    Ok(response.into_inner().allowed)
}

fn benchmark_native_protocol(c: &mut Criterion, port: u16) {
    let mut group = c.benchmark_group("protocol_native");
    group.throughput(Throughput::Elements(1));
    group.measurement_time(Duration::from_secs(10));
    group.warm_up_time(Duration::from_secs(2));

    group.bench_function("single_request", |b| {
        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        stream.set_nodelay(true).unwrap();
        let mut counter = 0u64;

        b.iter(|| {
            let key = format!("bench_key_{counter}");
            counter += 1;
            make_native_request(&mut stream, &key).unwrap()
        });
    });

    group.bench_function("batch_100", |b| {
        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        stream.set_nodelay(true).unwrap();
        let mut counter = 0u64;

        b.iter(|| {
            for _ in 0..100 {
                let key = format!("bench_key_{counter}");
                counter += 1;
                make_native_request(&mut stream, &key).unwrap();
            }
        });
    });

    group.finish();
}

fn benchmark_grpc_protocol(c: &mut Criterion, port: u16) {
    let mut group = c.benchmark_group("protocol_grpc");
    group.throughput(Throughput::Elements(1));
    group.measurement_time(Duration::from_secs(10));
    group.warm_up_time(Duration::from_secs(2));

    let runtime = Runtime::new().unwrap();

    group.bench_function("single_request", |b| {
        let client = runtime.block_on(async {
            RateLimiterClient::connect(format!("http://127.0.0.1:{port}"))
                .await
                .unwrap()
        });
        let mut client = client;
        let mut counter = 0u64;

        b.iter(|| {
            let key = format!("bench_key_{counter}");
            counter += 1;
            runtime
                .block_on(make_grpc_request(&mut client, &key))
                .unwrap()
        });
    });

    group.bench_function("batch_100", |b| {
        let client = runtime.block_on(async {
            RateLimiterClient::connect(format!("http://127.0.0.1:{port}"))
                .await
                .unwrap()
        });
        let mut client = client;
        let mut counter = 0u64;

        b.iter(|| {
            runtime.block_on(async {
                for _ in 0..100 {
                    let key = format!("bench_key_{counter}");
                    counter += 1;
                    make_grpc_request(&mut client, &key).await.unwrap();
                }
            });
        });
    });

    group.finish();
}

fn protocol_comparison(c: &mut Criterion) {
    println!("Make sure to run two server instances:");
    println!("  1. cargo run --features bin -- --server --port 9092 --native");
    println!("  2. cargo run --features bin -- --server --port 9093 --grpc");
    println!("Waiting for servers to start...");
    std::thread::sleep(Duration::from_secs(2));

    // Test native connection
    match TcpStream::connect("127.0.0.1:9092") {
        Ok(_) => println!("Connected to native server on port 9092"),
        Err(e) => {
            eprintln!("Failed to connect to native server on port 9092: {e}");
            return;
        }
    }

    // Test gRPC connection
    let runtime = Runtime::new().unwrap();
    match runtime.block_on(RateLimiterClient::connect("http://127.0.0.1:9093")) {
        Ok(_) => println!("Connected to grpc server on port 9093"),
        Err(e) => {
            eprintln!("Failed to connect to grpc server on port 9093: {e}");
            return;
        }
    }

    // Run benchmarks
    benchmark_native_protocol(c, 9092);
    benchmark_grpc_protocol(c, 9093);
}

criterion_group!(benches, protocol_comparison);
criterion_main!(benches);

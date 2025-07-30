use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use rmp_serde::Serializer;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::runtime::Runtime;

#[derive(Debug, Serialize, Deserialize)]
struct MsgPackRequest {
    cmd: u8,
    key: String,
    burst: i64,
    rate: i64,
    period: i64,
    quantity: i64,
    timestamp: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct MsgPackResponse {
    ok: bool,
    allowed: u8,
    limit: i64,
    remaining: i64,
    retry_after: i64,
    reset_after: i64,
}

fn make_msgpack_request(stream: &mut TcpStream, key: &str) -> std::io::Result<bool> {
    let request = MsgPackRequest {
        cmd: 1,
        key: key.to_string(),
        burst: 100,
        rate: 1000,
        period: 60,
        quantity: 1,
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64,
    };

    // Serialize request
    let mut buf = Vec::new();
    request.serialize(&mut Serializer::new(&mut buf)).unwrap();

    // Send length prefix
    let len = (buf.len() as u32).to_be_bytes();
    stream.write_all(&len)?;
    stream.write_all(&buf)?;

    // Read response length
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf)?;
    let len = u32::from_be_bytes(len_buf) as usize;

    // Read response
    let mut response_buf = vec![0u8; len];
    stream.read_exact(&mut response_buf)?;

    let response: MsgPackResponse = rmp_serde::from_slice(&response_buf).unwrap();
    Ok(response.allowed == 1)
}

fn make_compact_request(stream: &mut TcpStream, key: &str) -> std::io::Result<bool> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

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
        max_burst: 100,
        count_per_period: 1000,
        period: 60,
        quantity: 1,
        timestamp_secs: duration.as_secs() as i64,
        timestamp_nanos: duration.subsec_nanos() as i32,
    });

    let response = client.throttle(request).await?;
    Ok(response.into_inner().allowed)
}

fn benchmark_protocol(c: &mut Criterion, port: u16, name: &str, is_compact: bool) {
    let mut group = c.benchmark_group(name);
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
            if is_compact {
                make_compact_request(&mut stream, &key).unwrap()
            } else {
                make_msgpack_request(&mut stream, &key).unwrap()
            }
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
                if is_compact {
                    make_compact_request(&mut stream, &key).unwrap();
                } else {
                    make_msgpack_request(&mut stream, &key).unwrap();
                }
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
    println!("Make sure to run four server instances:");
    println!("  1. cargo run --features bin -- --server --port 9090");
    println!("  2. cargo run --features bin -- --server --port 9091 --optimized");
    println!("  3. cargo run --features bin -- --server --port 9092 --compact");
    println!("  4. cargo run --features bin -- --server --port 9093 --grpc");
    println!("Waiting for servers to start...");
    std::thread::sleep(Duration::from_secs(2));

    // Test connections
    let servers = [
        (9090, "standard", false),
        (9091, "optimized", false),
        (9092, "compact", true),
    ];

    for (port, name, _) in &servers {
        match TcpStream::connect(format!("127.0.0.1:{port}")) {
            Ok(_) => println!("Connected to {name} server on port {port}"),
            Err(e) => {
                eprintln!("Failed to connect to {name} server on port {port}: {e}");
                return;
            }
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
    for (port, name, is_compact) in servers {
        benchmark_protocol(c, port, &format!("protocol_{name}"), is_compact);
    }

    // Run gRPC benchmark
    benchmark_grpc_protocol(c, 9093);
}

criterion_group!(benches, protocol_comparison);
criterion_main!(benches);

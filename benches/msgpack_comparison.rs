use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use rmp_serde::Serializer;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Deserialize)]
struct Request {
    cmd: u8, // 1 = throttle
    key: String,
    burst: i64,
    rate: i64,
    period: i64,
    quantity: i64,
    timestamp: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct Response {
    ok: bool,
    allowed: u8, // 0 or 1
    limit: i64,
    remaining: i64,
    retry_after: i64,
    reset_after: i64,
}

fn make_request(stream: &mut TcpStream, key: &str) -> std::io::Result<bool> {
    let request = Request {
        cmd: 1, // throttle command
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

    let response: Response = rmp_serde::from_slice(&response_buf).unwrap();
    Ok(response.allowed == 1)
}

fn benchmark_msgpack_transport(c: &mut Criterion, port: u16, name: &str) {
    let mut group = c.benchmark_group(name);
    group.throughput(Throughput::Elements(1));
    group.measurement_time(Duration::from_secs(10));
    group.warm_up_time(Duration::from_secs(3));

    group.bench_function("single_request", |b| {
        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        stream.set_nodelay(true).unwrap();
        let mut counter = 0u64;

        b.iter(|| {
            let key = format!("bench_key_{counter}");
            counter += 1;
            make_request(&mut stream, &key).unwrap()
        });
    });

    group.bench_function("pipelined_10", |b| {
        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        stream.set_nodelay(true).unwrap();
        let mut counter = 0u64;

        b.iter(|| {
            for _ in 0..10 {
                let key = format!("bench_key_{counter}");
                counter += 1;
                make_request(&mut stream, &key).unwrap();
            }
        });
    });

    group.finish();
}

fn msgpack_comparison(c: &mut Criterion) {
    println!("Make sure to run two server instances:");
    println!("  1. cargo run --features bin -- --server --port 9090");
    println!("  2. cargo run --features bin -- --server --port 9091 --optimized");
    println!("Waiting for servers to start...");
    std::thread::sleep(Duration::from_secs(2));

    // Test connection to both servers
    match TcpStream::connect("127.0.0.1:9090") {
        Ok(_) => println!("Connected to standard server on port 9090"),
        Err(e) => {
            eprintln!("Failed to connect to standard server on port 9090: {e}");
            eprintln!(
                "Please start the server with: cargo run --features bin -- --server --port 9090"
            );
            return;
        }
    }

    match TcpStream::connect("127.0.0.1:9091") {
        Ok(_) => println!("Connected to optimized server on port 9091"),
        Err(e) => {
            eprintln!("Failed to connect to optimized server on port 9091: {e}");
            eprintln!(
                "Please start the server with: cargo run --features bin -- --server --port 9091 --optimized"
            );
            return;
        }
    }

    benchmark_msgpack_transport(c, 9090, "msgpack_standard");
    benchmark_msgpack_transport(c, 9091, "msgpack_optimized");
}

criterion_group!(benches, msgpack_comparison);
criterion_main!(benches);

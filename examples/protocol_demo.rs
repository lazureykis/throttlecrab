use rmp_serde::Serializer;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

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

fn test_msgpack(port: u16, protocol_name: &str) -> std::io::Result<()> {
    println!("\n{protocol_name} Protocol Test (port {port})");
    println!("{}", "-".repeat(40));

    let mut stream = TcpStream::connect(format!("127.0.0.1:{port}"))?;
    stream.set_nodelay(true)?;

    // Test single request
    let start = Instant::now();
    let request = MsgPackRequest {
        cmd: 1,
        key: "test_key".to_string(),
        burst: 100,
        rate: 1000,
        period: 60,
        quantity: 1,
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64,
    };

    let mut buf = Vec::new();
    request.serialize(&mut Serializer::new(&mut buf)).unwrap();

    stream.write_all(&(buf.len() as u32).to_be_bytes())?;
    stream.write_all(&buf)?;

    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf)?;
    let len = u32::from_be_bytes(len_buf) as usize;

    let mut response_buf = vec![0u8; len];
    stream.read_exact(&mut response_buf)?;

    let response: MsgPackResponse = rmp_serde::from_slice(&response_buf).unwrap();
    let latency = start.elapsed();

    println!("Single request latency: {latency:?}");
    println!(
        "Response: allowed={}, remaining={}",
        response.allowed, response.remaining
    );

    // Throughput test
    let start = Instant::now();
    let num_requests = 10_000;

    for i in 0..num_requests {
        let request = MsgPackRequest {
            cmd: 1,
            key: format!("key_{i}"),
            burst: 100,
            rate: 1000,
            period: 60,
            quantity: 1,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
        };

        buf.clear();
        request.serialize(&mut Serializer::new(&mut buf)).unwrap();

        stream.write_all(&(buf.len() as u32).to_be_bytes())?;
        stream.write_all(&buf)?;

        stream.read_exact(&mut len_buf)?;
        let len = u32::from_be_bytes(len_buf) as usize;

        if response_buf.len() < len {
            response_buf.resize(len, 0);
        }
        stream.read_exact(&mut response_buf[..len])?;
    }

    let duration = start.elapsed();
    let throughput = num_requests as f64 / duration.as_secs_f64();

    println!("Throughput test: {num_requests} requests in {duration:?}");
    println!("Rate: {throughput:.0} req/s");

    Ok(())
}

fn test_compact(port: u16) -> std::io::Result<()> {
    println!("\nCompact Binary Protocol Test (port {port})");
    println!("{}", "-".repeat(40));

    let mut stream = TcpStream::connect(format!("127.0.0.1:{port}"))?;
    stream.set_nodelay(true)?;

    // Test single request
    let start = Instant::now();
    let key = "test_key";
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    stream.write_all(&[1u8])?; // cmd
    stream.write_all(&[key.len() as u8])?; // key_len
    stream.write_all(&100i64.to_le_bytes())?; // burst
    stream.write_all(&1000i64.to_le_bytes())?; // rate
    stream.write_all(&60i64.to_le_bytes())?; // period
    stream.write_all(&1i64.to_le_bytes())?; // quantity
    stream.write_all(&timestamp.to_le_bytes())?; // timestamp
    stream.write_all(key.as_bytes())?; // key

    let mut response = [0u8; 33];
    stream.read_exact(&mut response)?;
    let latency = start.elapsed();

    println!("Single request latency: {latency:?}");
    println!("Response: ok={}, allowed={}", response[0], response[1]);

    // Throughput test
    let start = Instant::now();
    let num_requests = 10_000;

    for i in 0..num_requests {
        let key = format!("key_{i}");
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        stream.write_all(&[1u8])?; // cmd
        stream.write_all(&[key.len() as u8])?; // key_len
        stream.write_all(&100i64.to_le_bytes())?; // burst
        stream.write_all(&1000i64.to_le_bytes())?; // rate
        stream.write_all(&60i64.to_le_bytes())?; // period
        stream.write_all(&1i64.to_le_bytes())?; // quantity
        stream.write_all(&timestamp.to_le_bytes())?; // timestamp
        stream.write_all(key.as_bytes())?; // key

        stream.read_exact(&mut response)?;
    }

    let duration = start.elapsed();
    let throughput = num_requests as f64 / duration.as_secs_f64();

    println!("Throughput test: {num_requests} requests in {duration:?}");
    println!("Rate: {throughput:.0} req/s");

    Ok(())
}

fn main() -> std::io::Result<()> {
    println!("ThrottleCrab Protocol Performance Demo");
    println!("======================================");
    println!();
    println!("Start the servers with:");
    println!("  1. cargo run --features bin -- --server --port 9090");
    println!("  2. cargo run --features bin -- --server --port 9091 --optimized");
    println!("  3. cargo run --features bin -- --server --port 9092 --compact");
    println!();

    // Test standard MessagePack
    if let Err(e) = test_msgpack(9090, "Standard MessagePack") {
        eprintln!("Standard protocol test failed: {e}");
    }

    // Test optimized MessagePack
    if let Err(e) = test_msgpack(9091, "Optimized MessagePack") {
        eprintln!("Optimized protocol test failed: {e}");
    }

    // Test compact binary protocol
    if let Err(e) = test_compact(9092) {
        eprintln!("Compact protocol test failed: {e}");
    }

    println!("\nProtocol comparison summary:");
    println!("- Standard MessagePack: Good compatibility, moderate performance");
    println!("- Optimized MessagePack: Better performance, same compatibility");
    println!("- Compact Binary: Best performance, custom protocol");

    Ok(())
}

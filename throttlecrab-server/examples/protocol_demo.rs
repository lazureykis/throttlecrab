use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

fn test_native(port: u16) -> std::io::Result<()> {
    println!("\nNative Binary Protocol Test (port {port})");
    println!("{}", "-".repeat(40));

    let mut stream = TcpStream::connect(format!("127.0.0.1:{port}"))?;
    stream.set_nodelay(true)?;

    // Test single request
    let start = Instant::now();
    let key = "test_key";
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as i64;

    stream.write_all(&[1u8])?; // cmd
    stream.write_all(&[key.len() as u8])?; // key_len
    stream.write_all(&100i64.to_le_bytes())?; // burst
    stream.write_all(&1000i64.to_le_bytes())?; // rate
    stream.write_all(&60i64.to_le_bytes())?; // period
    stream.write_all(&1i64.to_le_bytes())?; // quantity
    stream.write_all(&timestamp.to_le_bytes())?; // timestamp
    stream.write_all(key.as_bytes())?; // key

    let mut response = [0u8; 34];
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
            .as_nanos() as i64;

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
    println!("Start the server with:");
    println!("  cargo run --release -- --native --native-port 9092");
    println!();

    // Test native binary protocol
    if let Err(e) = test_native(9092) {
        eprintln!("Native protocol test failed: {e}");
    }

    println!("\nProtocol summary:");
    println!("- Native Binary: Best performance, custom protocol");
    println!("- HTTP: Standard REST API (use curl or HTTP client)");
    println!("- gRPC: High-performance RPC protocol (use grpcurl or gRPC client)");

    Ok(())
}

use rmp_serde::Serializer;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Deserialize)]
struct Request {
    cmd: u8,
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
    allowed: u8,
    limit: i64,
    remaining: i64,
    retry_after: i64,
    reset_after: i64,
}

fn main() -> std::io::Result<()> {
    println!("Testing optimized MessagePack transport...");

    let mut stream = TcpStream::connect("127.0.0.1:9090")?;
    stream.set_nodelay(true)?;

    let request = Request {
        cmd: 1,
        key: "test_key".to_string(),
        burst: 100,
        rate: 1000,
        period: 60,
        quantity: 1,
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as i64,
    };

    // Serialize request
    let mut buf = Vec::new();
    request.serialize(&mut Serializer::new(&mut buf)).unwrap();
    println!("Request serialized, size: {} bytes", buf.len());

    // Send length prefix
    let len_bytes = (buf.len() as u32).to_be_bytes();
    println!("Sending length prefix: {len_bytes:?}");
    stream.write_all(&len_bytes)?;

    // Send request
    println!("Sending request data...");
    stream.write_all(&buf)?;
    stream.flush()?;

    println!("Waiting for response...");

    // Read response length
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf)?;
    let len = u32::from_be_bytes(len_buf) as usize;
    println!("Response length: {len} bytes");

    // Read response
    let mut response_buf = vec![0u8; len];
    stream.read_exact(&mut response_buf)?;

    let response: Response = rmp_serde::from_slice(&response_buf).unwrap();
    println!("Response: {response:?}");

    Ok(())
}

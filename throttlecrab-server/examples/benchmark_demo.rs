use rmp_serde::Serializer;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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
            .as_nanos() as i64,
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

fn main() {
    println!("ThrottleCrab Benchmark Demo");
    println!("===========================");
    println!();
    println!("Make sure the server is running:");
    println!("  cargo run --features bin -- --server");
    println!();

    // Test single thread performance
    println!("1. Single Thread Performance Test");
    println!("---------------------------------");
    let mut stream = match TcpStream::connect("127.0.0.1:9090") {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to connect to server: {e}");
            eprintln!("Is the server running?");
            return;
        }
    };

    let start = Instant::now();
    let num_requests = 10_000;

    for i in 0..num_requests {
        let key = format!("single_thread_key_{i}");
        make_request(&mut stream, &key).unwrap();
    }

    let duration = start.elapsed();
    let requests_per_sec = num_requests as f64 / duration.as_secs_f64();
    println!("  Requests: {num_requests}");
    println!("  Duration: {:.2}s", duration.as_secs_f64());
    println!("  Throughput: {requests_per_sec:.0} req/s");
    println!();

    // Test multi-thread performance
    println!("2. Multi-Thread Performance Test (8 threads)");
    println!("--------------------------------------------");

    let num_threads = 8;
    let requests_per_thread = 5_000;
    let total_requests = Arc::new(AtomicU64::new(0));
    let start = Instant::now();

    let mut handles = vec![];

    for thread_id in 0..num_threads {
        let total_requests = total_requests.clone();

        let handle = thread::spawn(move || {
            let mut stream = TcpStream::connect("127.0.0.1:9090").unwrap();

            for i in 0..requests_per_thread {
                let key = format!("thread_{thread_id}_key_{i}");
                if make_request(&mut stream, &key).unwrap() {
                    total_requests.fetch_add(1, Ordering::Relaxed);
                }
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let duration = start.elapsed();
    let total = total_requests.load(Ordering::Relaxed);
    let requests_per_sec = total as f64 / duration.as_secs_f64();

    println!("  Threads: {num_threads}");
    println!("  Requests per thread: {requests_per_thread}");
    println!("  Total requests: {total}");
    println!("  Duration: {:.2}s", duration.as_secs_f64());
    println!("  Throughput: {requests_per_sec:.0} req/s");
    println!();

    // Test burst pattern
    println!("3. Burst Pattern Test");
    println!("---------------------");

    let mut stream = TcpStream::connect("127.0.0.1:9090").unwrap();
    let num_bursts = 100;
    let burst_size = 50;
    let start = Instant::now();
    let mut total_allowed = 0;

    for burst in 0..num_bursts {
        // Send burst
        for i in 0..burst_size {
            let key = format!("burst_key_{burst}_{i}");
            if make_request(&mut stream, &key).unwrap() {
                total_allowed += 1;
            }
        }

        // Small pause between bursts
        thread::sleep(Duration::from_millis(10));
    }

    let duration = start.elapsed();
    let total_requests = num_bursts * burst_size;
    let requests_per_sec = total_requests as f64 / duration.as_secs_f64();

    println!("  Bursts: {num_bursts}");
    println!("  Burst size: {burst_size}");
    println!("  Total requests: {total_requests}");
    println!("  Allowed requests: {total_allowed}");
    println!("  Duration: {:.2}s", duration.as_secs_f64());
    println!("  Throughput: {requests_per_sec:.0} req/s");
}

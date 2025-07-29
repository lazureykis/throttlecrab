use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rmp_serde::Serializer;
use serde::{Deserialize, Serialize};
use std::hint::black_box;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
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

fn make_request(stream: &mut TcpStream, key: &str) -> bool {
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
    stream.write_all(&len).unwrap();
    stream.write_all(&buf).unwrap();

    // Read response length
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).unwrap();
    let len = u32::from_be_bytes(len_buf) as usize;

    // Read response
    let mut response_buf = vec![0u8; len];
    stream.read_exact(&mut response_buf).unwrap();

    let response: Response = rmp_serde::from_slice(&response_buf).unwrap();
    response.allowed == 1
}

fn bench_single_thread(c: &mut Criterion) {
    // Note: Server must be running before benchmarks
    // Run: cargo run --features bin -- --server
    
    let mut group = c.benchmark_group("single_thread");
    group.throughput(Throughput::Elements(1000));
    
    // Create connection once before benchmarking
    let mut stream = TcpStream::connect("127.0.0.1:9090")
        .expect("Failed to connect. Is the server running? Run: cargo run --features bin -- --server");
    
    // Set TCP_NODELAY for lower latency
    stream.set_nodelay(true).expect("Failed to set TCP_NODELAY");
    
    group.bench_function("sequential_requests", |b| {
        let mut counter = 0u64;
        
        b.iter(|| {
            let key = format!("bench_key_{}", counter);
            counter += 1;
            black_box(make_request(&mut stream, &key));
        });
    });

    group.finish();
}

fn bench_multi_thread(c: &mut Criterion) {
    let mut group = c.benchmark_group("multi_thread");
    
    for num_threads in [2, 4, 8, 16].iter() {
        group.throughput(Throughput::Elements(1000 * (*num_threads as u64)));
        
        group.bench_with_input(
            BenchmarkId::from_parameter(num_threads),
            num_threads,
            |b, &num_threads| {
                let stop = Arc::new(AtomicBool::new(false));
                let counter = Arc::new(AtomicU64::new(0));
                
                b.iter_custom(|iters| {
                    let mut handles = vec![];
                    let start = std::time::Instant::now();
                    let requests_per_thread = iters / num_threads as u64;
                    
                    for thread_id in 0..num_threads {
                        let stop = stop.clone();
                        let counter = counter.clone();
                        
                        let handle = thread::spawn(move || {
                            let mut stream = TcpStream::connect("127.0.0.1:9090").unwrap();
                            stream.set_nodelay(true).unwrap();
                            
                            for _ in 0..requests_per_thread {
                                if stop.load(Ordering::Relaxed) {
                                    break;
                                }
                                
                                let key_num = counter.fetch_add(1, Ordering::Relaxed);
                                let key = format!("bench_key_{}_{}", thread_id, key_num);
                                make_request(&mut stream, &key);
                            }
                        });
                        
                        handles.push(handle);
                    }
                    
                    for handle in handles {
                        handle.join().unwrap();
                    }
                    
                    start.elapsed()
                });
            },
        );
    }
    
    group.finish();
}

fn bench_burst_pattern(c: &mut Criterion) {
    let mut group = c.benchmark_group("burst_pattern");
    
    // Create connection once
    let mut stream = TcpStream::connect("127.0.0.1:9090").unwrap();
    stream.set_nodelay(true).unwrap();
    
    group.bench_function("burst_then_wait", |b| {
        let mut counter = 0u64;
        
        b.iter(|| {
            // Send burst of 10 requests
            for i in 0..10 {
                let key = format!("burst_key_{}_{}", counter, i);
                black_box(make_request(&mut stream, &key));
            }
            counter += 1;
            
            // Wait a bit
            thread::sleep(Duration::from_millis(10));
        });
    });
    
    group.finish();
}

fn bench_mixed_keys(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_keys");
    
    // Create connection once
    let mut stream = TcpStream::connect("127.0.0.1:9090").unwrap();
    stream.set_nodelay(true).unwrap();
    
    group.bench_function("rotating_keys", |b| {
        let keys = vec!["key_a", "key_b", "key_c", "key_d", "key_e"];
        let mut counter = 0usize;
        
        b.iter(|| {
            let key = keys[counter % keys.len()];
            counter += 1;
            black_box(make_request(&mut stream, key));
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_single_thread,
    bench_multi_thread,
    bench_burst_pattern,
    bench_mixed_keys
);
criterion_main!(benches);
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rmp_serde::Serializer;
use serde::{Deserialize, Serialize};
use std::hint::black_box;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

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

struct ConnectionPool {
    connections: Vec<Mutex<TcpStream>>,
}

impl ConnectionPool {
    fn new(size: usize, addr: &str) -> Self {
        let connections = (0..size)
            .map(|_| {
                let mut stream = TcpStream::connect(addr).unwrap();
                stream.set_nodelay(true).unwrap();
                Mutex::new(stream)
            })
            .collect();
        
        ConnectionPool { connections }
    }
    
    fn with_connection<F, R>(&self, idx: usize, f: F) -> R
    where
        F: FnOnce(&mut TcpStream) -> R,
    {
        let mut stream = self.connections[idx % self.connections.len()].lock().unwrap();
        f(&mut stream)
    }
}

fn bench_connection_pool(c: &mut Criterion) {
    let mut group = c.benchmark_group("connection_pool");
    
    for pool_size in [1, 4, 8, 16].iter() {
        group.throughput(Throughput::Elements(1000));
        
        // Create connection pool
        let pool = Arc::new(ConnectionPool::new(*pool_size, "127.0.0.1:9090"));
        
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("pool_{}", pool_size)),
            pool_size,
            |b, _| {
                let mut counter = 0u64;
                
                b.iter(|| {
                    let key = format!("pool_key_{}", counter);
                    let idx = counter as usize;
                    counter += 1;
                    
                    let result = pool.with_connection(idx, |stream| {
                        make_request(stream, &key)
                    });
                    
                    black_box(result);
                });
            },
        );
    }
    
    group.finish();
}

fn bench_concurrent_pool(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_pool");
    
    let num_threads = 8;
    let pool_size = 16;
    
    group.throughput(Throughput::Elements((num_threads * 1000) as u64));
    
    group.bench_function("8_threads_16_connections", |b| {
        let pool = Arc::new(ConnectionPool::new(pool_size, "127.0.0.1:9090"));
        
        b.iter_custom(|iters| {
            let requests_per_thread = iters / num_threads as u64;
            let mut handles = vec![];
            let start = std::time::Instant::now();
            
            for thread_id in 0..num_threads {
                let pool = pool.clone();
                
                let handle = thread::spawn(move || {
                    for i in 0..requests_per_thread {
                        let key = format!("concurrent_{}_{}", thread_id, i);
                        let idx = (thread_id * requests_per_thread as usize + i as usize);
                        
                        pool.with_connection(idx, |stream| {
                            make_request(stream, &key)
                        });
                    }
                });
                
                handles.push(handle);
            }
            
            for handle in handles {
                handle.join().unwrap();
            }
            
            start.elapsed()
        });
    });
    
    group.finish();
}

criterion_group!(benches, bench_connection_pool, bench_concurrent_pool);
criterion_main!(benches);
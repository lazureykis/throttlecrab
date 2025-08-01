use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Native protocol request format
struct Request {
    cmd: u8, // 1 = throttle
    key: String,
    burst: i64,
    rate: i64,
    period: i64,
    quantity: i64,
}

fn make_request(stream: &mut TcpStream, key: &str) -> bool {
    let request = Request {
        cmd: 1, // throttle command
        key: key.to_string(),
        burst: 100,
        rate: 1000,
        period: 60,
        quantity: 1,
    };

    // Send native protocol request
    let key_bytes = request.key.as_bytes();
    let key_len = key_bytes.len().min(255) as u8;

    // Write fixed header (34 bytes)
    stream.write_all(&[request.cmd]).unwrap(); // cmd: u8
    stream.write_all(&[key_len]).unwrap(); // key_len: u8
    stream.write_all(&request.burst.to_le_bytes()).unwrap(); // burst: i64
    stream.write_all(&request.rate.to_le_bytes()).unwrap(); // rate: i64
    stream.write_all(&request.period.to_le_bytes()).unwrap(); // period: i64
    stream.write_all(&request.quantity.to_le_bytes()).unwrap(); // quantity: i64

    // Write key
    stream.write_all(&key_bytes[..key_len as usize]).unwrap();

    // Read response (34 bytes fixed)
    let mut response_buf = [0u8; 34];
    stream.read_exact(&mut response_buf).unwrap();

    // Parse response
    let ok = response_buf[0];
    let allowed = response_buf[1];
    let _limit = i64::from_le_bytes(response_buf[2..10].try_into().unwrap());
    let _remaining = i64::from_le_bytes(response_buf[10..18].try_into().unwrap());
    let _retry_after = i64::from_le_bytes(response_buf[18..26].try_into().unwrap());
    let _reset_after = i64::from_le_bytes(response_buf[26..34].try_into().unwrap());

    ok == 1 && allowed == 1
}

struct ConnectionPool {
    connections: Vec<Mutex<TcpStream>>,
}

impl ConnectionPool {
    fn new(size: usize, addr: &str) -> Self {
        let connections = (0..size)
            .map(|_| {
                let stream = TcpStream::connect(addr).unwrap();
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
        let mut stream = self.connections[idx % self.connections.len()]
            .lock()
            .unwrap();
        f(&mut stream)
    }
}

fn bench_connection_pool(c: &mut Criterion) {
    let mut group = c.benchmark_group("connection_pool");

    for pool_size in [1, 4, 8, 16].iter() {
        group.throughput(Throughput::Elements(1000));

        // Create connection pool
        let pool = Arc::new(ConnectionPool::new(*pool_size, "127.0.0.1:9092"));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("pool_{pool_size}")),
            pool_size,
            |b, _| {
                let mut counter = 0u64;

                b.iter(|| {
                    let key = format!("pool_key_{counter}");
                    let idx = counter as usize;
                    counter += 1;

                    let result = pool.with_connection(idx, |stream| make_request(stream, &key));

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
        let pool = Arc::new(ConnectionPool::new(pool_size, "127.0.0.1:9092"));

        b.iter_custom(|iters| {
            let requests_per_thread = iters / num_threads as u64;
            let mut handles = vec![];
            let start = std::time::Instant::now();

            for thread_id in 0..num_threads {
                let pool = pool.clone();

                let handle = thread::spawn(move || {
                    for i in 0..requests_per_thread {
                        let key = format!("concurrent_{thread_id}_{i}");
                        let idx = thread_id * requests_per_thread as usize + i as usize;

                        pool.with_connection(idx, |stream| make_request(stream, &key));
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

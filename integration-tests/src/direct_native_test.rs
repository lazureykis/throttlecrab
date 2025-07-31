//! Direct native protocol test without connection pooling

use anyhow::Result;
use bytes::{BufMut, BytesMut};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Barrier;
use tokio::task::JoinSet;

pub async fn run_direct_native_test(
    num_threads: usize,
    requests_per_thread: usize,
    port: u16,
) -> Result<()> {
    println!("=== Direct Native Protocol Test (No Pool) ===");
    println!("Threads: {num_threads}");
    println!("Requests per thread: {requests_per_thread}");
    let total_expected = num_threads * requests_per_thread;
    println!("Total requests: {total_expected}");
    println!("Target port: {port}\n");

    let stats = Arc::new(Stats {
        total: AtomicU64::new(0),
        successful: AtomicU64::new(0),
    });
    let barrier = Arc::new(Barrier::new(num_threads + 1));
    let start_flag = Arc::new(AtomicU64::new(0));

    let mut tasks = JoinSet::new();
    for thread_id in 0..num_threads {
        let stats = stats.clone();
        let barrier = barrier.clone();
        let start_flag = start_flag.clone();

        tasks.spawn(async move {
            // Each thread gets its own connection
            let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).await?;
            stream.set_nodelay(true)?;
            
            // Pre-allocate buffers
            let mut request_buf = BytesMut::with_capacity(256);
            let mut response_buf = [0u8; 34];

            barrier.wait().await;

            while start_flag.load(Ordering::Acquire) == 0 {
                tokio::task::yield_now().await;
            }

            for i in 0..requests_per_thread {
                let key = format!("key_{}_{}", thread_id, i % 1000);
                
                // Build request
                request_buf.clear();
                request_buf.put_u8(1); // cmd
                request_buf.put_u8(key.len() as u8); // key_len
                request_buf.put_i64_le(10000); // burst
                request_buf.put_i64_le(100000); // rate
                request_buf.put_i64_le(60); // period
                request_buf.put_i64_le(1); // quantity
                
                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos() as i64;
                request_buf.put_i64_le(timestamp);
                request_buf.put_slice(key.as_bytes());

                // Send request
                stream.write_all(&request_buf).await?;
                
                // Read response
                stream.read_exact(&mut response_buf).await?;
                
                let allowed = response_buf[1];
                stats.total.fetch_add(1, Ordering::Relaxed);
                if allowed == 1 {
                    stats.successful.fetch_add(1, Ordering::Relaxed);
                }
            }
            
            Ok::<(), anyhow::Error>(())
        });
    }

    println!("Waiting for all threads to be ready...");
    barrier.wait().await;

    println!("Starting benchmark...");
    let bench_start = Instant::now();
    start_flag.store(1, Ordering::Release);

    while let Some(result) = tasks.join_next().await {
        result??;
    }

    let duration = bench_start.elapsed();

    let total = stats.total.load(Ordering::Relaxed);
    let successful = stats.successful.load(Ordering::Relaxed);
    let rps = total as f64 / duration.as_secs_f64();

    println!("\n=== Benchmark Results ===");
    println!("Duration: {duration:?}");
    println!("Total requests: {total}");
    println!("Throughput: {rps:.2} requests/sec");
    println!("Successful: {successful} ({}%)", successful * 100 / total);

    Ok(())
}

struct Stats {
    total: AtomicU64,
    successful: AtomicU64,
}
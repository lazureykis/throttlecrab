use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use std::time::Duration;
use throttlecrab_client::{ClientBuilder, ThrottleCrabClient};
use throttlecrab_server::actor::RateLimiterActor;
use throttlecrab_server::transport::{Transport, native::NativeTransport};
use tokio::runtime::Runtime;

fn setup_server_and_client(rt: &Runtime) -> (u16, ThrottleCrabClient) {
    rt.block_on(async {
        // Start server
        let store = throttlecrab::PeriodicStore::builder()
            .capacity(1_000_000)
            .cleanup_interval(Duration::from_secs(300))
            .build();
        let limiter = RateLimiterActor::spawn_periodic(100_000, store);

        // Get random port
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let transport = NativeTransport::new("127.0.0.1", port);

        // Spawn server
        tokio::spawn(async move {
            transport.start(limiter).await.unwrap();
        });

        // Small delay to ensure server is ready
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Create client with optimized settings
        let client = ClientBuilder::new()
            .max_connections(10)
            .min_idle_connections(5)
            .connect_timeout(Duration::from_secs(5))
            .request_timeout(Duration::from_secs(30))
            .tcp_nodelay(true)
            .build(format!("127.0.0.1:{port}"))
            .await
            .unwrap();

        (port, client)
    })
}

fn benchmark_single_request(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let (_port, client) = setup_server_and_client(&rt);

    c.bench_function("client_single_request", |b| {
        b.to_async(&rt).iter(|| async {
            client
                .check_rate_limit("bench_key", 1000000, 10000000, 60)
                .await
                .unwrap()
        });
    });
}

fn benchmark_concurrent_requests(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let (_port, client) = setup_server_and_client(&rt);

    let mut group = c.benchmark_group("client_concurrent");

    for num_tasks in [1, 5, 10, 20].iter() {
        group.throughput(Throughput::Elements(*num_tasks as u64));
        group.bench_with_input(format!("tasks_{num_tasks}"), num_tasks, |b, &num_tasks| {
            b.to_async(&rt).iter(|| async {
                let mut handles = vec![];

                for i in 0..num_tasks {
                    let client = client.clone();
                    handles.push(tokio::spawn(async move {
                        client
                            .check_rate_limit(format!("bench_key_{i}"), 1000000, 10000000, 60)
                            .await
                            .unwrap()
                    }));
                }

                for handle in handles {
                    handle.await.unwrap();
                }
            });
        });
    }

    group.finish();
}

fn benchmark_pool_saturation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    // Create client with small pool to test saturation
    let (_port, client) = rt.block_on(async {
        let store = throttlecrab::PeriodicStore::builder()
            .capacity(1_000_000)
            .cleanup_interval(Duration::from_secs(300))
            .build();
        let limiter = RateLimiterActor::spawn_periodic(100_000, store);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let transport = NativeTransport::new("127.0.0.1", port);

        tokio::spawn(async move {
            transport.start(limiter).await.unwrap();
        });

        tokio::time::sleep(Duration::from_millis(100)).await;

        let client = ClientBuilder::new()
            .max_connections(2) // Small pool
            .min_idle_connections(1)
            .build(format!("127.0.0.1:{port}"))
            .await
            .unwrap();

        (port, client)
    });

    c.bench_function("client_pool_saturation", |b| {
        b.to_async(&rt).iter(|| async {
            // Launch more tasks than pool size
            let mut handles = vec![];

            for i in 0..10 {
                let client = client.clone();
                handles.push(tokio::spawn(async move {
                    client
                        .check_rate_limit(format!("saturation_key_{i}"), 1000000, 10000000, 60)
                        .await
                        .unwrap()
                }));
            }

            for handle in handles {
                handle.await.unwrap();
            }
        });
    });
}

criterion_group!(
    benches,
    benchmark_single_request,
    benchmark_concurrent_requests,
    benchmark_pool_saturation
);
criterion_main!(benches);

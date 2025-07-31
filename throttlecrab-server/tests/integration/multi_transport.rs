use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinSet;

use super::transport_tests::{GrpcClient, HttpClient, NativeClient, Transport};
use super::workload::{WorkloadConfig, WorkloadGenerator, WorkloadStats};

pub async fn run_multi_transport_test(workload_config: WorkloadConfig) -> Result<()> {
    println!("\n=== Multi-Transport Concurrent Test ===");
    println!("Running all 4 transports simultaneously against a single server instance\n");

    // Start server with all transports enabled
    let mut cmd = tokio::process::Command::new("cargo");
    cmd.arg("run")
        .arg("--release")
        .arg("-p")
        .arg("throttlecrab-server")
        .arg("--")
        .arg("--http")
        .arg("--http-port")
        .arg("28080")
        .arg("--grpc")
        .arg("--grpc-port")
        .arg("28070")
        .arg("--native")
        .arg("--native-port")
        .arg("28071")
        .arg("--store")
        .arg("adaptive")
        .arg("--store-capacity")
        .arg("200000")
        .arg("--buffer-size")
        .arg("200000")
        .arg("--log-level")
        .arg("warn");

    let mut server_process = cmd.spawn()?;

    // Wait for server to start
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Create shared statistics
    let total_stats = Arc::new(WorkloadStats::new());
    let mut tasks = JoinSet::new();

    // Spawn workload for each transport
    let transports = vec![
        (Transport::Http, 28080),
        (Transport::Grpc, 28070),
        (Transport::Native, 28071),
    ];

    let start_time = std::time::Instant::now();

    for (transport, port) in transports {
        let workload = workload_config.clone();
        let stats = total_stats.clone();

        tasks.spawn(async move {
            println!(
                "Starting {} workload on port {}",
                transport.flag_name(),
                port
            );

            let generator = WorkloadGenerator::new(workload);
            let transport_stats = generator.stats();

            let result = match transport {
                Transport::Http => {
                    let client = Arc::new(HttpClient::new(port));
                    generator
                        .run(move |key| {
                            let client = client.clone();
                            async move { client.test_request(key).await }
                        })
                        .await
                }
                Transport::Grpc => match GrpcClient::new(port).await {
                    Ok(grpc_client) => {
                        generator
                            .run(move |key| {
                                let mut client = grpc_client.clone();
                                async move { client.test_request(key).await }
                            })
                            .await
                    }
                    Err(e) => Err(e),
                },
                Transport::Native => {
                    let client = Arc::new(NativeClient::new(port, 50));
                    generator
                        .run(move |key| {
                            let client = client.clone();
                            async move { client.test_request(key).await }
                        })
                        .await
                }
            };

            // Aggregate stats
            let summary = transport_stats.get_summary();
            stats
                .total_requests
                .fetch_add(summary.total_requests, std::sync::atomic::Ordering::Relaxed);
            stats.successful_requests.fetch_add(
                summary.successful_requests,
                std::sync::atomic::Ordering::Relaxed,
            );
            stats.failed_requests.fetch_add(
                summary.failed_requests,
                std::sync::atomic::Ordering::Relaxed,
            );
            stats
                .rate_limited
                .fetch_add(summary.rate_limited, std::sync::atomic::Ordering::Relaxed);

            (transport, summary, result)
        });
    }

    // Wait for all workloads to complete
    let mut results = Vec::new();
    while let Some(result) = tasks.join_next().await {
        match result {
            Ok((transport, summary, workload_result)) => {
                if let Err(e) = workload_result {
                    eprintln!("Error in {} workload: {}", transport.flag_name(), e);
                }
                results.push((transport, summary));
            }
            Err(e) => eprintln!("Task error: {e}"),
        }
    }

    let duration = start_time.elapsed();

    // Print individual transport results
    println!("\n--- Per-Transport Results ---");
    for (transport, summary) in results {
        println!("\n{} Transport:", transport.flag_name().to_uppercase());
        println!("  Requests: {}", summary.total_requests);
        println!(
            "  Success rate: {:.2}%",
            summary.successful_requests as f64 / summary.total_requests as f64 * 100.0
        );
        println!("  P50 latency: {:?}", summary.latency_p50);
        println!("  P99 latency: {:?}", summary.latency_p99);
    }

    // Print aggregate results
    println!("\n--- Aggregate Results ---");
    let total_summary = total_stats.get_summary();
    total_summary.print_summary(duration);

    // Calculate server-side metrics
    let total_rps = total_summary.total_requests as f64 / duration.as_secs_f64();
    println!("\nServer Performance:");
    println!("  Total throughput: {total_rps:.2} requests/sec across all transports");
    println!(
        "  Average per transport: {:.2} requests/sec",
        total_rps / 4.0
    );

    // Stop server
    server_process.kill().await?;

    Ok(())
}

pub async fn run_transport_isolation_test() -> Result<()> {
    println!("\n=== Transport Isolation Test ===");
    println!("Testing that rate limiting is shared across all transports\n");

    // Start server with all transports
    let mut cmd = tokio::process::Command::new("cargo");
    cmd.arg("run")
        .arg("--release")
        .arg("-p")
        .arg("throttlecrab-server")
        .arg("--")
        .arg("--http")
        .arg("--http-port")
        .arg("38080")
        .arg("--grpc")
        .arg("--grpc-port")
        .arg("38070")
        .arg("--native")
        .arg("--native-port")
        .arg("38071")
        .arg("--store")
        .arg("periodic")
        .arg("--log-level")
        .arg("info");

    let mut server_process = cmd.spawn()?;
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Create pooled clients for all transports
    let http_client = Arc::new(HttpClient::new(38080));
    let grpc_client = GrpcClient::new(38070).await?;
    let native_client = Arc::new(NativeClient::new(38071, 10));

    // Test with the same key across all transports
    let test_key = "shared_test_key".to_string();
    let mut limited_count = 0;
    let total_requests = 200;

    println!("Sending {total_requests} requests with key '{test_key}' across all transports");

    for i in 0..total_requests {
        let transport_idx = i % 3;
        let limited = match transport_idx {
            0 => http_client.test_request(test_key.clone()).await?,
            1 => grpc_client.clone().test_request(test_key.clone()).await?,
            2 => native_client.test_request(test_key.clone()).await?,
            _ => unreachable!(),
        };

        if limited {
            limited_count += 1;
        }
    }

    println!("\nResults:");
    println!("  Total requests: {total_requests}");
    println!(
        "  Rate limited: {} ({:.2}%)",
        limited_count,
        limited_count as f64 / total_requests as f64 * 100.0
    );

    if limited_count > 90 && limited_count < 110 {
        println!("✅ Rate limiting is correctly shared across transports");
    } else {
        println!("❌ Unexpected rate limiting behavior - limits may not be shared correctly");
    }

    server_process.kill().await?;

    Ok(())
}

#[cfg(test)]
mod tests {

    #[tokio::test]
    async fn test_multi_transport_quick() -> Result<()> {
        let workload = WorkloadConfig {
            pattern: WorkloadPattern::Steady { rps: 500 },
            duration: Duration::from_secs(5),
            key_space_size: 1000,
            key_pattern: KeyPattern::Random { pool_size: 1000 },
        };

        run_multi_transport_test(workload).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_transport_isolation() -> Result<()> {
        run_transport_isolation_test().await?;
        Ok(())
    }
}

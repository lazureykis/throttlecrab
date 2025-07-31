//! Basic example of using the throttlecrab client

use std::time::Duration;
use throttlecrab_client::{ClientBuilder, ThrottleCrabClient};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("throttlecrab_client=debug")
        .init();

    // Connect to throttlecrab server with default configuration
    println!("Connecting to throttlecrab server...");
    let client = ThrottleCrabClient::connect("127.0.0.1:9090").await?;
    println!("Connected! Pool size: {}", client.pool_size());

    // Example 1: Basic rate limit check
    println!("\n=== Basic Rate Limit Check ===");
    let response = client
        .check_rate_limit(
            "user:123", // key
            10,         // max burst
            100,        // count per period
            60,         // period in seconds
        )
        .await?;

    println!("Allowed: {}", response.allowed);
    println!("Limit: {}", response.limit);
    println!("Remaining: {}", response.remaining);
    println!("Retry after: {} seconds", response.retry_after);
    println!("Reset after: {} seconds", response.reset_after);

    // Example 2: Custom quantity
    println!("\n=== Rate Limit with Custom Quantity ===");
    let response = client
        .check_rate_limit_with_quantity(
            "api:bulk_operation",
            50,   // max burst
            1000, // count per period
            3600, // period (1 hour)
            10,   // quantity (consuming 10 tokens)
        )
        .await?;

    println!("Allowed: {}", response.allowed);
    println!("Remaining: {}", response.remaining);

    // Example 3: Using the builder for advanced configuration
    println!("\n=== Custom Client Configuration ===");
    let custom_client = ClientBuilder::new()
        .max_connections(20)
        .min_idle_connections(5)
        .connect_timeout(Duration::from_secs(10))
        .request_timeout(Duration::from_secs(2))
        .tcp_nodelay(true)
        .build("127.0.0.1:9090")
        .await?;

    println!("Custom client pool size: {}", custom_client.pool_size());
    println!(
        "Available connections: {}",
        custom_client.available_connections()
    );

    // Example 4: Concurrent requests
    println!("\n=== Concurrent Requests ===");
    let mut handles = vec![];

    for i in 0..5 {
        let client = client.clone();
        handles.push(tokio::spawn(async move {
            let response = client
                .check_rate_limit(format!("concurrent:test:{i}"), 100, 1000, 60)
                .await?;

            println!(
                "Request {}: allowed={}, remaining={}",
                i, response.allowed, response.remaining
            );

            Ok::<_, anyhow::Error>(())
        }));
    }

    // Wait for all concurrent requests
    for handle in handles {
        handle.await??;
    }

    println!("\nFinal pool size: {}", client.pool_size());
    Ok(())
}

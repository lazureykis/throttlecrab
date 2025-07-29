use throttlecrab::{RateLimiterActor, ThrottleRequest};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("throttlecrab=debug".parse()?),
        )
        .init();

    // Spawn the rate limiter actor
    let limiter = RateLimiterActor::spawn(10_000);

    // Test request
    let request = ThrottleRequest {
        key: "user:123".to_string(),
        max_burst: 15,
        count_per_period: 30,
        period: 60,
        quantity: 1,
    };

    println!("Testing rate limiter with redis-cell compatible API:");
    println!("Key: {}", request.key);
    println!("Burst: {}", request.max_burst);
    println!(
        "Rate: {} per {} seconds",
        request.count_per_period, request.period
    );
    println!();

    // Make a few requests
    for i in 1..=20 {
        let response = limiter.throttle(request.clone()).await?;

        println!(
            "Request #{}: {} (remaining: {}/{}, retry_after: {}s, reset_after: {}s)",
            i,
            if response.allowed {
                "ALLOWED"
            } else {
                "BLOCKED"
            },
            response.remaining,
            response.limit,
            response.retry_after,
            response.reset_after,
        );

        // If blocked, wait before retrying
        if !response.allowed && response.retry_after > 0 {
            println!(
                "  -> Rate limited! Waiting {}s before continuing...",
                response.retry_after
            );
            tokio::time::sleep(tokio::time::Duration::from_secs(
                response.retry_after as u64,
            ))
            .await;
        }
    }

    Ok(())
}

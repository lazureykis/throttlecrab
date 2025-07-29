mod actor;
mod transport;
mod types;

#[cfg(test)]
mod actor_tests;

use anyhow::Result;
use clap::Parser;

use crate::actor::RateLimiterActor;
use crate::transport::{Transport, msgpack::MsgPackTransport};
use crate::types::ThrottleRequest;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Run in server mode
    #[arg(long)]
    server: bool,

    /// Host to bind to (server mode)
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Port to bind to (server mode)
    #[arg(long, default_value = "9090")]
    port: u16,

    /// Channel buffer size
    #[arg(long, default_value = "10000")]
    buffer_size: usize,

    /// Run demo mode
    #[arg(long)]
    demo: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("throttlecrab=info".parse()?),
        )
        .init();

    let args = Args::parse();

    // Spawn the rate limiter actor
    let limiter = RateLimiterActor::spawn(args.buffer_size);

    if args.server {
        tracing::info!(
            "Starting ThrottleCrab server on {}:{}",
            args.host,
            args.port
        );

        let transport = MsgPackTransport::new(&args.host, args.port);
        transport.start(limiter).await?;
    } else if args.demo {
        run_demo(limiter).await?;
    } else {
        println!("ThrottleCrab - High-performance rate limiter");
        println!();
        println!("Usage:");
        println!("  throttlecrab --server              Start server mode");
        println!("  throttlecrab --demo                Run demo");
        println!();
        println!("Server options:");
        println!("  --host <HOST>                      Host to bind to [default: 127.0.0.1]");
        println!("  --port <PORT>                      Port to bind to [default: 9090]");
        println!("  --buffer-size <SIZE>               Channel buffer size [default: 10000]");
    }

    Ok(())
}

async fn run_demo(limiter: actor::RateLimiterHandle) -> Result<()> {
    println!("Running ThrottleCrab demo...");
    println!();

    // Test request
    let mut request = ThrottleRequest {
        key: "user:123".to_string(),
        max_burst: 15,
        count_per_period: 30,
        period: 60,
        quantity: 1,
        timestamp: std::time::SystemTime::now(),
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
        // Update timestamp for each request
        request.timestamp = std::time::SystemTime::now();
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

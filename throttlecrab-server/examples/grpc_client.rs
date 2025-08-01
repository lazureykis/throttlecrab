// Include the generated protobuf code
pub mod throttlecrab_proto {
    tonic::include_proto!("throttlecrab");
}

use throttlecrab_proto::ThrottleRequest;
use throttlecrab_proto::rate_limiter_client::RateLimiterClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to the gRPC server
    let mut client = RateLimiterClient::connect("http://127.0.0.1:9090").await?;

    println!("Connected to gRPC server at 127.0.0.1:9090");
    println!();

    // Test with multiple requests
    for i in 1..=20 {
        let request = tonic::Request::new(ThrottleRequest {
            key: "user:123".to_string(),
            max_burst: 15,
            count_per_period: 30,
            period: 60,
            quantity: 1,
        });

        let response = client.throttle(request).await?;
        let resp = response.into_inner();

        println!(
            "Request #{}: {} (remaining: {}/{}, retry_after: {}s, reset_after: {}s)",
            i,
            if resp.allowed { "ALLOWED" } else { "BLOCKED" },
            resp.remaining,
            resp.limit,
            resp.retry_after,
            resp.reset_after,
        );

        // If blocked, wait before retrying
        if !resp.allowed && resp.retry_after > 0 {
            println!(
                "  -> Rate limited! Waiting {}s before continuing...",
                resp.retry_after
            );
            tokio::time::sleep(tokio::time::Duration::from_secs(resp.retry_after as u64)).await;
        }
    }

    Ok(())
}

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize)]
struct HttpThrottleRequest {
    key: String,
    max_burst: i64,
    count_per_period: i64,
    period: i64,
    quantity: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct ThrottleResponse {
    allowed: bool,
    limit: i64,
    remaining: i64,
    #[allow(dead_code)]
    reset_after: i64,
    retry_after: i64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    let base_url = "http://127.0.0.1:9090";

    // Test basic rate limiting
    println!("Testing HTTP rate limiter...");
    println!();

    let request = HttpThrottleRequest {
        key: "user:456".to_string(),
        max_burst: 10,
        count_per_period: 20,
        period: 60,
        quantity: Some(1),
    };

    println!("Rate limit config:");
    println!("  Key: {}", request.key);
    println!("  Burst: {}", request.max_burst);
    println!(
        "  Rate: {} per {}s",
        request.count_per_period, request.period
    );
    println!();

    // Make several requests
    for i in 1..=15 {
        let response = client
            .post(format!("{base_url}/throttle"))
            .json(&request)
            .send()
            .await?;

        if response.status().is_success() {
            let throttle_response: ThrottleResponse = response.json().await?;

            println!(
                "Request #{}: {} (remaining: {}/{}, retry_after: {}s)",
                i,
                if throttle_response.allowed {
                    "ALLOWED"
                } else {
                    "BLOCKED"
                },
                throttle_response.remaining,
                throttle_response.limit,
                throttle_response.retry_after
            );

            if !throttle_response.allowed && throttle_response.retry_after > 0 {
                println!(
                    "  -> Waiting {}s before next request...",
                    throttle_response.retry_after
                );
                tokio::time::sleep(Duration::from_secs(throttle_response.retry_after as u64)).await;
            }
        } else {
            eprintln!("Request failed with status: {}", response.status());
            let error_text = response.text().await?;
            eprintln!("Error: {error_text}");
        }
    }

    Ok(())
}

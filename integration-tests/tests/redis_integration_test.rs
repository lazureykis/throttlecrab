//! Integration tests for Redis transport using real Redis client

use redis::{Client, Cmd, Value};
use std::process::Command;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_redis_throttle_command() {
    // Start the server
    let mut server = Command::new("cargo")
        .args([
            "run",
            "-p",
            "throttlecrab-server",
            "--",
            "--redis",
            "--redis-port",
            "6380",
        ])
        .spawn()
        .expect("Failed to start server");

    // Give server time to start
    sleep(Duration::from_secs(2)).await;

    // Connect to the server
    let client = Client::open("redis://127.0.0.1:6380/").expect("Failed to create client");
    let mut con = client
        .get_multiplexed_tokio_connection()
        .await
        .expect("Failed to connect");

    // Test THROTTLE command
    let mut cmd = Cmd::new();
    cmd.arg("THROTTLE")
        .arg("test_key")
        .arg(10)  // max_burst
        .arg(100) // count_per_period
        .arg(60)  // period
        .arg(1); // quantity

    let result: Value = cmd
        .query_async(&mut con)
        .await
        .expect("Failed to execute command");

    // Verify response format
    match result {
        Value::Array(values) => {
            assert_eq!(values.len(), 5, "Expected 5 elements in response");

            // Check allowed (should be 1)
            assert_eq!(values[0], Value::Int(1), "Expected allowed = 1");

            // Check limit
            assert_eq!(values[1], Value::Int(10), "Expected limit = 10");

            // Check remaining (should be 9 after consuming 1)
            assert_eq!(values[2], Value::Int(9), "Expected remaining = 9");

            // Check reset_after (should be positive)
            if let Value::Int(reset_after) = &values[3] {
                assert!(*reset_after > 0, "Expected positive reset_after");
            } else {
                panic!("Expected integer for reset_after");
            }

            // Check retry_after (should be 0 when allowed)
            assert_eq!(values[4], Value::Int(0), "Expected retry_after = 0");
        }
        _ => panic!("Expected array response from THROTTLE command"),
    }

    // Test PING command
    let mut ping_cmd = Cmd::new();
    ping_cmd.arg("PING");
    let ping_result: String = ping_cmd
        .query_async(&mut con)
        .await
        .expect("Failed to PING");
    assert_eq!(ping_result, "PONG");

    // Test QUIT command
    let mut quit_cmd = Cmd::new();
    quit_cmd.arg("QUIT");
    let _: String = quit_cmd
        .query_async(&mut con)
        .await
        .expect("Failed to QUIT");

    // Kill the server and wait for it to exit
    server.kill().expect("Failed to kill server");
    let _ = server.wait().expect("Failed to wait for server to exit");
}

#[tokio::test]
async fn test_redis_rate_limiting() {
    // Start the server
    let mut server = Command::new("cargo")
        .args([
            "run",
            "-p",
            "throttlecrab-server",
            "--",
            "--redis",
            "--redis-port",
            "6381",
        ])
        .spawn()
        .expect("Failed to start server");

    // Give server time to start
    sleep(Duration::from_secs(2)).await;

    // Connect to the server
    let client = Client::open("redis://127.0.0.1:6381/").expect("Failed to create client");
    let mut con = client
        .get_multiplexed_tokio_connection()
        .await
        .expect("Failed to connect");

    // Use a small burst limit to test rate limiting
    let key = "rate_limit_test";
    let max_burst = 3;

    // Make requests until we hit the limit
    let mut allowed_count = 0;
    let mut denied_count = 0;

    for _ in 0..5 {
        let mut cmd = Cmd::new();
        cmd.arg("THROTTLE")
            .arg(key)
            .arg(max_burst)
            .arg(100)
            .arg(60)
            .arg(1);

        let result: Value = cmd
            .query_async(&mut con)
            .await
            .expect("Failed to execute command");

        if let Value::Array(values) = result {
            if let Value::Int(allowed) = &values[0] {
                if *allowed == 1 {
                    allowed_count += 1;
                } else {
                    denied_count += 1;
                }
            }
        }
    }

    // Should have 3 allowed and 2 denied
    assert_eq!(allowed_count, 3, "Expected 3 requests to be allowed");
    assert_eq!(denied_count, 2, "Expected 2 requests to be denied");

    // Kill the server and wait for it to exit
    server.kill().expect("Failed to kill server");
    let _ = server.wait().expect("Failed to wait for server to exit");
}

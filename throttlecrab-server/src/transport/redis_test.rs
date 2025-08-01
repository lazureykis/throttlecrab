//! Tests for Redis protocol transport

use super::redis::resp::{RespParser, RespSerializer, RespValue};
use crate::actor::RateLimiterHandle;
use crate::config::StoreType;
use crate::metrics::Metrics;
use crate::store;
use std::sync::Arc;

// Helper function to create a new rate limiter for each test
fn create_test_rate_limiter() -> (RateLimiterHandle, Arc<Metrics>) {
    let metrics = Arc::new(Metrics::new());
    let store_config = crate::config::StoreConfig {
        store_type: StoreType::Periodic,
        capacity: 10000,
        cleanup_interval: 300,
        cleanup_probability: 10000,
        min_interval: 5,
        max_interval: 300,
        max_operations: 1000000,
    };
    let handle = store::create_rate_limiter(&store_config, 10000, metrics.clone());
    (handle, metrics)
}

// Helper to create a THROTTLE command
fn create_throttle_cmd(
    key: &str,
    max_burst: i64,
    count_per_period: i64,
    period: i64,
    quantity: Option<i64>,
) -> RespValue {
    let mut args = vec![
        RespValue::BulkString(Some("THROTTLE".to_string())),
        RespValue::BulkString(Some(key.to_string())),
        RespValue::BulkString(Some(max_burst.to_string())),
        RespValue::BulkString(Some(count_per_period.to_string())),
        RespValue::BulkString(Some(period.to_string())),
    ];
    if let Some(q) = quantity {
        args.push(RespValue::BulkString(Some(q.to_string())));
    }
    RespValue::Array(args)
}

// Helper to create a PING command
fn create_ping_cmd(message: Option<&str>) -> RespValue {
    let mut args = vec![RespValue::BulkString(Some("PING".to_string()))];
    if let Some(msg) = message {
        args.push(RespValue::BulkString(Some(msg.to_string())));
    }
    RespValue::Array(args)
}

// Helper to get throttle response fields
struct ThrottleResponse {
    allowed: bool,
    limit: i64,
    remaining: i64,
    reset_after: i64,
    retry_after: i64,
}

impl ThrottleResponse {
    fn from_resp(response: &RespValue) -> Self {
        match response {
            RespValue::Array(values) => {
                assert_eq!(values.len(), 5, "Throttle response should have 5 elements");
                Self {
                    allowed: match &values[0] {
                        RespValue::Integer(n) => *n == 1,
                        _ => panic!("Expected integer for allowed field"),
                    },
                    limit: match &values[1] {
                        RespValue::Integer(n) => *n,
                        _ => panic!("Expected integer for limit field"),
                    },
                    remaining: match &values[2] {
                        RespValue::Integer(n) => *n,
                        _ => panic!("Expected integer for remaining field"),
                    },
                    reset_after: match &values[3] {
                        RespValue::Integer(n) => *n,
                        _ => panic!("Expected integer for reset_after field"),
                    },
                    retry_after: match &values[4] {
                        RespValue::Integer(n) => *n,
                        _ => panic!("Expected integer for retry_after field"),
                    },
                }
            }
            _ => panic!("Expected array response for throttle command"),
        }
    }
}

#[tokio::test]
async fn test_redis_ping() {
    let (handle, metrics) = create_test_rate_limiter();

    let ping_cmd = create_ping_cmd(None);
    let response = process_command(ping_cmd, &handle, &metrics).await;
    assert_eq!(response, RespValue::SimpleString("PONG".to_string()));
}

#[tokio::test]
async fn test_redis_ping_with_message() {
    let (handle, metrics) = create_test_rate_limiter();

    let ping_cmd = create_ping_cmd(Some("hello"));
    let response = process_command(ping_cmd, &handle, &metrics).await;
    assert_eq!(response, RespValue::BulkString(Some("hello".to_string())));
}

#[tokio::test]
async fn test_redis_throttle_allowed() {
    let (handle, metrics) = create_test_rate_limiter();

    let throttle_cmd = create_throttle_cmd("test_key", 10, 100, 60, None);
    let response = process_command(throttle_cmd, &handle, &metrics).await;

    let throttle_resp = ThrottleResponse::from_resp(&response);
    assert!(throttle_resp.allowed);
    assert_eq!(throttle_resp.limit, 10);
    assert_eq!(throttle_resp.remaining, 9);
    assert_eq!(throttle_resp.reset_after, 5);
    assert_eq!(throttle_resp.retry_after, 0);
}

#[tokio::test]
async fn test_redis_throttle_with_quantity() {
    let (handle, metrics) = create_test_rate_limiter();

    let throttle_cmd = create_throttle_cmd("test_key2", 10, 100, 60, Some(5));
    let response = process_command(throttle_cmd, &handle, &metrics).await;

    let throttle_resp = ThrottleResponse::from_resp(&response);
    assert!(throttle_resp.allowed);
    assert_eq!(throttle_resp.limit, 10);
    assert_eq!(throttle_resp.remaining, 5); // 10-5
    assert_eq!(throttle_resp.reset_after, 7);
    assert_eq!(throttle_resp.retry_after, 0);
}

#[tokio::test]
async fn test_redis_unknown_command() {
    let (handle, metrics) = create_test_rate_limiter();

    let unknown_cmd = create_invalid_cmd("UNKNOWN", vec![]);
    let response = process_command(unknown_cmd, &handle, &metrics).await;
    assert_error_response(&response, "unknown command");
}

#[tokio::test]
async fn test_redis_invalid_throttle_args() {
    let (handle, metrics) = create_test_rate_limiter();

    // Too few arguments
    let throttle_cmd = create_invalid_cmd("THROTTLE", vec!["test_key"]);
    let response = process_command(throttle_cmd, &handle, &metrics).await;
    assert_error_response(&response, "wrong number of arguments");
}

#[tokio::test]
async fn test_resp_parser_partial_data() {
    let mut parser = RespParser::new();

    // Partial simple string
    let data1 = b"+OK";
    assert_eq!(parser.parse(data1).unwrap(), None);

    // Complete it
    let data2 = b"+OK\r\n";
    assert_eq!(
        parser.parse(data2).unwrap(),
        Some((RespValue::SimpleString("OK".to_string()), 5))
    );

    // Partial bulk string
    let data3 = b"$6\r\nfoo";
    assert_eq!(parser.parse(data3).unwrap(), None);

    // Complete bulk string
    let data4 = b"$6\r\nfoobar\r\n";
    assert_eq!(
        parser.parse(data4).unwrap(),
        Some((RespValue::BulkString(Some("foobar".to_string())), 12))
    );
}

#[tokio::test]
async fn test_resp_serializer_roundtrip() {
    let mut parser = RespParser::new();

    // Test various RESP values
    let values = vec![
        RespValue::SimpleString("OK".to_string()),
        RespValue::Error("ERR something".to_string()),
        RespValue::Integer(42),
        RespValue::BulkString(Some("hello world".to_string())),
        RespValue::BulkString(None),
        RespValue::Array(vec![
            RespValue::BulkString(Some("foo".to_string())),
            RespValue::Integer(123),
            RespValue::SimpleString("bar".to_string()),
        ]),
    ];

    for value in values {
        let serialized = RespSerializer::serialize(&value);
        let (parsed, consumed) = parser.parse(&serialized).unwrap().unwrap();
        assert_eq!(parsed, value);
        assert_eq!(consumed, serialized.len());
    }
}

#[tokio::test]
async fn test_resp_edge_cases() {
    let mut parser = RespParser::new();

    // Empty array
    let data = b"*0\r\n";
    assert_eq!(
        parser.parse(data).unwrap(),
        Some((RespValue::Array(vec![]), 4))
    );

    // Nested arrays
    let nested = RespValue::Array(vec![
        RespValue::Array(vec![RespValue::Integer(1), RespValue::Integer(2)]),
        RespValue::BulkString(Some("test".to_string())),
    ]);

    let serialized = RespSerializer::serialize(&nested);
    let (parsed, _) = parser.parse(&serialized).unwrap().unwrap();
    assert_eq!(parsed, nested);
}

// Helper to create invalid command
fn create_invalid_cmd(cmd: &str, args: Vec<&str>) -> RespValue {
    let mut cmd_args = vec![RespValue::BulkString(Some(cmd.to_string()))];
    for arg in args {
        cmd_args.push(RespValue::BulkString(Some(arg.to_string())));
    }
    RespValue::Array(cmd_args)
}

// Helper to assert error response
fn assert_error_response(response: &RespValue, expected_error_substring: &str) {
    match response {
        RespValue::Error(msg) => {
            assert!(
                msg.contains(expected_error_substring),
                "Expected error containing '{expected_error_substring}', got '{msg}'"
            );
        }
        _ => panic!("Expected error response"),
    }
}

// Helper function to test actual commands (exposed for testing)
async fn process_command(
    value: RespValue,
    limiter: &RateLimiterHandle,
    metrics: &Arc<Metrics>,
) -> RespValue {
    super::redis::process_command(value, limiter, metrics).await
}

#[tokio::test]
async fn test_redis_throttle_exhaustion() {
    let (handle, metrics) = create_test_rate_limiter();

    let key = "exhaustion_test";
    let max_burst = 3;
    let throttle_cmd = create_throttle_cmd(key, max_burst, 100, 60, None);

    // First request should succeed
    let response = process_command(throttle_cmd.clone(), &handle, &metrics).await;
    let resp = ThrottleResponse::from_resp(&response);
    assert!(resp.allowed);
    assert_eq!(resp.limit, 3);
    assert_eq!(resp.remaining, 2);

    // Second request should succeed
    let response = process_command(throttle_cmd.clone(), &handle, &metrics).await;
    let resp = ThrottleResponse::from_resp(&response);
    assert!(resp.allowed);
    assert_eq!(resp.remaining, 1);

    // Third request should succeed
    let response = process_command(throttle_cmd.clone(), &handle, &metrics).await;
    let resp = ThrottleResponse::from_resp(&response);
    assert!(resp.allowed);
    assert_eq!(resp.remaining, 0);

    // Fourth request should be denied
    let response = process_command(throttle_cmd, &handle, &metrics).await;
    let resp = ThrottleResponse::from_resp(&response);
    assert!(!resp.allowed);
    assert_eq!(resp.remaining, 0);
    assert!(resp.retry_after >= 0, "retry_after should be non-negative");
}

#[tokio::test]
async fn test_redis_multiple_keys() {
    let (handle, metrics) = create_test_rate_limiter();

    // Test with three different keys
    let keys = vec!["user:123", "user:456", "api:endpoint"];

    for key in &keys {
        let throttle_cmd = create_throttle_cmd(key, 5, 100, 60, None);
        let response = process_command(throttle_cmd, &handle, &metrics).await;
        let resp = ThrottleResponse::from_resp(&response);
        assert!(resp.allowed);
        assert_eq!(resp.limit, 5);
        assert_eq!(resp.remaining, 4);
    }

    // Verify each key maintains its own limit
    for key in &keys {
        let throttle_cmd = create_throttle_cmd(key, 5, 100, 60, None);
        let response = process_command(throttle_cmd, &handle, &metrics).await;
        let resp = ThrottleResponse::from_resp(&response);
        assert!(resp.allowed);
        assert_eq!(resp.remaining, 3); // each has 3 remaining
    }
}

#[tokio::test]
async fn test_redis_different_limits_same_key() {
    let (handle, metrics) = create_test_rate_limiter();

    let key = "dynamic_limit_key";

    // First request with limit of 10
    let throttle_cmd = RespValue::Array(vec![
        RespValue::BulkString(Some("THROTTLE".to_string())),
        RespValue::BulkString(Some(key.to_string())),
        RespValue::BulkString(Some("10".to_string())),
        RespValue::BulkString(Some("100".to_string())),
        RespValue::BulkString(Some("60".to_string())),
    ]);

    let response = process_command(throttle_cmd, &handle, &metrics).await;
    match response {
        RespValue::Array(values) => {
            assert_eq!(values[0], RespValue::Integer(1)); // allowed
            assert_eq!(values[1], RespValue::Integer(10)); // limit
            assert_eq!(values[2], RespValue::Integer(9)); // remaining
        }
        _ => panic!("Expected array response"),
    }

    // Same key but with different limit
    let throttle_cmd = RespValue::Array(vec![
        RespValue::BulkString(Some("THROTTLE".to_string())),
        RespValue::BulkString(Some(key.to_string())),
        RespValue::BulkString(Some("5".to_string())), // smaller limit
        RespValue::BulkString(Some("100".to_string())),
        RespValue::BulkString(Some("60".to_string())),
    ]);

    let response = process_command(throttle_cmd, &handle, &metrics).await;
    match response {
        RespValue::Array(values) => {
            assert_eq!(values[0], RespValue::Integer(1)); // allowed
            assert_eq!(values[1], RespValue::Integer(5)); // new limit
            // The remaining count depends on implementation - just verify it's within bounds
            if let RespValue::Integer(remaining) = &values[2] {
                assert!(
                    *remaining >= 0 && *remaining <= 5,
                    "remaining should be between 0 and 5"
                );
            }
        }
        _ => panic!("Expected array response"),
    }
}

#[tokio::test]
async fn test_redis_large_quantity() {
    let (handle, metrics) = create_test_rate_limiter();

    // Request with quantity larger than limit
    let throttle_cmd = create_throttle_cmd("large_quantity_key", 10, 100, 60, Some(15));
    let response = process_command(throttle_cmd, &handle, &metrics).await;

    let resp = ThrottleResponse::from_resp(&response);
    assert!(!resp.allowed); // denied
    assert_eq!(resp.limit, 10);
    assert_eq!(resp.remaining, 10); // remaining unchanged
}

#[tokio::test]
async fn test_redis_special_characters_in_key() {
    let (handle, metrics) = create_test_rate_limiter();

    // Test keys with special characters
    let special_keys = vec![
        "user:email@example.com",
        "api:v2/users/{id}",
        "rate:limit:user-123",
        "key with spaces",
        "key:with:colons:everywhere",
        "UTF8:测试键",
    ];

    for key in special_keys {
        let throttle_cmd = create_throttle_cmd(key, 5, 100, 60, None);
        let response = process_command(throttle_cmd, &handle, &metrics).await;
        let resp = ThrottleResponse::from_resp(&response);
        assert!(resp.allowed, "Failed for key: {key}");
        assert_eq!(resp.limit, 5);
        assert_eq!(resp.remaining, 4);
    }
}

#[tokio::test]
async fn test_redis_mixed_commands() {
    let (handle, metrics) = create_test_rate_limiter();

    // PING
    let ping_cmd = RespValue::Array(vec![RespValue::BulkString(Some("PING".to_string()))]);
    let response = process_command(ping_cmd, &handle, &metrics).await;
    assert_eq!(response, RespValue::SimpleString("PONG".to_string()));

    // THROTTLE
    let throttle_cmd = RespValue::Array(vec![
        RespValue::BulkString(Some("THROTTLE".to_string())),
        RespValue::BulkString(Some("mixed_key".to_string())),
        RespValue::BulkString(Some("10".to_string())),
        RespValue::BulkString(Some("100".to_string())),
        RespValue::BulkString(Some("60".to_string())),
    ]);
    let response = process_command(throttle_cmd, &handle, &metrics).await;
    match response {
        RespValue::Array(values) => {
            assert_eq!(values[0], RespValue::Integer(1));
        }
        _ => panic!("Expected array response"),
    }

    // PING with message
    let ping_cmd = RespValue::Array(vec![
        RespValue::BulkString(Some("PING".to_string())),
        RespValue::BulkString(Some("test message".to_string())),
    ]);
    let response = process_command(ping_cmd, &handle, &metrics).await;
    assert_eq!(
        response,
        RespValue::BulkString(Some("test message".to_string()))
    );

    // Another THROTTLE
    let throttle_cmd = RespValue::Array(vec![
        RespValue::BulkString(Some("throttle".to_string())), // lowercase
        RespValue::BulkString(Some("mixed_key".to_string())),
        RespValue::BulkString(Some("10".to_string())),
        RespValue::BulkString(Some("100".to_string())),
        RespValue::BulkString(Some("60".to_string())),
    ]);
    let response = process_command(throttle_cmd, &handle, &metrics).await;
    match response {
        RespValue::Array(values) => {
            assert_eq!(values[0], RespValue::Integer(1));
            assert_eq!(values[2], RespValue::Integer(8)); // one less remaining
        }
        _ => panic!("Expected array response"),
    }
}

#[tokio::test]
async fn test_redis_invalid_numeric_args() {
    let (handle, metrics) = create_test_rate_limiter();

    // Test invalid max_burst
    let throttle_cmd =
        create_invalid_cmd("THROTTLE", vec!["test_key", "not_a_number", "100", "60"]);
    let response = process_command(throttle_cmd, &handle, &metrics).await;
    assert_error_response(&response, "invalid max_burst");

    // Test negative values
    let throttle_cmd = create_invalid_cmd("THROTTLE", vec!["test_key", "-5", "100", "60"]);
    let response = process_command(throttle_cmd, &handle, &metrics).await;
    assert_error_response(&response, "ERR");
}

#[tokio::test]
async fn test_redis_zero_quantity() {
    let (handle, metrics) = create_test_rate_limiter();

    // Request with zero quantity
    let throttle_cmd = create_throttle_cmd("zero_quantity_key", 10, 100, 60, Some(0));
    let response = process_command(throttle_cmd, &handle, &metrics).await;

    let resp = ThrottleResponse::from_resp(&response);
    assert!(resp.allowed); // zero quantity always succeeds
    assert_eq!(resp.remaining, 10); // remaining unchanged
}

#[tokio::test]
async fn test_resp_parser_multiple_commands() {
    let mut parser = RespParser::new();

    // Simulate multiple commands in buffer
    let mut data = Vec::new();
    data.extend_from_slice(b"*1\r\n$4\r\nPING\r\n");
    data.extend_from_slice(b"*2\r\n$4\r\nPING\r\n$5\r\nhello\r\n");

    // Parse first command
    let (cmd1, consumed1) = parser.parse(&data).unwrap().unwrap();
    assert_eq!(
        cmd1,
        RespValue::Array(vec![RespValue::BulkString(Some("PING".to_string()))])
    );

    // Remove consumed data
    data.drain(..consumed1);

    // Parse second command
    let (cmd2, consumed2) = parser.parse(&data).unwrap().unwrap();
    assert_eq!(
        cmd2,
        RespValue::Array(vec![
            RespValue::BulkString(Some("PING".to_string())),
            RespValue::BulkString(Some("hello".to_string())),
        ])
    );
    assert_eq!(consumed2, data.len());
}

#[tokio::test]
async fn test_resp_integer_args() {
    let mut parser = RespParser::new();

    // Test command with integer arguments (not bulk strings)
    let data = b"*5\r\n$8\r\nTHROTTLE\r\n$8\r\ntest_key\r\n:10\r\n:100\r\n:60\r\n";
    let (cmd, _) = parser.parse(data).unwrap().unwrap();

    assert_eq!(
        cmd,
        RespValue::Array(vec![
            RespValue::BulkString(Some("THROTTLE".to_string())),
            RespValue::BulkString(Some("test_key".to_string())),
            RespValue::Integer(10),
            RespValue::Integer(100),
            RespValue::Integer(60),
        ])
    );
}

#[tokio::test]
async fn test_redis_concurrent_same_key() {
    let (handle, metrics) = create_test_rate_limiter();

    let key = "concurrent_key";
    let max_burst = 10;

    // Spawn multiple concurrent requests
    let mut handles = vec![];
    for _ in 0..5 {
        let handle_clone = handle.clone();
        let metrics_clone = metrics.clone();
        let key_clone = key.to_string();

        let task = tokio::spawn(async move {
            let throttle_cmd = RespValue::Array(vec![
                RespValue::BulkString(Some("THROTTLE".to_string())),
                RespValue::BulkString(Some(key_clone)),
                RespValue::BulkString(Some(max_burst.to_string())),
                RespValue::BulkString(Some("100".to_string())),
                RespValue::BulkString(Some("60".to_string())),
            ]);

            process_command(throttle_cmd, &handle_clone, &metrics_clone).await
        });
        handles.push(task);
    }

    // Wait for all requests to complete
    let mut results = Vec::new();
    for handle in handles {
        results.push(handle.await.unwrap());
    }

    // All requests should be allowed (we have 10 burst, requesting 5)
    let mut allowed_count = 0;
    for result in &results {
        match result {
            RespValue::Array(values) => {
                assert_eq!(values[0], RespValue::Integer(1)); // all should be allowed
                assert_eq!(values[1], RespValue::Integer(max_burst)); // limit
                allowed_count += 1;
            }
            _ => panic!("Expected array response"),
        }
    }

    // All 5 concurrent requests should be allowed
    assert_eq!(allowed_count, 5);
}

#[tokio::test]
async fn test_redis_rapid_succession() {
    let (handle, metrics) = create_test_rate_limiter();

    let key = "rapid_key";
    let max_burst = 5;
    let mut allowed_count = 0;
    let mut denied_count = 0;

    // Send 10 requests in rapid succession (should exhaust limit)
    for _ in 0..10 {
        let throttle_cmd = create_throttle_cmd(key, max_burst, 100, 60, None);
        let response = process_command(throttle_cmd, &handle, &metrics).await;
        let resp = ThrottleResponse::from_resp(&response);

        if resp.allowed {
            allowed_count += 1;
        } else {
            denied_count += 1;
        }
    }

    assert_eq!(allowed_count, 5, "Expected 5 requests to be allowed");
    assert_eq!(denied_count, 5, "Expected 5 requests to be denied");
}

#[tokio::test]
async fn test_redis_empty_key() {
    let (handle, metrics) = create_test_rate_limiter();

    // Test with empty key
    let throttle_cmd = RespValue::Array(vec![
        RespValue::BulkString(Some("THROTTLE".to_string())),
        RespValue::BulkString(Some("".to_string())), // empty key
        RespValue::BulkString(Some("10".to_string())),
        RespValue::BulkString(Some("100".to_string())),
        RespValue::BulkString(Some("60".to_string())),
    ]);

    let response = process_command(throttle_cmd, &handle, &metrics).await;
    // Empty key should still work
    match response {
        RespValue::Array(values) => {
            assert_eq!(values[0], RespValue::Integer(1)); // allowed
            assert_eq!(values[1], RespValue::Integer(10)); // limit
            assert_eq!(values[2], RespValue::Integer(9)); // remaining
        }
        _ => panic!("Expected array response"),
    }
}

#[tokio::test]
async fn test_redis_null_args() {
    let (handle, metrics) = create_test_rate_limiter();

    // Test with null bulk string
    let throttle_cmd = RespValue::Array(vec![
        RespValue::BulkString(Some("THROTTLE".to_string())),
        RespValue::BulkString(None), // null key
        RespValue::BulkString(Some("10".to_string())),
        RespValue::BulkString(Some("100".to_string())),
        RespValue::BulkString(Some("60".to_string())),
    ]);

    let response = process_command(throttle_cmd, &handle, &metrics).await;
    match response {
        RespValue::Error(msg) => assert!(msg.contains("invalid key")),
        _ => panic!("Expected error response for null key"),
    }
}

#[tokio::test]
async fn test_redis_boundary_values() {
    let (handle, metrics) = create_test_rate_limiter();

    // Test with max i64 values
    let throttle_cmd = RespValue::Array(vec![
        RespValue::BulkString(Some("THROTTLE".to_string())),
        RespValue::BulkString(Some("boundary_key".to_string())),
        RespValue::BulkString(Some(i64::MAX.to_string())),
        RespValue::BulkString(Some(i64::MAX.to_string())),
        RespValue::BulkString(Some(i64::MAX.to_string())),
    ]);

    let response = process_command(throttle_cmd, &handle, &metrics).await;
    match response {
        RespValue::Array(values) => {
            assert_eq!(values[0], RespValue::Integer(1)); // allowed
            assert_eq!(values[1], RespValue::Integer(i64::MAX)); // limit
        }
        _ => panic!("Expected array response"),
    }

    // Test with very small values
    let throttle_cmd = RespValue::Array(vec![
        RespValue::BulkString(Some("THROTTLE".to_string())),
        RespValue::BulkString(Some("tiny_key".to_string())),
        RespValue::BulkString(Some("1".to_string())),
        RespValue::BulkString(Some("1".to_string())),
        RespValue::BulkString(Some("1".to_string())),
    ]);

    let response = process_command(throttle_cmd, &handle, &metrics).await;
    match response {
        RespValue::Array(values) => {
            assert_eq!(values[0], RespValue::Integer(1)); // allowed
            assert_eq!(values[1], RespValue::Integer(1)); // limit
            assert_eq!(values[2], RespValue::Integer(0)); // remaining
        }
        _ => panic!("Expected array response"),
    }
}

#[tokio::test]
async fn test_redis_command_case_insensitive() {
    let (handle, metrics) = create_test_rate_limiter();

    // Test various case combinations
    let commands = vec!["ping", "PING", "Ping", "PiNg"];

    for cmd in commands {
        let ping_cmd = RespValue::Array(vec![RespValue::BulkString(Some(cmd.to_string()))]);

        let response = process_command(ping_cmd, &handle, &metrics).await;
        assert_eq!(
            response,
            RespValue::SimpleString("PONG".to_string()),
            "Failed for command: {cmd}"
        );
    }

    // Test throttle command case variations
    let throttle_commands = vec!["throttle", "THROTTLE", "Throttle", "ThRoTtLe"];

    for cmd in throttle_commands {
        let throttle_cmd = RespValue::Array(vec![
            RespValue::BulkString(Some(cmd.to_string())),
            RespValue::BulkString(Some("case_test_key".to_string())),
            RespValue::BulkString(Some("10".to_string())),
            RespValue::BulkString(Some("100".to_string())),
            RespValue::BulkString(Some("60".to_string())),
        ]);

        let response = process_command(throttle_cmd, &handle, &metrics).await;
        match response {
            RespValue::Array(values) => {
                assert_eq!(
                    values[0],
                    RespValue::Integer(1),
                    "Failed for command: {cmd}"
                );
            }
            _ => panic!("Expected array response for command: {cmd}"),
        }
    }
}

#[tokio::test]
async fn test_redis_very_long_key() {
    let (handle, metrics) = create_test_rate_limiter();

    // Test with a very long key (1000 characters)
    let long_key = "x".repeat(1000);

    let throttle_cmd = RespValue::Array(vec![
        RespValue::BulkString(Some("THROTTLE".to_string())),
        RespValue::BulkString(Some(long_key.clone())),
        RespValue::BulkString(Some("10".to_string())),
        RespValue::BulkString(Some("100".to_string())),
        RespValue::BulkString(Some("60".to_string())),
    ]);

    let response = process_command(throttle_cmd, &handle, &metrics).await;
    match response {
        RespValue::Array(values) => {
            assert_eq!(values[0], RespValue::Integer(1)); // allowed
            assert_eq!(values[1], RespValue::Integer(10)); // limit
            assert_eq!(values[2], RespValue::Integer(9)); // remaining
        }
        _ => panic!("Expected array response"),
    }

    // Verify the same key works again
    let throttle_cmd = RespValue::Array(vec![
        RespValue::BulkString(Some("THROTTLE".to_string())),
        RespValue::BulkString(Some(long_key)),
        RespValue::BulkString(Some("10".to_string())),
        RespValue::BulkString(Some("100".to_string())),
        RespValue::BulkString(Some("60".to_string())),
    ]);

    let response = process_command(throttle_cmd, &handle, &metrics).await;
    match response {
        RespValue::Array(values) => {
            assert_eq!(values[0], RespValue::Integer(1)); // allowed
            assert_eq!(values[2], RespValue::Integer(8)); // one less remaining
        }
        _ => panic!("Expected array response"),
    }
}

//! Tests for Redis protocol transport

use super::redis::resp::{RespParser, RespSerializer, RespValue};
use crate::actor::RateLimiterHandle;
use crate::config::StoreType;
use crate::metrics::Metrics;
use crate::store;
use std::sync::Arc;

#[tokio::test]
async fn test_redis_ping() {
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

    // Simulate command processing
    let ping_cmd = RespValue::Array(vec![RespValue::BulkString(Some("PING".to_string()))]);

    let response = process_command(ping_cmd, &handle, &metrics).await;
    assert_eq!(response, RespValue::SimpleString("PONG".to_string()));
}

#[tokio::test]
async fn test_redis_ping_with_message() {
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

    let ping_cmd = RespValue::Array(vec![
        RespValue::BulkString(Some("PING".to_string())),
        RespValue::BulkString(Some("hello".to_string())),
    ]);

    let response = process_command(ping_cmd, &handle, &metrics).await;
    assert_eq!(response, RespValue::BulkString(Some("hello".to_string())));
}

#[tokio::test]
async fn test_redis_throttle_allowed() {
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

    let throttle_cmd = RespValue::Array(vec![
        RespValue::BulkString(Some("THROTTLE".to_string())),
        RespValue::BulkString(Some("test_key".to_string())),
        RespValue::BulkString(Some("10".to_string())),
        RespValue::BulkString(Some("100".to_string())),
        RespValue::BulkString(Some("60".to_string())),
    ]);

    let response = process_command(throttle_cmd, &handle, &metrics).await;

    match response {
        RespValue::Array(values) => {
            assert_eq!(values.len(), 5);
            assert_eq!(values[0], RespValue::Integer(1)); // allowed
            assert_eq!(values[1], RespValue::Integer(10)); // limit
            assert_eq!(values[2], RespValue::Integer(9)); // remaining
            // reset_after should be positive
            match &values[3] {
                RespValue::Integer(n) => assert!(
                    *n > 0 && *n <= 60,
                    "reset_after should be between 0 and 60, got {n}"
                ),
                _ => panic!("Expected integer for reset_after"),
            }
            assert_eq!(values[4], RespValue::Integer(0)); // retry_after
        }
        _ => panic!("Expected array response"),
    }
}

#[tokio::test]
async fn test_redis_throttle_with_quantity() {
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

    let throttle_cmd = RespValue::Array(vec![
        RespValue::BulkString(Some("THROTTLE".to_string())),
        RespValue::BulkString(Some("test_key2".to_string())),
        RespValue::BulkString(Some("10".to_string())),
        RespValue::BulkString(Some("100".to_string())),
        RespValue::BulkString(Some("60".to_string())),
        RespValue::BulkString(Some("5".to_string())), // quantity
    ]);

    let response = process_command(throttle_cmd, &handle, &metrics).await;

    match response {
        RespValue::Array(values) => {
            assert_eq!(values.len(), 5);
            assert_eq!(values[0], RespValue::Integer(1)); // allowed
            assert_eq!(values[1], RespValue::Integer(10)); // limit
            assert_eq!(values[2], RespValue::Integer(5)); // remaining (10-5)
            // reset_after should be positive
            match &values[3] {
                RespValue::Integer(n) => assert!(
                    *n > 0 && *n <= 60,
                    "reset_after should be between 0 and 60, got {n}"
                ),
                _ => panic!("Expected integer for reset_after"),
            }
            assert_eq!(values[4], RespValue::Integer(0)); // retry_after
        }
        _ => panic!("Expected array response"),
    }
}

#[tokio::test]
async fn test_redis_unknown_command() {
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

    let unknown_cmd = RespValue::Array(vec![RespValue::BulkString(Some("UNKNOWN".to_string()))]);

    let response = process_command(unknown_cmd, &handle, &metrics).await;

    match response {
        RespValue::Error(msg) => {
            assert!(msg.contains("unknown command"));
        }
        _ => panic!("Expected error response"),
    }
}

#[tokio::test]
async fn test_redis_invalid_throttle_args() {
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

    // Too few arguments
    let throttle_cmd = RespValue::Array(vec![
        RespValue::BulkString(Some("THROTTLE".to_string())),
        RespValue::BulkString(Some("test_key".to_string())),
    ]);

    let response = process_command(throttle_cmd, &handle, &metrics).await;

    match response {
        RespValue::Error(msg) => {
            assert!(msg.contains("wrong number of arguments"));
        }
        _ => panic!("Expected error response"),
    }
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

// Helper function to test actual commands (exposed for testing)
async fn process_command(
    value: RespValue,
    limiter: &RateLimiterHandle,
    metrics: &Arc<Metrics>,
) -> RespValue {
    super::redis::process_command(value, limiter, metrics).await
}

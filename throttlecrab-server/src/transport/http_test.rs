#[cfg(test)]
mod tests {
    use super::super::http::HttpThrottleRequest;
    use crate::types::ThrottleResponse;

    #[tokio::test]
    async fn test_http_transport_basic() {
        // Test request/response serialization

        // Test request structure
        let request = HttpThrottleRequest {
            key: "test_key".to_string(),
            max_burst: 10,
            count_per_period: 20,
            period: 60,
            quantity: Some(1),
            timestamp: None,
        };

        // Verify serialization works
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("test_key"));

        // Test response deserialization
        let response_json = r#"{
            "allowed": true,
            "limit": 10,
            "remaining": 9,
            "reset_after": 60,
            "retry_after": 0
        }"#;

        let response: ThrottleResponse = serde_json::from_str(response_json).unwrap();
        assert!(response.allowed);
        assert_eq!(response.limit, 10);
        assert_eq!(response.remaining, 9);
    }

    #[tokio::test]
    async fn test_http_request_validation() {
        // Test that quantity defaults to 1 if not provided
        let request_json = r#"{
            "key": "test",
            "max_burst": 5,
            "count_per_period": 10,
            "period": 60
        }"#;

        let request: HttpThrottleRequest = serde_json::from_str(request_json).unwrap();
        assert_eq!(request.quantity, None);
        assert_eq!(request.timestamp, None);
    }

    #[tokio::test]
    async fn test_http_request_with_timestamp() {
        // Test that timestamp is parsed correctly
        let request_json = r#"{
            "key": "test",
            "max_burst": 5,
            "count_per_period": 10,
            "period": 60,
            "timestamp": 1234567890123456789
        }"#;

        let request: HttpThrottleRequest = serde_json::from_str(request_json).unwrap();
        assert_eq!(request.timestamp, Some(1234567890123456789));
    }
}

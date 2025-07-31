//! gRPC transport for service mesh integration
//!
//! This transport provides a gRPC API using Protocol Buffers over HTTP/2,
//! offering strong typing, bi-directional streaming capabilities, and
//! excellent integration with service mesh environments.
//!
//! # Protocol Definition
//!
//! The gRPC service is defined in `proto/throttlecrab.proto`:
//!
//! ```protobuf
//! service RateLimiter {
//!     rpc Throttle(ThrottleRequest) returns (ThrottleResponse);
//! }
//! ```
//!
//! ## Request Message
//!
//! ```protobuf
//! message ThrottleRequest {
//!     string key = 1;              // Rate limit key
//!     int32 max_burst = 2;         // Maximum burst capacity
//!     int32 count_per_period = 3;  // Requests allowed per period
//!     int32 period = 4;            // Period in seconds
//!     int32 quantity = 5;          // Tokens to consume
//! }
//! ```
//!
//! ## Response Message
//!
//! ```protobuf
//! message ThrottleResponse {
//!     bool allowed = 1;       // Whether request is allowed
//!     int32 limit = 2;        // Maximum burst capacity
//!     int32 remaining = 3;    // Tokens remaining
//!     int32 retry_after = 4;  // Seconds until retry
//!     int32 reset_after = 5;  // Seconds until reset
//! }
//! ```
//!
//! # Features
//!
//! - **HTTP/2 Transport**: Multiplexing, server push, header compression
//! - **Type Safety**: Strongly typed messages with code generation
//! - **Service Mesh Ready**: Works with Istio, Linkerd, etc.
//! - **Cross-Language**: Client libraries for many languages
//! - **Streaming Support**: Built-in support for streaming (future enhancement)
//!
//! # Client Example
//!
//! ```ignore
//! use throttlecrab_proto::rate_limiter_client::RateLimiterClient;
//! use throttlecrab_proto::ThrottleRequest;
//!
//! let mut client = RateLimiterClient::connect("http://127.0.0.1:50051").await?;
//!
//! let request = tonic::Request::new(ThrottleRequest {
//!     key: "user:123".to_string(),
//!     max_burst: 10,
//!     count_per_period: 100,
//!     period: 60,
//!     quantity: 1,
//! });
//!
//! let response = client.throttle(request).await?;
//! ```

use crate::actor::RateLimiterHandle;
use crate::transport::Transport;
use crate::types::ThrottleRequest as ActorRequest;
use anyhow::Result;
use async_trait::async_trait;
use std::net::SocketAddr;
use std::time::SystemTime;
use tonic::{Request, Response, Status, transport::Server};

// Include the generated protobuf code
pub mod throttlecrab_proto {
    tonic::include_proto!("throttlecrab");
}

use throttlecrab_proto::rate_limiter_server::{RateLimiter, RateLimiterServer};
use throttlecrab_proto::{ThrottleRequest, ThrottleResponse};

/// gRPC transport implementation
///
/// Provides a Protocol Buffers API over HTTP/2 for type-safe,
/// high-performance communication in microservice architectures.
pub struct GrpcTransport {
    addr: SocketAddr,
}

impl GrpcTransport {
    /// Create a new gRPC transport instance
    ///
    /// # Parameters
    ///
    /// - `host`: The host address to bind to (e.g., "0.0.0.0")
    /// - `port`: The port number to listen on (typically 50051)
    pub fn new(host: &str, port: u16) -> Self {
        let addr = format!("{host}:{port}").parse().expect("Invalid address");
        Self { addr }
    }
}

#[async_trait]
impl Transport for GrpcTransport {
    async fn start(self, limiter: RateLimiterHandle) -> Result<()> {
        let service = RateLimiterService { limiter };

        Server::builder()
            .add_service(RateLimiterServer::new(service))
            .serve(self.addr)
            .await?;

        Ok(())
    }
}

/// gRPC service implementation for rate limiting
///
/// This service handles incoming gRPC requests and forwards them
/// to the rate limiter actor for processing.
pub struct RateLimiterService {
    limiter: RateLimiterHandle,
}

#[tonic::async_trait]
impl RateLimiter for RateLimiterService {
    /// Handle a rate limit check request
    ///
    /// Validates the incoming request, forwards it to the rate limiter actor,
    /// and converts the response to the gRPC format.
    ///
    /// # Errors
    ///
    /// Returns a gRPC `Status` error if:
    /// - The rate limiter actor fails
    /// - Internal processing errors occur
    async fn throttle(
        &self,
        request: Request<ThrottleRequest>,
    ) -> Result<Response<ThrottleResponse>, Status> {
        let req = request.into_inner();

        // Use server timestamp
        let timestamp = SystemTime::now();

        // Convert to actor request
        let actor_request = ActorRequest {
            key: req.key,
            max_burst: req.max_burst as i64,
            count_per_period: req.count_per_period as i64,
            period: req.period as i64,
            quantity: req.quantity as i64,
            timestamp,
        };

        // Call the rate limiter
        let result = self
            .limiter
            .throttle(actor_request)
            .await
            .map_err(|e| Status::internal(format!("Rate limiter error: {e}")))?;

        // Convert to gRPC response
        let response = ThrottleResponse {
            allowed: result.allowed,
            limit: result.limit as i32,
            remaining: result.remaining as i32,
            retry_after: result.retry_after as i32,
            reset_after: result.reset_after as i32,
        };

        Ok(Response::new(response))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actor::RateLimiterActor;
    use tokio::time::{Duration, sleep};

    #[tokio::test]
    async fn test_grpc_server_basic() {
        // Start server
        let store = throttlecrab::PeriodicStore::builder()
            .capacity(1000)
            .cleanup_interval(std::time::Duration::from_secs(60))
            .build();
        let limiter = RateLimiterActor::spawn_periodic(1000, store);
        let transport = GrpcTransport::new("127.0.0.1", 9091);

        // Run server in background
        tokio::spawn(async move {
            transport.start(limiter).await.unwrap();
        });

        // Give server time to start
        sleep(Duration::from_millis(100)).await;

        // Connect client
        let mut client = throttlecrab_proto::rate_limiter_client::RateLimiterClient::connect(
            "http://127.0.0.1:9091",
        )
        .await
        .unwrap();

        let request = tonic::Request::new(ThrottleRequest {
            key: "test_key".to_string(),
            max_burst: 10,
            count_per_period: 20,
            period: 60,
            quantity: 1,
        });

        let response = client.throttle(request).await.unwrap();
        let resp = response.into_inner();

        assert!(resp.allowed);
        assert_eq!(resp.limit, 10);
        assert_eq!(resp.remaining, 9);
    }

    #[tokio::test]
    async fn test_grpc_rate_limiting() {
        // Start server
        let store = throttlecrab::PeriodicStore::builder()
            .capacity(1000)
            .cleanup_interval(std::time::Duration::from_secs(60))
            .build();
        let limiter = RateLimiterActor::spawn_periodic(1000, store);
        let transport = GrpcTransport::new("127.0.0.1", 9092);

        // Run server in background
        tokio::spawn(async move {
            transport.start(limiter).await.unwrap();
        });

        // Give server time to start
        sleep(Duration::from_millis(100)).await;

        // Connect client
        let mut client = throttlecrab_proto::rate_limiter_client::RateLimiterClient::connect(
            "http://127.0.0.1:9092",
        )
        .await
        .unwrap();

        // Send requests until we hit the limit
        let mut allowed_count = 0;
        for _ in 0..15 {
            let request = tonic::Request::new(ThrottleRequest {
                key: "rate_limit_test".to_string(),
                max_burst: 5,
                count_per_period: 10,
                period: 60,
                quantity: 1,
            });

            let response = client.throttle(request).await.unwrap();
            let resp = response.into_inner();

            if resp.allowed {
                allowed_count += 1;
            } else {
                // Should be rate limited after burst
                assert!(allowed_count >= 5);
                assert!(resp.retry_after > 0);
                break;
            }
        }

        assert_eq!(allowed_count, 5); // Should allow exactly the burst size
    }
}

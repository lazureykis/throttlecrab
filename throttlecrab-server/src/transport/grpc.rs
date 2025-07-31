use crate::actor::RateLimiterHandle;
use crate::transport::Transport;
use crate::types::ThrottleRequest as ActorRequest;
use anyhow::Result;
use async_trait::async_trait;
use std::net::SocketAddr;
use std::time::{Duration, UNIX_EPOCH};
use tonic::{Request, Response, Status, transport::Server};

// Include the generated protobuf code
pub mod throttlecrab_proto {
    tonic::include_proto!("throttlecrab");
}

use throttlecrab_proto::rate_limiter_server::{RateLimiter, RateLimiterServer};
use throttlecrab_proto::{ThrottleRequest, ThrottleResponse};

pub struct GrpcTransport {
    addr: SocketAddr,
}

impl GrpcTransport {
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

pub struct RateLimiterService {
    limiter: RateLimiterHandle,
}

#[tonic::async_trait]
impl RateLimiter for RateLimiterService {
    async fn throttle(
        &self,
        request: Request<ThrottleRequest>,
    ) -> Result<Response<ThrottleResponse>, Status> {
        let req = request.into_inner();

        // Convert timestamp from nanoseconds to SystemTime
        let timestamp = UNIX_EPOCH + Duration::from_nanos(req.timestamp as u64);

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
    use std::time::{SystemTime, UNIX_EPOCH};
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

        let now = SystemTime::now();
        let duration = now.duration_since(UNIX_EPOCH).unwrap();

        let request = tonic::Request::new(ThrottleRequest {
            key: "test_key".to_string(),
            max_burst: 10,
            count_per_period: 20,
            period: 60,
            quantity: 1,
            timestamp: duration.as_nanos() as i64,
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
            let now = SystemTime::now();
            let duration = now.duration_since(UNIX_EPOCH).unwrap();

            let request = tonic::Request::new(ThrottleRequest {
                key: "rate_limit_test".to_string(),
                max_burst: 5,
                count_per_period: 10,
                period: 60,
                quantity: 1,
                timestamp: duration.as_nanos() as i64,
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

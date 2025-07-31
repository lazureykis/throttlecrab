//! Optimized client implementation with better connection pooling

use std::net::ToSocketAddrs;
use std::sync::Arc;
use std::time::Duration;

use crate::error::Result;
use crate::pool_v2::{ConnectionPoolV2, PoolConfig};
use crate::protocol::{ThrottleRequest, ThrottleResponse};

/// Optimized ThrottleCrab client
#[derive(Clone)]
pub struct ThrottleCrabClientV2 {
    pool: Arc<ConnectionPoolV2>,
}

impl ThrottleCrabClientV2 {
    /// Create a new client with default configuration
    pub async fn connect<A: ToSocketAddrs>(addr: A) -> Result<Self> {
        Self::builder().build(addr).await
    }

    /// Create a new client builder
    pub fn builder() -> ClientBuilderV2 {
        ClientBuilderV2::new()
    }

    /// Check rate limit
    pub async fn check_rate_limit(
        &self,
        key: &str,
        max_burst: i64,
        count_per_period: i64,
        period: i64,
    ) -> Result<ThrottleResponse> {
        let request = ThrottleRequest {
            key: key.to_string(),
            max_burst,
            count_per_period,
            period,
            quantity: 1,
        };

        self.pool.send_request(&request).await
    }

    /// Check rate limit with custom quantity
    pub async fn check_rate_limit_with_quantity(
        &self,
        key: &str,
        max_burst: i64,
        count_per_period: i64,
        period: i64,
        quantity: i64,
    ) -> Result<ThrottleResponse> {
        let request = ThrottleRequest {
            key: key.to_string(),
            max_burst,
            count_per_period,
            period,
            quantity,
        };

        self.pool.send_request(&request).await
    }

    /// Get pool statistics
    pub fn pool_stats(&self) -> crate::pool_v2::PoolStats {
        self.pool.stats()
    }
}

/// Builder for creating optimized clients
pub struct ClientBuilderV2 {
    config: PoolConfig,
}

impl Default for ClientBuilderV2 {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientBuilderV2 {
    pub fn new() -> Self {
        Self {
            config: PoolConfig::default(),
        }
    }

    /// Set maximum idle connections
    pub fn max_idle_connections(mut self, max: usize) -> Self {
        self.config.max_idle_connections = max;
        self
    }

    /// Set idle timeout
    pub fn idle_timeout(mut self, timeout: Duration) -> Self {
        self.config.idle_timeout = timeout;
        self
    }

    /// Set connection timeout
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.config.connect_timeout = timeout;
        self
    }

    /// Set request timeout
    pub fn request_timeout(mut self, timeout: Duration) -> Self {
        self.config.request_timeout = timeout;
        self
    }

    /// Enable or disable TCP nodelay
    pub fn tcp_nodelay(mut self, nodelay: bool) -> Self {
        self.config.tcp_nodelay = nodelay;
        self
    }

    /// Build the client
    pub async fn build<A: ToSocketAddrs>(self, addr: A) -> Result<ThrottleCrabClientV2> {
        let addr = addr.to_socket_addrs()?.next().ok_or_else(|| {
            crate::error::ClientError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid address",
            ))
        })?;

        let pool = Arc::new(ConnectionPoolV2::new(addr, self.config));

        Ok(ThrottleCrabClientV2 { pool })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use std::time::Duration;
    use throttlecrab_server::actor::RateLimiterActor;
    use throttlecrab_server::transport::{Transport, native::NativeTransport};
    use tokio::net::TcpListener;

    async fn start_test_server() -> (SocketAddr, tokio::task::JoinHandle<()>) {
        // Create rate limiter
        let store = throttlecrab::PeriodicStore::builder()
            .capacity(1000)
            .cleanup_interval(Duration::from_secs(60))
            .build();
        let limiter = RateLimiterActor::spawn_periodic(1000, store);

        // Bind to random port
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);

        let transport = NativeTransport::new("127.0.0.1", addr.port());

        // Start server in background
        let handle = tokio::spawn(async move {
            transport.start(limiter).await.unwrap();
        });

        // Give server time to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        (addr, handle)
    }

    #[tokio::test]
    async fn test_connect_and_basic_request() {
        let (addr, _handle) = start_test_server().await;

        // Create client using connect method
        let client = ThrottleCrabClientV2::connect(addr).await.unwrap();

        // Make a basic request
        let response = client
            .check_rate_limit("test_key", 10, 100, 60)
            .await
            .unwrap();

        assert!(response.allowed);
        assert_eq!(response.limit, 10);
        assert_eq!(response.remaining, 9);
    }

    #[tokio::test]
    async fn test_builder_with_custom_config() {
        let (addr, _handle) = start_test_server().await;

        // Create client with custom configuration
        let client = ThrottleCrabClientV2::builder()
            .max_idle_connections(5)
            .idle_timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(2))
            .request_timeout(Duration::from_secs(2))
            .tcp_nodelay(false)
            .build(addr)
            .await
            .unwrap();

        // Make a request to verify client works
        let response = client
            .check_rate_limit("test_builder", 20, 200, 60)
            .await
            .unwrap();

        assert!(response.allowed);
        assert_eq!(response.limit, 20);
    }

    #[tokio::test]
    async fn test_check_rate_limit_with_quantity() {
        let (addr, _handle) = start_test_server().await;

        let client = ThrottleCrabClientV2::connect(addr).await.unwrap();

        // Request with quantity of 5
        let response = client
            .check_rate_limit_with_quantity("quantity_test", 10, 100, 60, 5)
            .await
            .unwrap();

        assert!(response.allowed);
        assert_eq!(response.limit, 10);
        assert_eq!(response.remaining, 5); // 10 - 5 = 5

        // Request another 5
        let response = client
            .check_rate_limit_with_quantity("quantity_test", 10, 100, 60, 5)
            .await
            .unwrap();

        assert!(response.allowed);
        assert_eq!(response.remaining, 0); // 5 - 5 = 0

        // Request 1 more should be rejected
        let response = client
            .check_rate_limit_with_quantity("quantity_test", 10, 100, 60, 1)
            .await
            .unwrap();

        assert!(!response.allowed);
        assert_eq!(response.remaining, 0);
    }

    #[tokio::test]
    async fn test_pool_statistics() {
        let (addr, _handle) = start_test_server().await;

        let client = ThrottleCrabClientV2::builder()
            .max_idle_connections(3)
            .build(addr)
            .await
            .unwrap();

        // Initial stats should show 0 idle connections
        let stats = client.pool_stats();
        assert_eq!(stats.idle_connections, 0);

        // Make some requests
        for i in 0..5 {
            client
                .check_rate_limit(&format!("stats_test_{i}"), 10, 100, 60)
                .await
                .unwrap();
        }

        // After requests, we should have some idle connections (up to max_idle_connections)
        let stats = client.pool_stats();
        assert!(stats.idle_connections <= 3);
    }

    #[tokio::test]
    async fn test_concurrent_requests() {
        let (addr, _handle) = start_test_server().await;

        let client = ThrottleCrabClientV2::connect(addr).await.unwrap();

        // Spawn multiple concurrent requests
        let mut handles = vec![];
        for i in 0..10 {
            let client = client.clone();
            let handle = tokio::spawn(async move {
                client
                    .check_rate_limit(&format!("concurrent_{i}"), 100, 1000, 60)
                    .await
            });
            handles.push(handle);
        }

        // All requests should succeed
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
            let response = result.unwrap();
            assert!(response.allowed);
        }
    }

    #[tokio::test]
    async fn test_invalid_address() {
        // Test with invalid address
        let result = ThrottleCrabClientV2::connect("invalid_address").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_connection_refused() {
        // Test with port that's likely not in use
        let result = ThrottleCrabClientV2::connect("127.0.0.1:54321").await;
        // This should succeed in creating the client (lazy connection)
        // The actual error will happen when making a request
        if let Ok(client) = result {
            let response = client.check_rate_limit("test", 10, 100, 60).await;
            assert!(response.is_err());
        }
    }

    #[tokio::test]
    async fn test_client_clone() {
        let (addr, _handle) = start_test_server().await;

        let client1 = ThrottleCrabClientV2::connect(addr).await.unwrap();
        let client2 = client1.clone();

        // Both clients should work independently
        let response1 = client1
            .check_rate_limit("clone_test", 10, 100, 60)
            .await
            .unwrap();
        let response2 = client2
            .check_rate_limit("clone_test", 10, 100, 60)
            .await
            .unwrap();

        assert!(response1.allowed);
        assert!(response2.allowed);
        assert_eq!(response1.remaining, 9);
        assert_eq!(response2.remaining, 8);
    }
}

use std::net::ToSocketAddrs;
use std::time::Duration;

use crate::error::Result;
use crate::pool::{ConnectionPool, PoolConfig};
use crate::protocol::{ThrottleRequest, ThrottleResponse};

/// Builder for creating a ThrottleCrabClient
#[derive(Default)]
pub struct ClientBuilder {
    pool_config: PoolConfig,
}

impl ClientBuilder {
    /// Create a new client builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum number of connections in the pool
    pub fn max_connections(mut self, max: usize) -> Self {
        self.pool_config.max_size = max;
        self
    }

    /// Set minimum number of idle connections
    pub fn min_idle_connections(mut self, min: usize) -> Self {
        self.pool_config.min_idle = min;
        self
    }

    /// Set connection timeout
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.pool_config.connect_timeout = timeout;
        self
    }

    /// Set request timeout
    pub fn request_timeout(mut self, timeout: Duration) -> Self {
        self.pool_config.request_timeout = timeout;
        self
    }

    /// Set idle connection timeout (0 means no timeout)
    pub fn idle_timeout(mut self, timeout: Duration) -> Self {
        self.pool_config.idle_timeout = timeout;
        self
    }

    /// Enable or disable TCP nodelay
    pub fn tcp_nodelay(mut self, nodelay: bool) -> Self {
        self.pool_config.tcp_nodelay = nodelay;
        self
    }

    /// Build the client with the given address
    pub async fn build(self, addr: impl ToSocketAddrs) -> Result<ThrottleCrabClient> {
        let addr = addr.to_socket_addrs()?.next().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid address")
        })?;

        let pool = ConnectionPool::new(addr, self.pool_config);

        // Pre-warm the pool
        pool.warm_up().await?;

        Ok(ThrottleCrabClient { pool })
    }
}

/// High-performance client for throttlecrab server
#[derive(Clone)]
pub struct ThrottleCrabClient {
    pool: ConnectionPool,
}

impl ThrottleCrabClient {
    /// Create a new client with default configuration
    pub async fn connect(addr: impl ToSocketAddrs) -> Result<Self> {
        ClientBuilder::new().build(addr).await
    }

    /// Create a new client builder for advanced configuration
    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    /// Check if a request is allowed by the rate limiter
    pub async fn check_rate_limit(
        &self,
        key: impl Into<String>,
        max_burst: i64,
        count_per_period: i64,
        period: i64,
    ) -> Result<ThrottleResponse> {
        let request = ThrottleRequest::new(key, max_burst, count_per_period, period);
        self.throttle(request).await
    }

    /// Check if a request is allowed with custom quantity
    pub async fn check_rate_limit_with_quantity(
        &self,
        key: impl Into<String>,
        max_burst: i64,
        count_per_period: i64,
        period: i64,
        quantity: i64,
    ) -> Result<ThrottleResponse> {
        let request =
            ThrottleRequest::new(key, max_burst, count_per_period, period).with_quantity(quantity);
        self.throttle(request).await
    }

    /// Send a throttle request
    pub async fn throttle(&self, request: ThrottleRequest) -> Result<ThrottleResponse> {
        self.pool.throttle(request).await
    }

    /// Get current pool size
    pub fn pool_size(&self) -> usize {
        self.pool.size()
    }

    /// Get available connections count
    pub fn available_connections(&self) -> usize {
        self.pool.available()
    }
}

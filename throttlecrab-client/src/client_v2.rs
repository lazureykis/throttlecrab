//! Optimized client implementation with better connection pooling

use std::net::ToSocketAddrs;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

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
        let addr = addr
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| crate::error::ClientError::Io(
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid address")
            ))?;

        let pool = Arc::new(ConnectionPoolV2::new(addr, self.config));

        Ok(ThrottleCrabClientV2 { pool })
    }
}
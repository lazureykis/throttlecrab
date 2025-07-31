use parking_lot::Mutex;
use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::time::timeout;

use crate::error::{ClientError, Result};
use crate::protocol::{NativeProtocol, ThrottleRequest, ThrottleResponse};

/// Configuration for connection pool
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Maximum number of connections in the pool
    pub max_size: usize,
    /// Minimum number of idle connections to maintain
    pub min_idle: usize,
    /// Connection timeout
    pub connect_timeout: Duration,
    /// Request timeout
    pub request_timeout: Duration,
    /// Idle connection timeout (0 means no timeout)
    pub idle_timeout: Duration,
    /// Enable TCP nodelay
    pub tcp_nodelay: bool,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_size: 10,
            min_idle: 1,
            connect_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(300),
            tcp_nodelay: true,
        }
    }
}

/// A connection wrapper that returns to pool on drop
pub struct PooledConnection {
    stream: Option<TcpStream>,
    pool: Arc<ConnectionPoolInner>,
}

impl Drop for PooledConnection {
    fn drop(&mut self) {
        if let Some(stream) = self.stream.take() {
            self.pool.clone().return_connection(stream);
        }
    }
}

struct ConnectionPoolInner {
    addr: SocketAddr,
    config: PoolConfig,
    connections: Mutex<VecDeque<TcpStream>>,
    semaphore: Arc<Semaphore>,
}

impl ConnectionPoolInner {
    fn return_connection(self: Arc<Self>, stream: TcpStream) {
        let mut connections = self.connections.lock();
        if connections.len() < self.config.max_size {
            connections.push_back(stream);
        }
        // If pool is full, just drop the connection
    }

    async fn create_connection(&self) -> Result<TcpStream> {
        let stream = timeout(self.config.connect_timeout, TcpStream::connect(self.addr))
            .await
            .map_err(|_| ClientError::Timeout)??;

        if self.config.tcp_nodelay {
            stream.set_nodelay(true)?;
        }

        Ok(stream)
    }
}

/// Connection pool for throttlecrab native protocol
#[derive(Clone)]
pub struct ConnectionPool {
    inner: Arc<ConnectionPoolInner>,
}

impl ConnectionPool {
    /// Create a new connection pool
    pub fn new(addr: SocketAddr, config: PoolConfig) -> Self {
        let inner = Arc::new(ConnectionPoolInner {
            addr,
            config: config.clone(),
            connections: Mutex::new(VecDeque::with_capacity(config.max_size)),
            semaphore: Arc::new(Semaphore::new(config.max_size)),
        });

        Self { inner }
    }

    /// Get a connection from the pool
    async fn get_connection(&self) -> Result<(TcpStream, OwnedSemaphorePermit)> {
        // Acquire permit first
        let permit = self
            .inner
            .semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| ClientError::Pool("Failed to acquire permit".to_string()))?;

        // Try to get existing connection
        let stream = {
            let mut connections = self.inner.connections.lock();
            connections.pop_front()
        };

        let stream = match stream {
            Some(stream) => stream,
            None => self.inner.create_connection().await?,
        };

        Ok((stream, permit))
    }

    /// Send a throttle request using a pooled connection
    pub async fn throttle(&self, request: ThrottleRequest) -> Result<ThrottleResponse> {
        let (mut stream, _permit) = self.get_connection().await?;

        let response = timeout(
            self.inner.config.request_timeout,
            NativeProtocol::send_request(&mut stream, &request),
        )
        .await
        .map_err(|_| ClientError::Timeout)??;

        // Return connection to pool
        self.inner.clone().return_connection(stream);

        Ok(response)
    }

    /// Get current pool size
    pub fn size(&self) -> usize {
        self.inner.connections.lock().len()
    }

    /// Get available connections count
    pub fn available(&self) -> usize {
        self.inner.semaphore.available_permits()
    }

    /// Pre-warm the pool by creating min_idle connections
    pub async fn warm_up(&self) -> Result<()> {
        let mut handles = vec![];

        for _ in 0..self.inner.config.min_idle {
            let inner = self.inner.clone();
            handles.push(tokio::spawn(async move {
                match inner.create_connection().await {
                    Ok(stream) => {
                        inner.return_connection(stream);
                        Ok(())
                    }
                    Err(e) => Err(e),
                }
            }));
        }

        // Wait for all connections to be created
        for handle in handles {
            handle
                .await
                .map_err(|e| ClientError::Pool(e.to_string()))??;
        }

        Ok(())
    }
}

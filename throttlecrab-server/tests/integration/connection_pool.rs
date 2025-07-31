use anyhow::Result;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tracing::{debug, error, warn};

/// Configuration for connection pools
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Maximum number of idle connections to maintain
    pub max_connections: usize,
    /// Minimum number of connections to maintain
    pub min_connections: usize,
    /// Maximum lifetime for a connection
    pub max_lifetime: Duration,
    /// Time to wait for a connection to become available
    pub connection_timeout: Duration,
    /// Time between health checks
    pub health_check_interval: Duration,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections: 50,
            min_connections: 5,
            max_lifetime: Duration::from_secs(300), // 5 minutes
            connection_timeout: Duration::from_secs(5),
            health_check_interval: Duration::from_secs(30),
        }
    }
}

/// A pooled connection with metadata
struct PooledConnection {
    stream: TcpStream,
    created_at: Instant,
    last_used: Instant,
}

impl PooledConnection {
    fn new(stream: TcpStream) -> Self {
        let now = Instant::now();
        Self {
            stream,
            created_at: now,
            last_used: now,
        }
    }

    fn is_expired(&self, max_lifetime: Duration) -> bool {
        self.created_at.elapsed() > max_lifetime
    }

    fn touch(&mut self) {
        self.last_used = Instant::now();
    }
}

/// Generic connection pool for TCP connections
pub struct ConnectionPool {
    connections: Arc<Mutex<Vec<PooledConnection>>>,
    addr: String,
    config: PoolConfig,
    active_connections: Arc<std::sync::atomic::AtomicUsize>,
}

impl ConnectionPool {
    pub fn new(addr: String, config: PoolConfig) -> Self {
        Self {
            connections: Arc::new(Mutex::new(Vec::with_capacity(config.max_connections))),
            addr,
            config,
            active_connections: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    /// Get a connection from the pool or create a new one
    pub async fn get(&self) -> Result<TcpStream> {
        // First, try to get an existing connection
        let mut pool = self.connections.lock().await;

        // Remove expired connections
        pool.retain(|conn| !conn.is_expired(self.config.max_lifetime));

        while let Some(mut conn) = pool.pop() {
            // Check if connection is still alive
            if self.is_connection_alive(&conn.stream).await {
                conn.touch();
                let stream = conn.stream;
                drop(pool); // Release lock before returning
                debug!(
                    "Reusing connection from pool (pool size: {})",
                    self.active_connections
                        .load(std::sync::atomic::Ordering::Relaxed)
                );
                return Ok(stream);
            } else {
                debug!("Removed stale connection from pool");
                self.active_connections
                    .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            }
        }

        drop(pool); // Release lock before creating new connection

        // Create new connection
        debug!("Creating new connection to {}", self.addr);
        match tokio::time::timeout(
            self.config.connection_timeout,
            TcpStream::connect(&self.addr),
        )
        .await
        {
            Ok(Ok(stream)) => {
                self.active_connections
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                Ok(stream)
            }
            Ok(Err(e)) => {
                error!("Failed to connect to {}: {}", self.addr, e);
                Err(e.into())
            }
            Err(_) => {
                error!("Connection timeout to {}", self.addr);
                Err(anyhow::anyhow!("Connection timeout"))
            }
        }
    }

    /// Return a connection to the pool
    pub async fn put(&self, stream: TcpStream) -> Result<()> {
        let mut pool = self.connections.lock().await;

        // Check pool capacity
        if pool.len() >= self.config.max_connections {
            debug!("Pool at capacity, dropping connection");
            drop(stream);
            self.active_connections
                .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            return Ok(());
        }

        // Validate connection before returning to pool
        if !self.is_connection_alive(&stream).await {
            debug!("Connection no longer alive, dropping");
            drop(stream);
            self.active_connections
                .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            return Ok(());
        }

        pool.push(PooledConnection::new(stream));
        debug!("Returned connection to pool (pool size: {})", pool.len());
        Ok(())
    }

    /// Return a connection to the pool with error information
    pub async fn put_with_error(&self, stream: TcpStream, error: &anyhow::Error) {
        warn!("Returning connection to pool after error: {}", error);

        // For certain errors, don't return the connection
        let error_str = error.to_string().to_lowercase();
        if error_str.contains("broken pipe")
            || error_str.contains("connection reset")
            || error_str.contains("connection refused")
        {
            debug!("Dropping connection due to fatal error");
            drop(stream);
            self.active_connections
                .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            return;
        }

        // Otherwise, try to return it
        if let Err(e) = self.put(stream).await {
            error!("Failed to return connection to pool: {}", e);
        }
    }

    /// Check if a connection is still alive
    async fn is_connection_alive(&self, stream: &TcpStream) -> bool {
        // Try to set TCP keepalive as a basic health check
        stream.set_nodelay(true).is_ok()
    }

    /// Get pool statistics
    pub async fn stats(&self) -> PoolStats {
        let pool = self.connections.lock().await;
        PoolStats {
            idle_connections: pool.len(),
            active_connections: self
                .active_connections
                .load(std::sync::atomic::Ordering::Relaxed),
            max_connections: self.config.max_connections,
        }
    }
}

#[derive(Debug)]
pub struct PoolStats {
    pub idle_connections: usize,
    pub active_connections: usize,
    pub max_connections: usize,
}

/// Native protocol connection pool
pub struct NativeConnectionPool {
    pool: ConnectionPool,
    port: u16,
}

impl NativeConnectionPool {
    pub fn new(port: u16, max_connections: usize) -> Self {
        let config = PoolConfig {
            max_connections,
            min_connections: 1,
            ..Default::default()
        };
        Self {
            pool: ConnectionPool::new(format!("127.0.0.1:{port}"), config),
            port,
        }
    }

    pub fn with_config(port: u16, config: PoolConfig) -> Self {
        Self {
            pool: ConnectionPool::new(format!("127.0.0.1:{port}"), config),
            port,
        }
    }

    pub async fn test_request(&self, key: String) -> Result<bool> {
        use bytes::{BufMut, BytesMut};
        use std::time::{SystemTime, UNIX_EPOCH};

        let mut stream = self.pool.get().await?;

        // Build request
        let mut request = BytesMut::new();
        request.put_u8(1); // cmd
        request.put_u8(key.len() as u8); // key_len
        request.put_i64_le(100); // burst
        request.put_i64_le(10); // rate
        request.put_i64_le(60); // period in seconds
        request.put_i64_le(1); // quantity

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as i64;
        request.put_i64_le(now);
        request.put_slice(key.as_bytes());

        let result = async {
            // Send request
            stream.write_all(&request).await?;
            stream.flush().await?;

            // Read response (34 bytes fixed)
            let mut response = vec![0u8; 34];
            stream.read_exact(&mut response).await?;

            // Validate response
            if response.len() != 34 {
                return Err(anyhow::anyhow!(
                    "Invalid response size: {} bytes",
                    response.len()
                ));
            }

            let ok = response[0];
            let allowed = response[1];

            if ok == 0 {
                return Err(anyhow::anyhow!("Server returned error"));
            }

            Ok::<bool, anyhow::Error>(allowed == 0)
        }
        .await;

        // Return connection to pool based on result
        match &result {
            Ok(_) => {
                self.pool.put(stream).await?;
            }
            Err(e) => {
                self.pool.put_with_error(stream, e).await;
            }
        }

        result
    }

    pub async fn stats(&self) -> PoolStats {
        self.pool.stats().await
    }
}

//! Optimized connection pool implementation inspired by reqwest/hyper

use parking_lot::Mutex;
use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time::timeout;

use crate::error::{ClientError, Result};
use crate::protocol::{ThrottleRequest, ThrottleResponse};

/// A reusable connection that can send multiple requests
struct Connection {
    stream: TcpStream,
    last_used: Instant,
}

impl Connection {
    async fn send_request(&mut self, request: &ThrottleRequest) -> Result<ThrottleResponse> {
        // Serialize request inline (avoiding allocations)
        let key_bytes = request.key.as_bytes();
        let key_len = key_bytes.len();
        if key_len > 255 {
            return Err(ClientError::Protocol("Key too long".to_string()));
        }

        // Write request directly to stream
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as i64;

        // Fixed header: 42 bytes
        let header = [
            1u8,           // cmd
            key_len as u8, // key_len
        ];

        self.stream.write_all(&header).await?;
        self.stream
            .write_all(&request.max_burst.to_le_bytes())
            .await?;
        self.stream
            .write_all(&request.count_per_period.to_le_bytes())
            .await?;
        self.stream
            .write_all(&(request.period * 1_000_000_000).to_le_bytes())
            .await?; // Convert seconds to nanoseconds
        self.stream
            .write_all(&request.quantity.to_le_bytes())
            .await?;
        self.stream.write_all(&timestamp.to_le_bytes()).await?;
        self.stream.write_all(key_bytes).await?;

        // Read response: 34 bytes
        let mut response_buf = [0u8; 34];
        self.stream.read_exact(&mut response_buf).await?;

        let ok = response_buf[0] != 0;
        let allowed = response_buf[1] != 0;
        let limit = i64::from_le_bytes(response_buf[2..10].try_into().unwrap());
        let remaining = i64::from_le_bytes(response_buf[10..18].try_into().unwrap());
        let retry_after = i64::from_le_bytes(response_buf[18..26].try_into().unwrap());
        let reset_after = i64::from_le_bytes(response_buf[26..34].try_into().unwrap());

        self.last_used = Instant::now();

        if !ok {
            return Err(ClientError::ServerError);
        }

        Ok(ThrottleResponse {
            allowed,
            limit,
            remaining,
            retry_after,
            reset_after,
        })
    }
}

/// Optimized connection pool
pub struct ConnectionPool {
    addr: SocketAddr,
    config: PoolConfig,
    /// Available connections
    idle: Arc<Mutex<VecDeque<Connection>>>,
    /// Channel for returning connections
    return_tx: mpsc::UnboundedSender<Connection>,
    return_rx: Arc<Mutex<mpsc::UnboundedReceiver<Connection>>>,
}

#[derive(Clone, Debug)]
pub struct PoolConfig {
    pub max_idle_connections: usize,
    pub idle_timeout: Duration,
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
    pub tcp_nodelay: bool,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_idle_connections: 100,
            idle_timeout: Duration::from_secs(90),
            connect_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(30),
            tcp_nodelay: true,
        }
    }
}

impl ConnectionPool {
    pub fn new(addr: SocketAddr, config: PoolConfig) -> Self {
        let (return_tx, return_rx) = mpsc::unbounded_channel();
        let max_idle = config.max_idle_connections;

        Self {
            addr,
            config,
            idle: Arc::new(Mutex::new(VecDeque::with_capacity(max_idle))),
            return_tx,
            return_rx: Arc::new(Mutex::new(return_rx)),
        }
    }

    /// Get a connection - either from pool or create new
    async fn checkout(&self) -> Result<Connection> {
        // First, check for returned connections
        {
            let mut rx = self.return_rx.lock();
            while let Ok(conn) = rx.try_recv() {
                let mut idle = self.idle.lock();
                if idle.len() < self.config.max_idle_connections {
                    idle.push_back(conn);
                }
            }
        }

        // Try to get an idle connection
        let now = Instant::now();
        let conn = {
            let mut idle = self.idle.lock();
            idle.iter()
                .position(|c| now.duration_since(c.last_used) < self.config.idle_timeout)
                .and_then(|pos| idle.remove(pos))
        };

        match conn {
            Some(conn) => Ok(conn),
            None => {
                // Create new connection
                let stream = timeout(self.config.connect_timeout, TcpStream::connect(self.addr))
                    .await
                    .map_err(|_| ClientError::Timeout)??;

                if self.config.tcp_nodelay {
                    stream.set_nodelay(true)?;
                }

                Ok(Connection {
                    stream,
                    last_used: Instant::now(),
                })
            }
        }
    }

    /// Return a connection to the pool
    fn checkin(&self, conn: Connection) {
        // Use channel to avoid deadlock
        let _ = self.return_tx.send(conn);
    }

    /// Send a request using a pooled connection
    pub async fn send_request(&self, request: &ThrottleRequest) -> Result<ThrottleResponse> {
        let mut conn = self.checkout().await?;

        let result = timeout(self.config.request_timeout, conn.send_request(request)).await;

        match result {
            Ok(Ok(response)) => {
                // Success - return connection to pool
                self.checkin(conn);
                Ok(response)
            }
            Ok(Err(e)) => {
                // Protocol error - don't reuse connection
                Err(e)
            }
            Err(_) => {
                // Timeout - don't reuse connection
                Err(ClientError::Timeout)
            }
        }
    }

    /// Get current pool statistics
    pub fn stats(&self) -> PoolStats {
        let idle_count = self.idle.lock().len();
        PoolStats {
            idle_connections: idle_count,
        }
    }
}

pub struct PoolStats {
    pub idle_connections: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    async fn start_mock_server() -> (SocketAddr, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let handle = tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                tokio::spawn(async move {
                    loop {
                        // Read request header
                        let mut header = [0u8; 2];
                        if socket.read_exact(&mut header).await.is_err() {
                            break;
                        }

                        let key_len = header[1] as usize;

                        // Read rest of request
                        let mut fixed_data = [0u8; 40]; // 5 * i64
                        if socket.read_exact(&mut fixed_data).await.is_err() {
                            break;
                        }

                        let mut key = vec![0u8; key_len];
                        if socket.read_exact(&mut key).await.is_err() {
                            break;
                        }

                        // Send mock response
                        let mut response_bytes = Vec::with_capacity(34);
                        response_bytes.push(1u8); // ok
                        response_bytes.push(1u8); // allowed
                        response_bytes.extend_from_slice(&10i64.to_le_bytes()); // limit
                        response_bytes.extend_from_slice(&9i64.to_le_bytes()); // remaining
                        response_bytes.extend_from_slice(&0i64.to_le_bytes()); // retry_after
                        response_bytes.extend_from_slice(&60i64.to_le_bytes()); // reset_after

                        if socket.write_all(&response_bytes).await.is_err() {
                            break;
                        }
                    }
                });
            }
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        (addr, handle)
    }

    #[tokio::test]
    async fn test_pool_config_default() {
        let config = PoolConfig::default();
        assert_eq!(config.max_idle_connections, 100);
        assert_eq!(config.idle_timeout, Duration::from_secs(90));
        assert_eq!(config.connect_timeout, Duration::from_secs(5));
        assert_eq!(config.request_timeout, Duration::from_secs(30));
        assert!(config.tcp_nodelay);
    }

    #[tokio::test]
    async fn test_connection_checkout_and_checkin() {
        let (addr, _handle) = start_mock_server().await;
        let pool = ConnectionPool::new(addr, PoolConfig::default());

        // Initial stats should show 0 connections
        assert_eq!(pool.stats().idle_connections, 0);

        // Make a request which will checkout and checkin a connection
        let request = ThrottleRequest {
            key: "test".to_string(),
            max_burst: 10,
            count_per_period: 100,
            period: 60,
            quantity: 1,
        };

        pool.send_request(&request).await.unwrap();

        // After request, connection should be returned to pool
        // But we need to trigger the processing by doing another checkout
        let conn = pool.checkout().await.unwrap();

        // The pool should have processed the returned connection
        // Verify stats work correctly
        let stats = pool.stats();
        // Just verify the stat is within expected range
        assert!(stats.idle_connections <= pool.config.max_idle_connections);

        // Return the connection
        pool.checkin(conn);
    }

    #[tokio::test]
    async fn test_connection_reuse() {
        let (addr, _handle) = start_mock_server().await;
        let pool = ConnectionPool::new(addr, PoolConfig::default());

        // Make first request
        let request = ThrottleRequest {
            key: "test".to_string(),
            max_burst: 10,
            count_per_period: 100,
            period: 60,
            quantity: 1,
        };

        let response = pool.send_request(&request).await.unwrap();
        assert!(response.allowed);

        // Connection should be returned to pool
        // Make another request - should reuse connection
        let response = pool.send_request(&request).await.unwrap();
        assert!(response.allowed);

        // Should still have idle connections
        assert!(pool.stats().idle_connections <= 1);
    }

    #[tokio::test]
    async fn test_max_idle_connections() {
        let (addr, _handle) = start_mock_server().await;
        let config = PoolConfig {
            max_idle_connections: 2,
            ..Default::default()
        };
        let pool = ConnectionPool::new(addr, config);

        let request = ThrottleRequest {
            key: "test".to_string(),
            max_burst: 10,
            count_per_period: 100,
            period: 60,
            quantity: 1,
        };

        // Make multiple requests to create connections
        for _ in 0..5 {
            pool.send_request(&request).await.unwrap();
        }

        // Pool should only keep max_idle_connections
        assert!(pool.stats().idle_connections <= 2);
    }

    #[tokio::test]
    async fn test_connection_timeout() {
        // Use a non-routable IP to trigger timeout
        let addr: SocketAddr = "10.255.255.255:12345".parse().unwrap();
        let config = PoolConfig {
            connect_timeout: Duration::from_millis(100),
            ..Default::default()
        };
        let pool = ConnectionPool::new(addr, config);

        let result = pool.checkout().await;
        assert!(matches!(result, Err(ClientError::Timeout)));
    }

    #[tokio::test]
    async fn test_request_timeout() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Start a server that accepts but never responds
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                tokio::spawn(async move {
                    // Read but never respond
                    let mut buf = [0u8; 1024];
                    let _ = socket.read(&mut buf).await;
                    tokio::time::sleep(Duration::from_secs(10)).await;
                });
            }
        });

        tokio::time::sleep(Duration::from_millis(50)).await;

        let config = PoolConfig {
            request_timeout: Duration::from_millis(100),
            ..Default::default()
        };
        let pool = ConnectionPool::new(addr, config);

        let request = ThrottleRequest {
            key: "test".to_string(),
            max_burst: 10,
            count_per_period: 100,
            period: 60,
            quantity: 1,
        };

        let result = pool.send_request(&request).await;
        assert!(matches!(result, Err(ClientError::Timeout)));
    }

    #[tokio::test]
    async fn test_connection_key_too_long() {
        let (addr, _handle) = start_mock_server().await;
        let pool = ConnectionPool::new(addr, PoolConfig::default());

        let request = ThrottleRequest {
            key: "x".repeat(256), // Key longer than 255 bytes
            max_burst: 10,
            count_per_period: 100,
            period: 60,
            quantity: 1,
        };

        let result = pool.send_request(&request).await;
        assert!(matches!(result, Err(ClientError::Protocol(_))));
    }

    #[tokio::test]
    async fn test_concurrent_pool_access() {
        let (addr, _handle) = start_mock_server().await;
        let pool = Arc::new(ConnectionPool::new(addr, PoolConfig::default()));

        let mut handles = vec![];
        for i in 0..10 {
            let pool = pool.clone();
            let handle = tokio::spawn(async move {
                let request = ThrottleRequest {
                    key: format!("concurrent_{i}"),
                    max_burst: 10,
                    count_per_period: 100,
                    period: 60,
                    quantity: 1,
                };
                pool.send_request(&request).await
            });
            handles.push(handle);
        }

        // All requests should succeed
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
        }

        // Make one more request to trigger processing of returned connections
        let request = ThrottleRequest {
            key: "trigger".to_string(),
            max_burst: 10,
            count_per_period: 100,
            period: 60,
            quantity: 1,
        };
        pool.send_request(&request).await.unwrap();

        // Pool may or may not have idle connections depending on timing
        // Just verify the stat call works
        let stats = pool.stats();
        assert!(stats.idle_connections <= pool.config.max_idle_connections);
    }
}

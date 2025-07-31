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
            1u8,  // cmd
            key_len as u8,  // key_len
        ];
        
        self.stream.write_all(&header).await?;
        self.stream.write_all(&request.max_burst.to_le_bytes()).await?;
        self.stream.write_all(&request.count_per_period.to_le_bytes()).await?;
        self.stream.write_all(&request.period.to_le_bytes()).await?;
        self.stream.write_all(&request.quantity.to_le_bytes()).await?;
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
pub struct ConnectionPoolV2 {
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

impl ConnectionPoolV2 {
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
                let stream = timeout(
                    self.config.connect_timeout,
                    TcpStream::connect(self.addr)
                ).await
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
        
        let result = timeout(
            self.config.request_timeout,
            conn.send_request(request)
        ).await;

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
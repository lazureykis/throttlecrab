//! Native binary protocol transport
//!
//! This transport provides the highest performance by using a compact binary
//! protocol with fixed-size fields and minimal parsing overhead.
//!
//! # Key Length Limitation
//!
//! The native protocol has a **maximum key length of 255 bytes** due to the
//! protocol design using a single byte (u8) for key length. This is a protocol
//! limitation, not a policy decision. HTTP and gRPC transports do not have
//! this restriction.
//!
//! # Protocol Specification
//!
//! ## Request Format (34 bytes + variable key)
//!
//! ```text
//! ┌─────┬─────────┬─────────┬──────┬────────┬──────────┬─────┐
//! │ cmd │ key_len │  burst  │ rate │ period │ quantity │ key │
//! ├─────┼─────────┼─────────┼──────┼────────┼──────────┼─────┤
//! │ u8  │   u8    │   i64   │ i64  │  i64   │   i64    │ var │
//! └─────┴─────────┴─────────┴──────┴────────┴──────────┴─────┘
//! ```
//!
//! Fields:
//!  - `cmd`: Command type (currently only 1 for rate_limit)
//!  - `key_len`: Length of the key in bytes (max 255)
//!  - `burst`: Maximum burst capacity
//!  - `rate`: Requests per period
//!  - `period`: Time period in seconds
//!  - `quantity`: Number of tokens to consume
//!  - `key`: UTF-8 encoded key string
//!
//! ## Response Format (34 bytes)
//!
//! ```text
//! ┌────┬─────────┬───────┬───────────┬─────────────┬─────────────┐
//! │ ok │ allowed │ limit │ remaining │ retry_after │ reset_after │
//! ├────┼─────────┼───────┼───────────┼─────────────┼─────────────┤
//! │ u8 │   u8    │  i64  │    i64    │     i64     │     i64     │
//! └────┴─────────┴───────┴───────────┴─────────────┴─────────────┘
//! ```
//!
//! Fields:
//!  - `ok`: Success indicator (1 = success, 0 = error)
//!  - `allowed`: Request allowed (1) or denied (0)
//!  - `limit`: Maximum burst capacity
//!  - `remaining`: Tokens remaining
//!  - `retry_after`: Seconds until next request allowed
//!  - `reset_after`: Seconds until full capacity restored

use super::Transport;
use crate::actor::RateLimiterHandle;
use crate::types::ThrottleRequest;
use anyhow::Result;
use async_trait::async_trait;
use bytes::{BufMut, BytesMut};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const READ_BUFFER_SIZE: usize = 256;
const WRITE_BUFFER_SIZE: usize = 64;
const MAX_KEY_LENGTH: usize = 255;

/// Native binary protocol transport implementation
///
/// Provides the highest throughput and lowest latency by using a
/// compact binary format with minimal overhead.
pub struct NativeTransport {
    host: String,
    port: u16,
}

impl NativeTransport {
    /// Create a new native transport instance
    ///
    /// # Parameters
    ///
    /// - `host`: The host address to bind to (e.g., "0.0.0.0")
    /// - `port`: The port number to listen on
    pub fn new(host: &str, port: u16) -> Self {
        NativeTransport {
            host: host.to_string(),
            port,
        }
    }

    /// Handle a single client connection
    ///
    /// Processes rate limit requests from the client until the connection
    /// is closed or an error occurs.
    async fn handle_connection(mut socket: TcpStream, limiter: RateLimiterHandle) -> Result<()> {
        // Pre-allocate buffers
        let mut read_buffer = BytesMut::with_capacity(READ_BUFFER_SIZE);
        let mut write_buffer = BytesMut::with_capacity(WRITE_BUFFER_SIZE);

        // Set TCP_NODELAY for lower latency
        socket.set_nodelay(true)?;

        // Fixed-size header buffer
        let mut header = [0u8; 34]; // Max size for fixed fields

        loop {
            // Read fixed header (first 2 bytes: cmd + key_len)
            match socket.read_exact(&mut header[0..2]).await {
                Ok(_) => {}
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::UnexpectedEof {
                        break; // Client disconnected
                    }
                    return Err(e.into());
                }
            }

            let cmd = header[0];
            let key_len = header[1] as usize;

            if cmd != 1 {
                tracing::warn!("Unknown command: {}", cmd);
                break;
            }

            if key_len > MAX_KEY_LENGTH {
                tracing::warn!("Key too long: {} bytes", key_len);
                break;
            }

            // Read remaining fixed fields (32 bytes)
            socket.read_exact(&mut header[2..34]).await?;

            // Parse fixed fields
            let burst = i64::from_le_bytes(header[2..10].try_into().unwrap());
            let rate = i64::from_le_bytes(header[10..18].try_into().unwrap());
            let period = i64::from_le_bytes(header[18..26].try_into().unwrap());
            let quantity = i64::from_le_bytes(header[26..34].try_into().unwrap());

            // Read key
            read_buffer.clear();
            read_buffer.resize(key_len, 0);
            socket.read_exact(&mut read_buffer).await?;

            let key = match std::str::from_utf8(&read_buffer) {
                Ok(s) => s.to_string(),
                Err(_) => {
                    tracing::error!("Invalid UTF-8 in key");
                    break;
                }
            };

            // Get server timestamp
            let timestamp = SystemTime::now();

            // Create request
            let request = ThrottleRequest {
                key,
                max_burst: burst,
                count_per_period: rate,
                period,
                quantity,
                timestamp,
            };

            // Process request
            let response = match limiter.throttle(request).await {
                Ok(resp) => resp,
                Err(e) => {
                    tracing::error!("Rate limiter error: {}", e);
                    // Send error response
                    write_buffer.clear();
                    write_buffer.put_u8(0); // ok = false
                    write_buffer.put_u8(0); // allowed = 0
                    write_buffer.put_i64_le(0); // limit
                    write_buffer.put_i64_le(0); // remaining
                    write_buffer.put_i64_le(0); // retry_after
                    write_buffer.put_i64_le(0); // reset_after
                    socket.write_all(&write_buffer).await?;
                    socket.flush().await?;
                    continue;
                }
            };

            // Write response (34 bytes)
            write_buffer.clear();
            write_buffer.put_u8(1); // ok = true
            write_buffer.put_u8(if response.allowed { 1 } else { 0 });
            write_buffer.put_i64_le(response.limit);
            write_buffer.put_i64_le(response.remaining);
            write_buffer.put_i64_le(response.retry_after);
            write_buffer.put_i64_le(response.reset_after);

            socket.write_all(&write_buffer).await?;
            socket.flush().await?;
        }

        Ok(())
    }
}

#[async_trait]
impl Transport for NativeTransport {
    async fn start(self, limiter: RateLimiterHandle) -> Result<()> {
        let addr = format!("{}:{}", self.host, self.port);
        let listener = TcpListener::bind(&addr).await?;

        tracing::info!("Native protocol transport listening on {}", addr);

        let limiter = Arc::new(limiter);

        loop {
            let (socket, peer_addr) = listener.accept().await?;
            let limiter = Arc::clone(&limiter);

            tracing::debug!("New connection from {}", peer_addr);

            // Spawn a task to handle this connection
            tokio::spawn(async move {
                if let Err(e) = Self::handle_connection(socket, (*limiter).clone()).await {
                    tracing::error!("Connection error from {}: {}", peer_addr, e);
                }
                tracing::debug!("Connection closed from {}", peer_addr);
            });
        }
    }
}

use super::Transport;
use crate::actor::RateLimiterHandle;
use crate::types::ThrottleRequest;
use anyhow::Result;
use async_trait::async_trait;
use bytes::{BufMut, BytesMut};
use std::sync::Arc;
use std::time::UNIX_EPOCH;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Native binary protocol for minimal overhead
///
/// Request format (fixed size: 32 bytes + variable key length):
/// - cmd: u8 (1 byte)
/// - key_len: u8 (1 byte)
/// - burst: i64 (8 bytes)
/// - rate: i64 (8 bytes)
/// - period: i64 (8 bytes)
/// - quantity: i64 (8 bytes)
/// - timestamp: i64 (8 bytes, nanoseconds since UNIX epoch)
/// - key: [u8; key_len] (variable)
///
/// Response format (fixed size: 33 bytes):
/// - ok: u8 (1 byte)
/// - allowed: u8 (1 byte)
/// - limit: i64 (8 bytes)
/// - remaining: i64 (8 bytes)
/// - retry_after: i64 (8 bytes)
/// - reset_after: i64 (8 bytes)
const READ_BUFFER_SIZE: usize = 256;
const WRITE_BUFFER_SIZE: usize = 64;
const MAX_KEY_LENGTH: usize = 255;

pub struct NativeTransport {
    host: String,
    port: u16,
}

impl NativeTransport {
    pub fn new(host: &str, port: u16) -> Self {
        NativeTransport {
            host: host.to_string(),
            port,
        }
    }

    async fn handle_connection(mut socket: TcpStream, limiter: RateLimiterHandle) -> Result<()> {
        // Pre-allocate buffers
        let mut read_buffer = BytesMut::with_capacity(READ_BUFFER_SIZE);
        let mut write_buffer = BytesMut::with_capacity(WRITE_BUFFER_SIZE);

        // Set TCP_NODELAY for lower latency
        socket.set_nodelay(true)?;

        // Fixed-size header buffer
        let mut header = [0u8; 42]; // Max size for fixed fields

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

            // Read remaining fixed fields (40 bytes)
            socket.read_exact(&mut header[2..42]).await?;

            // Parse fixed fields
            let burst = i64::from_le_bytes(header[2..10].try_into().unwrap());
            let rate = i64::from_le_bytes(header[10..18].try_into().unwrap());
            let period = i64::from_le_bytes(header[18..26].try_into().unwrap());
            let quantity = i64::from_le_bytes(header[26..34].try_into().unwrap());
            let timestamp_nanos = i64::from_le_bytes(header[34..42].try_into().unwrap());

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

            // Convert timestamp from nanoseconds
            let timestamp = UNIX_EPOCH + std::time::Duration::from_nanos(timestamp_nanos as u64);

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
                    continue;
                }
            };

            // Write response (33 bytes)
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

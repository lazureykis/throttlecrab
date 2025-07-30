use super::{
    Transport,
    msgpack_protocol::{MsgPackRequest, MsgPackResponse},
};
use crate::actor::RateLimiterHandle;
use anyhow::Result;
use async_trait::async_trait;
use bytes::BytesMut;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Size of the read buffer - tuned for typical request sizes
const READ_BUFFER_SIZE: usize = 4096;
/// Size of the write buffer - tuned for typical response sizes  
const WRITE_BUFFER_SIZE: usize = 512;
/// Maximum message size (1MB)
const MAX_MESSAGE_SIZE: usize = 1024 * 1024;

pub struct MsgPackTransport {
    host: String,
    port: u16,
}

impl MsgPackTransport {
    pub fn new(host: &str, port: u16) -> Self {
        MsgPackTransport {
            host: host.to_string(),
            port,
        }
    }

    async fn handle_connection(mut socket: TcpStream, limiter: RateLimiterHandle) -> Result<()> {
        // Pre-allocate buffers that will be reused
        let mut read_buffer = BytesMut::with_capacity(READ_BUFFER_SIZE);
        let mut write_buffer = BytesMut::with_capacity(WRITE_BUFFER_SIZE);

        // Set TCP_NODELAY for lower latency
        socket.set_nodelay(true)?;

        tracing::debug!("Starting optimized connection handler");

        loop {
            // Read length prefix (4 bytes)
            let mut len_bytes = [0u8; 4];
            match socket.read_exact(&mut len_bytes).await {
                Ok(_) => {}
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::UnexpectedEof {
                        break; // Client disconnected
                    }
                    return Err(e.into());
                }
            }

            let len = u32::from_be_bytes(len_bytes) as usize;
            tracing::debug!("Received message with length: {}", len);

            // Validate length
            if len > MAX_MESSAGE_SIZE {
                tracing::warn!("Message too large: {} bytes", len);
                break;
            }

            // Ensure buffer has enough capacity
            if read_buffer.capacity() < len {
                read_buffer.reserve(len - read_buffer.capacity());
            }

            // Clear and resize buffer
            read_buffer.clear();
            read_buffer.resize(len, 0);

            // Read message directly into buffer
            socket.read_exact(&mut read_buffer).await?;

            // Decode request
            let response = match rmp_serde::from_slice::<MsgPackRequest>(&read_buffer) {
                Ok(request) => {
                    if request.cmd != 1 {
                        MsgPackResponse::error("Unknown command")
                    } else {
                        // Send to actor via channel
                        match limiter.throttle(request.into()).await {
                            Ok(resp) => resp.into(),
                            Err(e) => {
                                tracing::error!("Rate limiter error: {}", e);
                                MsgPackResponse::error("Internal error")
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to decode request: {}", e);
                    MsgPackResponse::error("Invalid request")
                }
            };

            // Serialize response to temporary buffer first
            let response_bytes = rmp_serde::to_vec(&response)?;

            // Clear write buffer and write length-prefixed message
            write_buffer.clear();
            let len_bytes = (response_bytes.len() as u32).to_be_bytes();
            write_buffer.extend_from_slice(&len_bytes);
            write_buffer.extend_from_slice(&response_bytes);

            // Send the entire buffer in one write
            socket.write_all(&write_buffer).await?;
            socket.flush().await?;
        }

        Ok(())
    }
}

#[async_trait]
impl Transport for MsgPackTransport {
    async fn start(self, limiter: RateLimiterHandle) -> Result<()> {
        let addr = format!("{}:{}", self.host, self.port);
        let listener = TcpListener::bind(&addr).await?;

        tracing::info!("MessagePack transport listening on {}", addr);

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

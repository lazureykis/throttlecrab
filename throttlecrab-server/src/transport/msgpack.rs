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
        let mut buffer = BytesMut::with_capacity(8192);

        loop {
            // Read length prefix (4 bytes)
            let mut len_bytes = [0u8; 4];
            match socket.read_exact(&mut len_bytes).await {
                Ok(_) => {}
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::UnexpectedEof {
                        // Client disconnected
                        break;
                    }
                    return Err(e.into());
                }
            }

            let len = u32::from_be_bytes(len_bytes) as usize;

            // Validate length
            if len > 1024 * 1024 {
                // 1MB max message size
                tracing::warn!("Message too large: {} bytes", len);
                break;
            }

            // Read message
            buffer.resize(len, 0);
            socket.read_exact(&mut buffer).await?;

            // Decode request
            let response = match rmp_serde::from_slice::<MsgPackRequest>(&buffer) {
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

            // Encode and send response
            let response_bytes = rmp_serde::to_vec(&response)?;
            let len_bytes = (response_bytes.len() as u32).to_be_bytes();

            socket.write_all(&len_bytes).await?;
            socket.write_all(&response_bytes).await?;
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

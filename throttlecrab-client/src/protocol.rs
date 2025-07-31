use bytes::{BufMut, BytesMut};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::error::{ClientError, Result};

/// Request to check rate limit
#[derive(Debug, Clone)]
pub struct ThrottleRequest {
    pub key: String,
    pub max_burst: i64,
    pub count_per_period: i64,
    pub period: i64,
    pub quantity: i64,
}

impl ThrottleRequest {
    pub fn new(key: impl Into<String>, max_burst: i64, count_per_period: i64, period: i64) -> Self {
        Self {
            key: key.into(),
            max_burst,
            count_per_period,
            period,
            quantity: 1,
        }
    }

    pub fn with_quantity(mut self, quantity: i64) -> Self {
        self.quantity = quantity;
        self
    }
}

/// Response from rate limiter
#[derive(Debug, Clone)]
pub struct ThrottleResponse {
    pub allowed: bool,
    pub limit: i64,
    pub remaining: i64,
    pub retry_after: i64,
    pub reset_after: i64,
}

/// Native protocol implementation
///
/// Request format (fixed size: 42 bytes + variable key length):
/// - cmd: u8 (1 byte)
/// - key_len: u8 (1 byte)
/// - burst: i64 (8 bytes)
/// - rate: i64 (8 bytes)
/// - period: i64 (8 bytes) - in nanoseconds
/// - quantity: i64 (8 bytes)
/// - timestamp: i64 (8 bytes, nanoseconds since UNIX epoch)
/// - key: [u8; key_len] (variable)
///
/// Response format (fixed size: 34 bytes):
/// - ok: u8 (1 byte)
/// - allowed: u8 (1 byte)
/// - limit: i64 (8 bytes)
/// - remaining: i64 (8 bytes)
/// - retry_after: i64 (8 bytes)
/// - reset_after: i64 (8 bytes)
pub struct NativeProtocol;

impl NativeProtocol {
    const CMD_THROTTLE: u8 = 1;
    const MAX_KEY_LENGTH: usize = 255;
    const REQUEST_HEADER_SIZE: usize = 42;
    const RESPONSE_SIZE: usize = 34;

    /// Send a throttle request and receive response
    pub async fn send_request(
        stream: &mut TcpStream,
        request: &ThrottleRequest,
    ) -> Result<ThrottleResponse> {
        // Validate key length
        if request.key.len() > Self::MAX_KEY_LENGTH {
            return Err(ClientError::Protocol(format!(
                "Key too long: {} bytes (max: {})",
                request.key.len(),
                Self::MAX_KEY_LENGTH
            )));
        }

        // Build request
        let mut buffer = BytesMut::with_capacity(Self::REQUEST_HEADER_SIZE + request.key.len());

        buffer.put_u8(Self::CMD_THROTTLE);
        buffer.put_u8(request.key.len() as u8);
        buffer.put_i64_le(request.max_burst);
        buffer.put_i64_le(request.count_per_period);
        buffer.put_i64_le(request.period * 1_000_000_000); // Convert seconds to nanoseconds
        buffer.put_i64_le(request.quantity);

        // Current timestamp
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as i64;
        buffer.put_i64_le(now);
        buffer.put_slice(request.key.as_bytes());

        // Send request
        stream.write_all(&buffer).await?;
        stream.flush().await?;

        // Read response
        let mut response_buf = [0u8; Self::RESPONSE_SIZE];
        stream.read_exact(&mut response_buf).await?;

        // Parse response
        let ok = response_buf[0] != 0;
        let allowed = response_buf[1] != 0;

        if !ok {
            return Err(ClientError::ServerError);
        }

        let limit = i64::from_le_bytes(response_buf[2..10].try_into().unwrap());
        let remaining = i64::from_le_bytes(response_buf[10..18].try_into().unwrap());
        let retry_after = i64::from_le_bytes(response_buf[18..26].try_into().unwrap());
        let reset_after = i64::from_le_bytes(response_buf[26..34].try_into().unwrap());

        Ok(ThrottleResponse {
            allowed,
            limit,
            remaining,
            retry_after,
            reset_after,
        })
    }
}

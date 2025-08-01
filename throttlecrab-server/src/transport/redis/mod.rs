//! Redis protocol transport for throttlecrab
//!
//! This transport provides a Redis-compatible interface for rate limiting,
//! allowing clients to use standard Redis clients to interact with throttlecrab.
//!
//! # Protocol
//!
//! Implements RESP (Redis Serialization Protocol) for communication.
//!
//! # Supported Commands
//!
//! - `THROTTLE key max_burst count_per_period period [quantity]` - Check rate limit
//! - `PING` - Health check
//! - `QUIT` - Close connection
//!
//! # Example Usage
//!
//! ```bash
//! redis-cli -p 6379
//! > THROTTLE user:123 10 100 60
//! 1) (integer) 1    # allowed
//! 2) (integer) 10   # limit
//! 3) (integer) 9    # remaining
//! 4) (integer) 60   # reset_after
//! 5) (integer) 0    # retry_after
//! ```

pub mod resp;

use self::resp::{RespParser, RespSerializer, RespValue};
use super::Transport;
use crate::actor::RateLimiterHandle;
use crate::metrics::{Metrics, Transport as MetricsTransport};
use crate::types::ThrottleRequest;
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;
use tracing::{debug, error, info};

/// Redis transport implementation
pub struct RedisTransport {
    addr: SocketAddr,
    metrics: Arc<Metrics>,
}

impl RedisTransport {
    pub fn new(host: &str, port: u16, metrics: Arc<Metrics>) -> Result<Self> {
        let addr = format!("{host}:{port}")
            .parse()
            .with_context(|| format!("Invalid address: {host}:{port}"))?;
        Ok(Self { addr, metrics })
    }
}

#[async_trait]
impl Transport for RedisTransport {
    async fn start(self, limiter: RateLimiterHandle) -> Result<()> {
        let listener = TcpListener::bind(&self.addr)
            .await
            .with_context(|| format!("Failed to bind to {}", self.addr))?;

        info!("Redis transport listening on {}", self.addr);

        loop {
            let (socket, addr) = listener.accept().await?;
            let limiter = limiter.clone();
            let metrics = Arc::clone(&self.metrics);

            tokio::spawn(async move {
                if let Err(e) = handle_connection(socket, addr, limiter, metrics).await {
                    error!("Error handling Redis connection from {}: {}", addr, e);
                }
            });
        }
    }
}

const MAX_BUFFER_SIZE: usize = 64 * 1024; // 64KB max buffer per connection

async fn handle_connection(
    mut socket: TcpStream,
    addr: SocketAddr,
    limiter: RateLimiterHandle,
    metrics: Arc<Metrics>,
) -> Result<()> {
    debug!("New Redis connection from {}", addr);

    let mut buffer = Vec::new();
    let mut parser = RespParser::new();

    loop {
        // Read data from socket with timeout
        let mut temp_buf = vec![0; 1024];
        let read_timeout = Duration::from_secs(300); // 5 minutes timeout

        let n = match timeout(read_timeout, socket.read(&mut temp_buf)).await {
            Ok(Ok(n)) => n,
            Ok(Err(e)) => return Err(e.into()),
            Err(_) => {
                debug!(
                    "Redis connection {} timed out after 5 minutes of inactivity",
                    addr
                );
                return Ok(());
            }
        };

        if n == 0 {
            debug!("Redis connection closed by client {}", addr);
            return Ok(());
        }

        buffer.extend_from_slice(&temp_buf[..n]);

        // Check buffer size limit
        if buffer.len() > MAX_BUFFER_SIZE {
            error!("Redis connection {} exceeded buffer size limit", addr);
            return Err(anyhow::anyhow!("Buffer size limit exceeded"));
        }

        // Try to parse RESP values
        while let Some((value, consumed)) = parser.parse(&buffer)? {
            buffer.drain(..consumed);

            // Check if this is a QUIT command before processing
            let is_quit = matches!(&value, RespValue::Array(arr) if arr.first().map(|v| {
                matches!(v, RespValue::BulkString(Some(cmd)) if cmd.to_uppercase() == "QUIT")
            }).unwrap_or(false));

            // Process the command
            let response = process_command(value, &limiter, &metrics).await;

            // Serialize and send response
            let response_bytes = RespSerializer::serialize(&response);
            socket.write_all(&response_bytes).await?;

            // Close connection if this was a QUIT command
            if is_quit {
                debug!("Closing Redis connection for {} after QUIT", addr);
                return Ok(());
            }
        }
    }
}

pub(super) async fn process_command(
    value: RespValue,
    limiter: &RateLimiterHandle,
    metrics: &Arc<Metrics>,
) -> RespValue {
    let start = Instant::now();

    // Parse command from array
    let command_array = match value {
        RespValue::Array(arr) => arr,
        _ => return RespValue::Error("ERR expected array of commands".to_string()),
    };

    if command_array.is_empty() {
        return RespValue::Error("ERR empty command".to_string());
    }

    // Redis commands are case-insensitive, so we uppercase them for matching
    let command = match &command_array[0] {
        RespValue::BulkString(Some(cmd)) => cmd.to_uppercase(),
        _ => return RespValue::Error("ERR invalid command format".to_string()),
    };

    let (result, key_opt) = match command.as_str() {
        "PING" => (handle_ping(&command_array), None),
        "THROTTLE" => {
            // Extract key for metrics
            let key = if command_array.len() > 1 {
                match &command_array[1] {
                    RespValue::BulkString(Some(k)) => Some(k.clone()),
                    _ => None,
                }
            } else {
                None
            };
            (handle_throttle(&command_array, limiter, metrics).await, key)
        }
        "QUIT" => (RespValue::SimpleString("OK".to_string()), None),
        _ => (
            RespValue::Error(format!("ERR unknown command '{command}'")),
            None,
        ),
    };

    let duration = start.elapsed();
    let latency_us = duration.as_micros() as u64;

    // Check if the request was allowed (for THROTTLE commands)
    let allowed = match &result {
        RespValue::Array(values) if values.len() >= 5 => {
            matches!(&values[0], RespValue::Integer(1))
        }
        _ => true, // Non-throttle commands are considered allowed
    };

    if let Some(key) = key_opt {
        metrics.record_request_with_key(MetricsTransport::Redis, latency_us, allowed, &key);
    } else {
        metrics.record_request(MetricsTransport::Redis, latency_us, allowed);
    }

    result
}

fn handle_ping(args: &[RespValue]) -> RespValue {
    if args.len() == 1 {
        RespValue::SimpleString("PONG".to_string())
    } else if args.len() == 2 {
        // PING with message
        args[1].clone()
    } else {
        RespValue::Error("ERR wrong number of arguments for 'ping' command".to_string())
    }
}

async fn handle_throttle(
    args: &[RespValue],
    limiter: &RateLimiterHandle,
    _metrics: &Arc<Metrics>,
) -> RespValue {
    // THROTTLE key max_burst count_per_period period [quantity]
    if args.len() < 5 || args.len() > 6 {
        return RespValue::Error(
            "ERR wrong number of arguments for 'throttle' command".to_string(),
        );
    }

    // Parse arguments
    let key = match &args[1] {
        RespValue::BulkString(Some(s)) => s.clone(),
        _ => return RespValue::Error("ERR invalid key".to_string()),
    };

    let max_burst = match parse_integer(&args[2]) {
        Some(n) => n,
        None => return RespValue::Error("ERR invalid max_burst".to_string()),
    };

    let count_per_period = match parse_integer(&args[3]) {
        Some(n) => n,
        None => return RespValue::Error("ERR invalid count_per_period".to_string()),
    };

    let period = match parse_integer(&args[4]) {
        Some(n) => n,
        None => return RespValue::Error("ERR invalid period".to_string()),
    };

    let quantity = if args.len() == 6 {
        match parse_integer(&args[5]) {
            Some(n) => n,
            None => return RespValue::Error("ERR invalid quantity".to_string()),
        }
    } else {
        1
    };

    // Create throttle request
    let request = ThrottleRequest {
        key,
        max_burst,
        count_per_period,
        period,
        quantity,
        timestamp: SystemTime::now(),
    };

    // Check rate limit
    match limiter.throttle(request).await {
        Ok(response) => {
            // Return array with response fields
            RespValue::Array(vec![
                RespValue::Integer(if response.allowed { 1 } else { 0 }),
                RespValue::Integer(response.limit),
                RespValue::Integer(response.remaining),
                RespValue::Integer(response.reset_after),
                RespValue::Integer(response.retry_after),
            ])
        }
        Err(e) => RespValue::Error(format!("ERR {e}")),
    }
}

fn parse_integer(value: &RespValue) -> Option<i64> {
    match value {
        RespValue::BulkString(Some(s)) => s.parse().ok(),
        RespValue::Integer(n) => Some(*n),
        _ => None,
    }
}

//! Transport layer implementations for the rate limiting server
//!
//! This module provides different network protocols for clients to communicate
//! with the rate limiter. All transports implement the [`Transport`] trait and
//! share the same underlying rate limiter state via the actor pattern.
//!
//! # Available Transports
//!
//! - [`http`]: REST API with JSON (easy integration)
//! - [`grpc`]: Protocol Buffers over HTTP/2 (service mesh friendly)
//! - [`redis`]: Redis protocol for native Redis client support

pub mod grpc;
pub mod http;
pub mod redis;

#[cfg(test)]
mod http_test;

#[cfg(test)]
mod redis_test;

#[cfg(test)]
mod redis_security_test;

use crate::actor::RateLimiterHandle;
use anyhow::Result;
use async_trait::async_trait;

/// Common interface for all transport implementations
///
/// Each transport is responsible for:
/// - Accepting client connections
/// - Parsing protocol-specific requests
/// - Forwarding requests to the rate limiter actor
/// - Sending responses back to clients
#[async_trait]
pub trait Transport {
    /// Start the transport server
    ///
    /// This method should:
    /// 1. Bind to the configured address/port
    /// 2. Accept incoming connections
    /// 3. Handle requests using the provided rate limiter
    ///
    /// The method runs indefinitely until an error occurs or the server shuts down.
    async fn start(self, limiter: RateLimiterHandle) -> Result<()>;
}

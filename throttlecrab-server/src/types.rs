//! Common types used across the server
//!
//! This module defines the core request and response types that are
//! shared between different transport protocols and the actor system.
//!
//! # Type Conversions
//!
//! Each transport protocol converts between its protocol-specific types
//! and these common types:
//!
//! - **Native**: Direct binary serialization
//! - **HTTP**: JSON serialization
//! - **gRPC**: Protocol Buffers

use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use throttlecrab::RateLimitResult;

/// Internal rate limit request structure
///
/// This is the common request format used by all transports
/// after parsing their protocol-specific formats.
///
/// # Fields
///
/// - `key`: Unique identifier for the rate limit (e.g., "user:123", "api:endpoint")
/// - `max_burst`: Maximum tokens available at once (burst capacity)
/// - `count_per_period`: Total tokens replenished per period
/// - `period`: Time period in seconds for token replenishment
/// - `quantity`: Number of tokens to consume (typically 1)
/// - `timestamp`: Request timestamp for consistent rate limiting
#[derive(Debug, Clone)]
pub struct ThrottleRequest {
    /// The key to rate limit (e.g., "user:123", "ip:192.168.1.1")
    pub key: String,
    /// Maximum burst capacity (tokens available at once)
    pub max_burst: i64,
    /// Tokens replenished per period
    pub count_per_period: i64,
    /// Period in seconds for token replenishment
    pub period: i64,
    /// Number of tokens to consume (default: 1)
    pub quantity: i64,
    /// Request timestamp for consistent rate limiting
    pub timestamp: SystemTime,
}

/// Rate limit response structure
///
/// This is the common response format returned by all transports
/// after checking a rate limit.
///
/// # Response Interpretation
///
/// - If `allowed` is true: The request can proceed
/// - If `allowed` is false: The request should be rejected
///   - Check `retry_after` to know when to retry
///   - Check `reset_after` to know when the bucket resets
///
/// # Example
///
/// ```json
/// {
///   "allowed": false,
///   "limit": 10,
///   "remaining": 0,
///   "retry_after": 30,
///   "reset_after": 60
/// }
/// ```
///
/// This response indicates the request was denied, no tokens remain,
/// retry in 30 seconds, and the bucket fully resets in 60 seconds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThrottleResponse {
    /// Whether the request is allowed
    pub allowed: bool,
    /// Maximum burst capacity
    pub limit: i64,
    /// Tokens remaining in the bucket
    pub remaining: i64,
    /// Seconds until the bucket fully resets
    pub reset_after: i64,
    /// Seconds until the next request can be made (0 if allowed)
    pub retry_after: i64,
}

impl From<(bool, RateLimitResult)> for ThrottleResponse {
    fn from((allowed, result): (bool, RateLimitResult)) -> Self {
        ThrottleResponse {
            allowed,
            limit: result.limit,
            remaining: result.remaining,
            reset_after: result.reset_after.as_secs() as i64,
            retry_after: result.retry_after.as_secs() as i64,
        }
    }
}

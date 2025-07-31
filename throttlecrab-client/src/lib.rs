//! High-performance client library for throttlecrab rate limiting server
//!
//! This crate provides an async client for connecting to throttlecrab servers
//! using the native binary protocol with connection pooling support.

pub mod client;
pub mod client_v2;
pub mod error;
pub mod pool;
pub mod pool_v2;
pub mod protocol;

pub use client::ThrottleCrabClient;
pub use error::{ClientError, Result};
pub use pool::{ConnectionPool, PoolConfig};
pub use protocol::{ThrottleRequest, ThrottleResponse};

// Re-export commonly used items
pub use client::ClientBuilder;

// V2 exports
pub use client_v2::{ClientBuilderV2, ThrottleCrabClientV2};

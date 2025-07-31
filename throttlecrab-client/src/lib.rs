//! High-performance client library for throttlecrab rate limiting server
//!
//! This crate provides an async client for connecting to throttlecrab servers
//! using the native binary protocol with connection pooling support.

pub mod client;
pub mod error;
pub mod pool;
pub mod protocol;

pub use client::{ClientBuilder, ThrottleCrabClient};
pub use error::{ClientError, Result};
pub use pool::{ConnectionPool, PoolConfig};
pub use protocol::{ThrottleRequest, ThrottleResponse};

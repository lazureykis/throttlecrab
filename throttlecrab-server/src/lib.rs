//! # ThrottleCrab Server
//!
//! A high-performance, standalone rate limiting service with multiple protocol support.
//!
//! ## Purpose
//!
//! ThrottleCrab Server solves the problem of distributed rate limiting by providing
//! a centralized service that multiple applications can use to enforce rate limits.
//! Instead of implementing rate limiting logic in every service, you can:
//!
//! - **Centralize rate limiting logic** in one place
//! - **Share rate limit state** across multiple services
//! - **Enforce consistent policies** across your entire infrastructure
//! - **Scale independently** from your application services
//!
//! ## Use Cases
//!
//! - **API Gateway Rate Limiting**: Protect backend services from overload
//! - **Multi-Service Rate Limiting**: Share rate limits across microservices
//! - **User Action Throttling**: Prevent abuse across multiple endpoints
//! - **Resource Protection**: Guard expensive operations with global limits
//!
//! ## Installation
//!
//! ```bash
//! cargo install throttlecrab-server
//! ```
//!
//! ## Quick Start
//!
//! ```bash
//! # Show all available options
//! throttlecrab-server --help
//!
//! # Start with http transport on port 8080
//! throttlecrab-server --http --http-port 8080
//!
//! # Enable multiple protocols
//! throttlecrab-server --http --grpc
//!
//! # Custom store configuration
//! throttlecrab-server --http --http-port 8080 --store adaptive --store-capacity 100000 --store-cleanup-interval 60
//! ```
//!
//! ## Configuration
//!
//! Configure via CLI arguments or environment variables (CLI takes precedence):
//!
//! ```bash
//! # Via CLI
//! throttlecrab-server --http --http-port 9090 --store periodic
//!
//! # Via environment variables
//! export THROTTLECRAB_HTTP=true
//! export THROTTLECRAB_HTTP_PORT=9090
//! export THROTTLECRAB_STORE=periodic
//! throttlecrab-server
//!
//! # List all available environment variables
//! throttlecrab-server --list-env-vars
//! ```
//!
//! ### Key Configuration Options
//!
//! - **Transports**: Enable with `--http`, `--grpc` (at least one required)
//! - **Ports**: `--http-port 8080`, `--grpc-port 50051`
//! - **Store Type**: `--store periodic|probabilistic|adaptive`
//! - **Store Capacity**: `--store-capacity 100000`
//! - **Log Level**: `--log-level error|warn|info|debug|trace`
//!
//! ## How It Works
//!
//! The server uses GCRA (Generic Cell Rate Algorithm) with a token bucket approach:
//! - Each key gets a bucket with `max_burst` capacity
//! - Tokens refill at `count_per_period / period` per second
//! - Requests consume `quantity` tokens (default: 1)
//! - Denied requests receive `retry_after` and `reset_after` times
//!
//! ## Available Protocols
//!
//! - **HTTP/JSON**: Easy integration with any programming language (173K req/s)
//! - **gRPC**: Service mesh and microservices integration (163K req/s)
//!
//! All protocols share the same underlying rate limiter, ensuring consistent
//! rate limiting across different client types. RPS are provided for comparison
//! purposes only.
//!
//! ## Architecture
//!
//! The server uses an actor-based architecture with Tokio for async I/O:
//!
//! ```text
//! ┌─────────────┐   ┌─────────────┐   ┌─────────────┐
//!     │    HTTP     │   │    gRPC     │
//! │  Transport  │   │  Transport  │   │  Transport  │
//! └──────┬──────┘   └──────┬──────┘   └──────┬──────┘
//!        │                 │                 │
//!        └─────────────────┴─────────────────┘
//!                          │
//!                    ┌─────▼─────┐
//!                    │   Actor   │
//!                    │  (Shared  │
//!                    │   State)  │
//!                    └─────┬─────┘
//!                          │
//!                    ┌─────▼─────┐
//!                    │RateLimiter│
//!                    │   Store   │
//!                    └───────────┘
//! ```
//!
//! ## Performance
//!
//! Benchmark results on modern hardware:
//!
//! | Protocol | Throughput | P99 Latency | P50 Latency |
//! |----------|------------|-------------|-------------|
//! | HTTP     | 173K req/s | 309 μs      | 177 μs      |
//! | gRPC     | 163K req/s | 370 μs      | 186 μs      |
//!
//! ## Usage
//!
//! ### Starting the Server
//!
//! ```bash
//! # HTTP protocol only
//! throttlecrab-server --http --http-port 8080
//!
//! # All protocols
//! throttlecrab-server --http --grpc
//! ```
//!
//! ### Client Examples
//!
//! #### HTTP Protocol (curl)
//! ```bash
//! curl -X POST http://localhost:8080/throttle \
//!   -H "Content-Type: application/json" \
//!   -d '{"key": "user:123", "max_burst": 10, "count_per_period": 100, "period": 60}'
//! ```
//!
//! #### gRPC Protocol
//! Use any gRPC client library with the provided protobuf definitions.

pub mod actor;
pub mod config;
pub mod metrics;
pub mod store;
pub mod transport;
pub mod types;

// Re-export grpc types for tests
pub mod grpc {
    pub use crate::transport::grpc::throttlecrab_proto::*;
}

# throttlecrab

[![CI](https://github.com/lazureykis/throttlecrab/actions/workflows/ci.yml/badge.svg)](https://github.com/lazureykis/throttlecrab/actions/workflows/ci.yml)
[![Coverage Status](https://codecov.io/gh/lazureykis/throttlecrab/branch/master/graph/badge.svg)](https://codecov.io/gh/lazureykis/throttlecrab)
[![Crates.io](https://img.shields.io/crates/v/throttlecrab.svg)](https://crates.io/crates/throttlecrab)
[![Documentation](https://docs.rs/throttlecrab/badge.svg)](https://docs.rs/throttlecrab)
[![License](https://img.shields.io/crates/l/throttlecrab.svg)](LICENSE-MIT)

A high-performance GCRA (Generic Cell Rate Algorithm) rate limiter for Rust. Inspired by [redis-cell](https://github.com/brandur/redis-cell), throttlecrab offers a pure Rust implementation with multiple storage backends and deployment options.

## Project Structure

This workspace contains three crates:

| Crate | Description | Use Case |
|-------|-------------|----------|
| [`throttlecrab`](./throttlecrab) | Core rate limiting library | Embed rate limiting in your Rust application |
| [`throttlecrab-server`](./throttlecrab-server) | Standalone server with multiple protocols | Distributed rate limiting service |
| [`throttlecrab-client`](./throttlecrab-client) | Native async client library | High-performance client for throttlecrab-server |

## Quick Start

### As a Library

```rust
use throttlecrab::{RateLimiter, AdaptiveStore};
use std::time::SystemTime;

// Create a rate limiter with adaptive store (best performance)
let mut limiter = RateLimiter::new(AdaptiveStore::new());

// Check rate limit: 10 burst, 100 requests per 60 seconds
let (allowed, result) = limiter
    .rate_limit("user:123", 10, 100, 60, 1, SystemTime::now())
    .unwrap();

if allowed {
    println!("Request allowed! Remaining: {}", result.remaining);
} else {
    println!("Rate limited! Retry after: {} seconds", result.retry_after);
}
```

### As a Server

```bash
# Install the server
cargo install throttlecrab-server

# Run with native protocol (best performance)
throttlecrab-server --native --native-port 9090

# Run with HTTP for easy integration
throttlecrab-server --http --http-port 8080
```

### With Client Library

```rust
use throttlecrab_client::ThrottleCrabClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = ThrottleCrabClient::connect("127.0.0.1:9090").await?;

    let response = client
        .check_rate_limit("user:123", 10, 100, 60)
        .await?;

    if response.allowed {
        // Process request
    }
    Ok(())
}
```

## Features

### Core Library (`throttlecrab`)
- **GCRA Algorithm**: Smooth rate limiting without sudden spikes or drops
- **Multiple Store Types**:
  - `AdaptiveStore`: Self-tuning cleanup intervals (recommended)
  - `PeriodicStore`: Fixed interval cleanup
  - `ProbabilisticStore`: Random sampling cleanup
- **Zero Dependencies**: Pure Rust implementation
- **Thread-Safe**: Can be used with `Arc<Mutex<>>` for concurrent access

### Server (`throttlecrab-server`)
- **Multiple Protocols**:
  - **Native binary**: Highest performance, minimal overhead
  - **HTTP/JSON**: REST API for easy integration
  - **gRPC**: Service mesh and microservices
  - **MessagePack**: Good balance of performance and compatibility
- **Shared State**: All protocols share the same rate limiter store
- **Production Ready**: Health checks, metrics, configurable logging
- **Flexible Deployment**: Docker, systemd, or standalone binary

### Client Library (`throttlecrab-client`)
- **Connection Pooling**: Reuse connections for better performance
- **Async/Await**: Full Tokio integration
- **Automatic Reconnection**: Handles network failures gracefully
- **Type-Safe**: Strongly typed request/response API

## Performance

### Store Type Performance

`cd integration-tests && ./run-transport-test.sh -t all -T 32 -r 10000`

| Store Type | Best For | Cleanup Strategy | Memory Usage |
|------------|----------|------------------|---------------|
| Adaptive | Variable workloads | Self-tuning intervals | Dynamic |
| Periodic | Predictable load | Fixed intervals | Predictable |
| Probabilistic | High throughput | Random sampling | Efficient |

## When to Use ThrottleCrab

### Use the Library When:
- Building a Rust application that needs rate limiting
- Want zero network overhead
- Need custom storage backends
- Require fine-grained control over the algorithm

### Use the Server When:
- Building a microservices architecture
- Need language-agnostic rate limiting
- Want centralized rate limit management
- Require high availability with multiple instances

### Use the Client Library When:
- Building Rust services that connect to throttlecrab-server
- Need maximum performance from the server
- Want connection pooling and automatic retries

## Common Use Cases

### API Rate Limiting
```rust
// Limit each API key to 1000 requests per minute with burst of 50
let (allowed, result) = limiter
    .rate_limit(&api_key, 50, 1000, 60, 1, SystemTime::now())?;

if !allowed {
    return Err("Rate limit exceeded, retry after {} seconds", result.retry_after);
}
```

### User Action Throttling
```rust
// Limit password reset attempts: 3 per hour, no burst
let (allowed, _) = limiter
    .rate_limit(&format!("password_reset:{}", user_id), 1, 3, 3600, 1, SystemTime::now())?;
```

### Resource Protection
```rust
// Limit expensive operations: 10 per minute with burst of 2
let (allowed, _) = limiter
    .rate_limit("expensive_operation", 2, 10, 60, 1, SystemTime::now())?;
```

## Getting Started

### Installation

```toml
# For library usage
[dependencies]
throttlecrab = "0.1"

# For client usage
[dependencies]
throttlecrab-client = "0.1"
tokio = { version = "1", features = ["full"] }
```

### Running the Server

```bash
# Install
cargo install throttlecrab-server

# Run with Native protocol (recommended for production)
throttlecrab-server --native --native-port 9090 --store adaptive

# Run with multiple protocols
throttlecrab-server --native --http --grpc \
    --native-port 9090 \
    --http-port 8080 \
    --grpc-port 50051

# Run with custom configuration
throttlecrab-server --native \
    --store adaptive \
    --store-capacity 1000000 \
    --log-level info
```

### Docker Deployment

```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release -p throttlecrab-server

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/throttlecrab-server /usr/local/bin/
EXPOSE 9090 8080 50051
CMD ["throttlecrab-server", "--native", "--http", "--grpc"]
```

### Systemd Service

```ini
[Unit]
Description=ThrottleCrab Rate Limiting Server
After=network.target

[Service]
Type=simple
User=throttlecrab
ExecStart=/usr/local/bin/throttlecrab-server --native --native-port 9090 --store adaptive
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

## Protocol Documentation

### Native Protocol (Recommended)

Optimized binary protocol with minimal overhead:

```rust
// Request format (42 bytes + variable key):
// - cmd: u8 (1 byte)
// - key_len: u8 (1 byte)
// - burst: i64 (8 bytes)
// - rate: i64 (8 bytes)
// - period: i64 (8 bytes)
// - quantity: i64 (8 bytes)
// - timestamp: i64 (8 bytes)
// - key: [u8; key_len] (variable, max 255)

// Response format (34 bytes):
// - ok: u8 (1 byte)
// - allowed: u8 (1 byte)
// - limit: i64 (8 bytes)
// - remaining: i64 (8 bytes)
// - retry_after: i64 (8 bytes)
// - reset_after: i64 (8 bytes)
```

### HTTP REST API

**Endpoint**: `POST /throttle`

```bash
curl -X POST http://localhost:8080/throttle \
  -H "Content-Type: application/json" \
  -d '{
    "key": "user:123",
    "max_burst": 10,
    "count_per_period": 100,
    "period": 60
  }'
```

### MessagePack Protocol

Framed protocol with MessagePack encoding:
- 4-byte message length (big-endian)
- MessagePack-encoded request/response

### gRPC Protocol

See `proto/throttlecrab.proto` for the service definition.

## Testing & Benchmarking

### Running Tests

```bash
# Run all tests
cargo test --all

# Run integration tests
cd integration-tests
cargo test --release

# Run benchmarks
cd throttlecrab
cargo bench
```

### Performance Testing

The project includes comprehensive performance testing tools:

```bash
# Run server benchmarks
cd throttlecrab-server/tests
./run-benchmarks.sh

# Run custom performance test
cd integration-tests
./run-custom-test.sh 50 10000  # 50 threads, 10k requests each
```

### Load Testing Example

```bash
# Start server
throttlecrab-server --native --store adaptive

# In another terminal, run load test
cd integration-tests
./run-perf-test.sh
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

### Development Setup

```bash
# Clone the repository
git clone https://github.com/lazureykis/throttlecrab
cd throttlecrab

# Build all components
cargo build --all

# Run tests
cargo test --all

# Run lints
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
```

## Production Deployment

### Performance Tuning

```bash
# Optimal configuration for production
throttlecrab-server \
    --native --native-port 9090 \
    --store adaptive \
    --store-capacity 1000000 \
    --buffer-size 100000 \
    --log-level warn
```

### Monitoring

- **Health Check**: `GET /health` returns 200 OK
- **Metrics**: Internal performance metrics available via logs
- **Resource Usage**: Monitor memory usage based on active keys

### Scaling Strategies

#### Vertical Scaling
A single instance can handle:
- 500K+ requests/second (native protocol)
- 1M+ unique keys in memory
- Sub-millisecond P99 latency

#### Horizontal Scaling
For extreme scale, use client-side sharding:

```rust
use throttlecrab_client::{ThrottleCrabClient, ClientBuilder};

// Create a sharded client pool
struct ShardedRateLimiter {
    clients: Vec<ThrottleCrabClient>,
}

impl ShardedRateLimiter {
    async fn check_limit(&self, key: &str, burst: i64, rate: i64, period: i64) -> Result<bool> {
        let shard = self.get_shard(key);
        let response = self.clients[shard]
            .check_rate_limit(key, burst, rate, period)
            .await?;
        Ok(response.allowed)
    }

    fn get_shard(&self, key: &str) -> usize {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish() as usize % self.clients.len()
    }
}
```

### Migration from Redis-Cell

1. **Algorithm**: Same GCRA implementation
2. **Performance**: 5-100x faster depending on protocol
3. **API**: Similar request/response structure
4. **Key differences**:
   - No Redis dependency
   - Multiple protocol options
   - Better performance characteristics
   - Native Rust client available

## Related Projects

- [redis-cell](https://github.com/brandur/redis-cell) - Redis module implementing GCRA (inspiration for this project)
- [governor](https://github.com/antifuchs/governor) - Another Rust rate limiter with different design goals
- [leaky-bucket](https://github.com/udoprog/leaky-bucket) - Async rate limiter based on leaky bucket algorithm

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

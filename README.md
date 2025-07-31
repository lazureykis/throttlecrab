# ThrottleCrab

[![CI](https://github.com/lazureykis/throttlecrab/actions/workflows/ci.yml/badge.svg)](https://github.com/lazureykis/throttlecrab/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/throttlecrab.svg)](https://crates.io/crates/throttlecrab)
[![Docker](https://img.shields.io/docker/v/lazureykis/throttlecrab?label=docker)](https://hub.docker.com/r/lazureykis/throttlecrab)
[![Documentation](https://docs.rs/throttlecrab/badge.svg)](https://docs.rs/throttlecrab)
[![License](https://img.shields.io/crates/l/throttlecrab.svg)](LICENSE-MIT)

A high-performance GCRA (Generic Cell Rate Algorithm) rate limiter for Rust. ThrottleCrab offers a pure Rust implementation with multiple storage backends and deployment options.

## Project Structure

This workspace contains three crates:

| Crate | Description | Use Case |
|-------|-------------|----------|
| [`throttlecrab`](./throttlecrab) | Core rate limiting library | Embed rate limiting in your Rust application |
| [`throttlecrab-server`](./throttlecrab-server) | Standalone server with multiple protocols | Distributed rate limiting service |

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

# Run with native protocol (best performance - up to 183K req/s)
throttlecrab-server --native --native-port 9090

# Run with HTTP for easy integration (173K req/s with minimal overhead)
throttlecrab-server --http --http-port 8080
```

### Client Integration

The HTTP/JSON protocol makes it easy to integrate with any programming language or tool:

```bash
# Check rate limit with curl
curl -X POST http://localhost:8080/throttle \
  -H "Content-Type: application/json" \
  -d '{
    "key": "user:123",
    "max_burst": 10,
    "count_per_period": 100,
    "period": 60,
    "quantity": 1
  }'

# Response:
# {
#   "allowed": true,
#   "limit": 10,
#   "remaining": 9,
#   "retry_after": 0,
#   "reset_after": 60
# }
```

The `quantity` parameter is optional (defaults to 1) and allows you to check/consume multiple tokens at once.

For production applications, use your language's HTTP client with connection pooling.

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
  - **Native binary**: Highest performance (183K req/s), minimal overhead
  - **HTTP/JSON**: REST API for easy integration (173K req/s - only 6% slower)
  - **gRPC**: Service mesh and microservices (163K req/s)
- **Shared State**: All protocols share the same rate limiter store
- **Production Ready**: Health checks, metrics, configurable logging
- **Flexible Deployment**: Docker, systemd, or standalone binary

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

#### Using Pre-built Image

```bash
# Pull the latest image
docker pull lazureykis/throttlecrab:latest

# Run with default settings (all protocols enabled)
docker run -d \
  --name throttlecrab \
  -p 8080:8080 \
  -p 50051:50051 \
  -p 8072:8072 \
  lazureykis/throttlecrab:latest

# Run with custom configuration
docker run -d \
  --name throttlecrab \
  -p 8080:8080 \
  -e THROTTLECRAB_HTTP=true \
  -e THROTTLECRAB_GRPC=false \
  -e THROTTLECRAB_NATIVE=false \
  -e THROTTLECRAB_STORE=adaptive \
  -e THROTTLECRAB_STORE_CAPACITY=1000000 \
  -e THROTTLECRAB_LOG_LEVEL=info \
  lazureykis/throttlecrab:latest
```

#### Using Docker Compose

```yaml
# docker-compose.yml
version: '3.8'

services:
  throttlecrab:
    image: lazureykis/throttlecrab:latest
    container_name: throttlecrab-server
    ports:
      - "8080:8080"   # HTTP
      - "50051:50051" # gRPC
      - "8072:8072"   # Native
    environment:
      THROTTLECRAB_STORE: "adaptive"
      THROTTLECRAB_STORE_CAPACITY: "100000"
      THROTTLECRAB_LOG_LEVEL: "info"
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
      interval: 30s
      timeout: 3s
      retries: 3
```

Then run:
```bash
docker-compose up -d
```

#### Building Your Own Image

```dockerfile
# Use the provided Dockerfile in the repository
docker build -t my-throttlecrab .
docker run -d -p 8080:8080 my-throttlecrab
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
// - period: i64 (8 bytes, seconds)
// - quantity: i64 (8 bytes)
// - timestamp: i64 (8 bytes, nanoseconds since UNIX epoch)
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

## Time Synchronization

### Important Considerations

ThrottleCrab uses system time for rate limiting calculations. When deploying in distributed environments:

1. **Client-Server Time Sync**: Ensure all clients and servers have synchronized clocks (use NTP)
   - Time drift between systems can affect rate limiting accuracy
   - Consider using server-provided timestamps for consistency

2. **Time Adjustments**: ThrottleCrab handles system time changes gracefully:
   - If time goes backwards, it starts a fresh rate limiting window
   - No panics or service interruptions during NTP adjustments
   - Uses saturating arithmetic to prevent overflow issues

3. **Best Practices**:
   - Use NTP to keep all systems synchronized within 1-2 seconds
   - For critical applications, use the server's timestamp in requests
   - Monitor clock drift between your systems
   - Consider using monotonic clocks for interval measurements

4. **Protocol Support**:
   - Native protocol includes timestamp in requests
   - HTTP clients can include timestamp in request body
   - Server can optionally use its own time if client timestamps are unreliable

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

## Related Projects

- [redis-cell](https://github.com/brandur/redis-cell) - Redis module implementing GCRA
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

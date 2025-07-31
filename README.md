# throttlecrab

[![CI](https://github.com/lazureykis/throttlecrab/actions/workflows/ci.yml/badge.svg)](https://github.com/lazureykis/throttlecrab/actions/workflows/ci.yml)
[![Coverage Status](https://codecov.io/gh/lazureykis/throttlecrab/branch/master/graph/badge.svg)](https://codecov.io/gh/lazureykis/throttlecrab)
[![Crates.io](https://img.shields.io/crates/v/throttlecrab.svg)](https://crates.io/crates/throttlecrab)
[![Documentation](https://docs.rs/throttlecrab/badge.svg)](https://docs.rs/throttlecrab)
[![License](https://img.shields.io/crates/l/throttlecrab.svg)](LICENSE-MIT)

A high-performance GCRA (Generic Cell Rate Algorithm) rate limiter for Rust. This workspace contains two crates:
- `throttlecrab` - Pure Rust rate limiting library
- `throttlecrab-server` - Standalone server with multiple protocol support

## Features

- **Pure Rust library**: Zero-dependency GCRA rate limiter implementation
- **GCRA algorithm**: Implements the Generic Cell Rate Algorithm for smooth and predictable rate limiting
- **High performance**: Lock-free design with minimal overhead
- **Flexible parameters**: Different rate limits per key with dynamic configuration
- **TTL support**: Automatic cleanup of expired entries
- **Standalone server**: Multiple protocol support for distributed rate limiting:
  - **MessagePack over TCP**: Most efficient, minimal overhead
  - **HTTP with JSON**: Standard REST API for easy integration
  - **gRPC**: For service mesh and microservices

## Installation

### As a Library

Add this to your `Cargo.toml`:

```toml
[dependencies]
throttlecrab = "0.1.0"
```

### As a Server

Install the server binary with cargo:

```bash
cargo install throttlecrab-server
```

Or build from source:

```bash
git clone https://github.com/lazureykis/throttlecrab
cd throttlecrab
cargo build --release -p throttlecrab-server
./target/release/throttlecrab
```

## Usage

### Library Usage

```rust
use throttlecrab::{RateLimiter, MemoryStore};
use std::time::SystemTime;

fn main() {
    // Create a rate limiter with an in-memory store
    let mut limiter = RateLimiter::new(MemoryStore::new());
    
    // Check if a request is allowed
    // Parameters: key, max_burst, count_per_period, period (seconds), quantity, timestamp
    let (allowed, result) = limiter
        .rate_limit("api_key_123", 10, 100, 60, 1, SystemTime::now())
        .unwrap();
    
    if allowed {
        println!("Request allowed! Remaining: {}", result.remaining);
    } else {
        println!("Rate limit exceeded! Retry after: {:?}", result.retry_after);
    }
}
```

### Running the Server

Start the throttlecrab server:

```bash
# Install the server binary
cargo install throttlecrab --features bin

# Run with default settings (listens on 127.0.0.1:9090)
throttlecrab --server

# Or with custom address
throttlecrab --server --host 0.0.0.0 --port 8080

# Use different transports:
throttlecrab --server --http      # HTTP with JSON (REST API)
throttlecrab --server --grpc      # gRPC transport
throttlecrab --server             # MessagePack over TCP (default, most efficient)
```

### Client Example

The server uses MessagePack protocol over TCP with a specific wire format. Here's an example client:

```rust
use std::net::TcpStream;
use std::io::{Read, Write};
use std::time::{SystemTime, UNIX_EPOCH};

fn check_rate_limit(key: &str, max_burst: i64, rate: i64, period: i64) -> bool {
    let mut stream = TcpStream::connect("127.0.0.1:9090").unwrap();
    
    // Create request with wire protocol format
    let request = Request {
        cmd: 1, // throttle command
        key: key.to_string(),
        burst: max_burst,
        rate: rate,
        period,
        quantity: 1,
        timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as i64,
    };
    
    // Serialize with MessagePack and send
    let data = rmp_serde::to_vec(&request).unwrap();
    let len = (data.len() as u32).to_be_bytes();
    stream.write_all(&len).unwrap();
    stream.write_all(&data).unwrap();
    
    // Read response
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).unwrap();
    let len = u32::from_be_bytes(len_buf) as usize;
    
    let mut data = vec![0u8; len];
    stream.read_exact(&mut data).unwrap();
    
    let response: Response = rmp_serde::from_slice(&data).unwrap();
    response.allowed
}
```

## Architecture

### Library
The core library (`throttlecrab`) provides a pure Rust implementation of GCRA with:
- `RateLimiter`: The main rate limiting engine
- `Store` trait: Abstract storage interface
- `MemoryStore`: In-memory storage implementation

### Server
The optional server binary provides:
- TCP server with MessagePack or gRPC protocol
- Actor-based concurrency model using Tokio
- Thread-safe rate limiting for distributed systems
- **Shared state across transports**: All enabled transports (HTTP, gRPC, MessagePack, Native) share the same rate limiter store, ensuring consistent rate limiting regardless of which protocol clients use

## Protocol

### MessagePack Protocol

The default server uses a simple framed protocol:
1. 4-byte message length (big-endian)
2. MessagePack-encoded request/response

Request fields:
- `cmd`: Command type (1 = throttle)
- `key`: Unique identifier for rate limiting
- `burst`: Maximum burst capacity
- `rate`: Number of requests allowed per period
- `period`: Time period in seconds
- `quantity`: Number of tokens to consume (default: 1)
- `timestamp`: Unix timestamp in nanoseconds (default: current time)

Response fields:
- `ok`: Boolean indicating success
- `allowed`: 0 or 1 indicating if request is allowed
- `limit`: The burst limit
- `remaining`: Tokens remaining in current window
- `reset_after`: Time until full capacity reset (seconds)
- `retry_after`: Time until next request allowed (seconds)

### HTTP REST API

When running with `--http`, the server exposes a REST API:

**Endpoint**: `POST /throttle`

**Request Body** (JSON):
```json
{
  "key": "user:123",
  "max_burst": 10,
  "count_per_period": 100,
  "period": 60,
  "quantity": 1
}
```

**Response** (JSON):
```json
{
  "allowed": true,
  "limit": 10,
  "remaining": 9,
  "reset_after": 60,
  "retry_after": 0
}
```

**Example**:
```bash
curl -X POST http://localhost:9090/throttle \
  -H "Content-Type: application/json" \
  -d '{"key":"api_key_123","max_burst":10,"count_per_period":100,"period":60}'
```

### gRPC Protocol

When running with `--grpc`, the server exposes a gRPC service defined in `proto/throttlecrab.proto`:

```protobuf
service RateLimiter {
    rpc Throttle(ThrottleRequest) returns (ThrottleResponse);
}
```

To use the gRPC server:
1. Install protoc: `brew install protobuf` (macOS) or `apt-get install protobuf-compiler` (Ubuntu)
2. Run server: `throttlecrab --server --grpc`
3. Use any gRPC client library to connect

## What is GCRA?

The Generic Cell Rate Algorithm (GCRA) is a rate limiting algorithm that provides:
- **Smooth traffic shaping**: No sudden bursts followed by long waits
- **Precise rate limiting**: Exact control over request rates
- **Fairness**: All clients get predictable access to resources
- **Memory efficiency**: O(1) space per key

GCRA works by tracking the "Theoretical Arrival Time" (TAT) of requests, ensuring consistent spacing between allowed requests while permitting controlled bursts.

## Scaling Strategy

ThrottleCrab is designed for single-instance performance, but can be scaled horizontally using a sharding approach:

### Single Instance Performance
A single ThrottleCrab instance can handle hundreds of thousands of requests per second on modern hardware, which is sufficient for most use cases.

### Horizontal Scaling with Sharding
For extreme scale, you can run multiple ThrottleCrab instances and shard by key:

```rust
// Client-side sharding example
fn get_rate_limiter_instance(key: &str, instances: &[String]) -> &str {
    let hash = calculate_hash(key);
    let shard_index = hash % instances.len();
    &instances[shard_index]
}

// Use consistent hashing for better distribution
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

fn calculate_hash(key: &str) -> usize {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    hasher.finish() as usize
}
```

### Sharding Best Practices
1. **Consistent Hashing**: Use consistent hashing to minimize reshuffling when adding/removing instances
2. **Key Design**: Design your rate limit keys to distribute evenly (e.g., include user IDs)
3. **Health Checks**: Implement health checks and failover for high availability
4. **Monitoring**: Track key distribution across shards to detect hot spots

### Example Deployment
```yaml
# docker-compose.yml for 3-shard deployment
version: '3'
services:
  throttlecrab-1:
    image: throttlecrab:latest
    command: ["--server", "--host", "0.0.0.0", "--port", "9090"]
    
  throttlecrab-2:
    image: throttlecrab:latest
    command: ["--server", "--host", "0.0.0.0", "--port", "9090"]
    
  throttlecrab-3:
    image: throttlecrab:latest
    command: ["--server", "--host", "0.0.0.0", "--port", "9090"]
```

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
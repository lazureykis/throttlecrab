# throttlecrab

[![CI](https://github.com/lazureykis/throttlecrab/actions/workflows/ci.yml/badge.svg)](https://github.com/lazureykis/throttlecrab/actions/workflows/ci.yml)
[![Coverage Status](https://codecov.io/gh/lazureykis/throttlecrab/branch/master/graph/badge.svg)](https://codecov.io/gh/lazureykis/throttlecrab)
[![Crates.io](https://img.shields.io/crates/v/throttlecrab.svg)](https://crates.io/crates/throttlecrab)
[![Documentation](https://docs.rs/throttlecrab/badge.svg)](https://docs.rs/throttlecrab)
[![License](https://img.shields.io/crates/l/throttlecrab.svg)](LICENSE-MIT)

A high-performance GCRA (Generic Cell Rate Algorithm) rate limiter library for Rust with a standalone server.

## Features

- **Pure Rust library**: Zero-dependency GCRA rate limiter implementation
- **GCRA algorithm**: Implements the Generic Cell Rate Algorithm for smooth and predictable rate limiting
- **High performance**: Lock-free design with minimal overhead
- **Flexible parameters**: Different rate limits per key with dynamic configuration
- **TTL support**: Automatic cleanup of expired entries
- **Standalone server**: Optional TCP server with MessagePack protocol for distributed rate limiting

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
cargo install throttlecrab --features bin
```

Or build from source:

```bash
git clone https://github.com/lazureykis/throttlecrab
cd throttlecrab
cargo build --release --features bin
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

# Run with default settings (listens on 127.0.0.1:7777)
throttlecrab

# Or with custom address
throttlecrab --listen 0.0.0.0:8080
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
        timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64,
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
- TCP server with MessagePack protocol
- Actor-based concurrency model using Tokio
- Thread-safe rate limiting for distributed systems

## Protocol

The server uses a simple framed protocol:
1. 4-byte message length (big-endian)
2. MessagePack-encoded request/response

Request fields:
- `cmd`: Command type (1 = throttle)
- `key`: Unique identifier for rate limiting
- `burst`: Maximum burst capacity
- `rate`: Number of requests allowed per period
- `period`: Time period in seconds
- `quantity`: Number of tokens to consume (default: 1)
- `timestamp`: Unix timestamp in seconds (default: current time)

Response fields:
- `ok`: Boolean indicating success
- `allowed`: 0 or 1 indicating if request is allowed
- `limit`: The burst limit
- `remaining`: Tokens remaining in current window
- `reset_after`: Time until full capacity reset (seconds)
- `retry_after`: Time until next request allowed (seconds)

## What is GCRA?

The Generic Cell Rate Algorithm (GCRA) is a rate limiting algorithm that provides:
- **Smooth traffic shaping**: No sudden bursts followed by long waits
- **Precise rate limiting**: Exact control over request rates
- **Fairness**: All clients get predictable access to resources
- **Memory efficiency**: O(1) space per key

GCRA works by tracking the "Theoretical Arrival Time" (TAT) of requests, ensuring consistent spacing between allowed requests while permitting controlled bursts.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
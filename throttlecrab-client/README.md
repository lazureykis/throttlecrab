# throttlecrab-client

[![Crates.io](https://img.shields.io/crates/v/throttlecrab-client.svg)](https://crates.io/crates/throttlecrab-client)
[![Documentation](https://docs.rs/throttlecrab-client/badge.svg)](https://docs.rs/throttlecrab-client)
[![License](https://img.shields.io/crates/l/throttlecrab-client.svg)](LICENSE-MIT)

High-performance async client library for [throttlecrab](https://crates.io/crates/throttlecrab-server) rate limiting server.

## Features

- **Native Protocol Support**: Optimized binary protocol for minimal overhead
- **Connection Pooling**: Built-in connection pool with configurable limits
- **Async/Await**: Full tokio-based async implementation
- **High Performance**: Minimal allocations and efficient protocol encoding
- **Configurable**: Timeouts, pool sizes, and TCP options

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
throttlecrab-client = "0.1"
tokio = { version = "1", features = ["full"] }
```

## Quick Start

```rust
use throttlecrab_client::ThrottleCrabClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to throttlecrab server
    let client = ThrottleCrabClient::connect("127.0.0.1:9090").await?;

    // Check rate limit
    let response = client
        .check_rate_limit(
            "user:123",  // key
            10,          // max burst
            100,         // count per period
            60,          // period in seconds
        )
        .await?;

    if response.allowed {
        println!("Request allowed! Remaining: {}", response.remaining);
    } else {
        println!("Rate limited! Retry after: {} seconds", response.retry_after);
    }

    Ok(())
}
```

## Advanced Configuration

```rust
use throttlecrab_client::ClientBuilder;
use std::time::Duration;

let client = ClientBuilder::new()
    .max_connections(20)
    .min_idle_connections(5)
    .connect_timeout(Duration::from_secs(10))
    .request_timeout(Duration::from_secs(2))
    .tcp_nodelay(true)
    .build("127.0.0.1:9090")
    .await?;
```

## Connection Pooling

The client maintains a pool of connections to the server for optimal performance:

- Connections are reused across requests
- Automatic reconnection on failure
- Configurable pool size limits
- Pre-warming support for reduced latency

## Examples

Check the `examples/` directory for more usage examples:

- `basic.rs` - Basic usage and configuration
- `concurrent.rs` - Concurrent request handling
- `custom_pool.rs` - Advanced pool configuration

Run examples with:

```bash
cargo run --example basic
```

## Performance

The native protocol is designed for minimal overhead:
- Fixed-size request/response format
- No dynamic allocations for protocol encoding
- Efficient binary encoding
- TCP_NODELAY enabled by default

## Error Handling

All operations return `Result<T, ClientError>` with detailed error types:

```rust
use throttlecrab_client::ClientError;

match client.check_rate_limit("key", 10, 100, 60).await {
    Ok(response) => println!("Allowed: {}", response.allowed),
    Err(ClientError::Timeout) => println!("Request timed out"),
    Err(ClientError::ConnectionClosed) => println!("Connection lost"),
    Err(e) => println!("Error: {}", e),
}
```

## License

Licensed under MIT license ([LICENSE-MIT](../LICENSE-MIT) or http://opensource.org/licenses/MIT)
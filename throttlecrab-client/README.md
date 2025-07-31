# throttlecrab-client

[![Crates.io](https://img.shields.io/crates/v/throttlecrab-client.svg)](https://crates.io/crates/throttlecrab-client)
[![Documentation](https://docs.rs/throttlecrab-client/badge.svg)](https://docs.rs/throttlecrab-client)
[![License](https://img.shields.io/crates/l/throttlecrab-client.svg)](LICENSE-MIT)

High-performance async Rust client for [throttlecrab-server](https://crates.io/crates/throttlecrab-server). Provides connection pooling, automatic retries, and excellent performance for the native binary protocol.

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

## Best Practices

### Connection Management

1. **Share clients across your application**
   ```rust
   // Create once, clone for each component
   let client = Arc::new(ThrottleCrabClient::connect("127.0.0.1:9090").await?);
   ```

2. **Use connection pooling effectively**
   - The client automatically manages a connection pool
   - Connections are reused across requests
   - Failed connections are automatically replaced

## Examples

### Basic Usage
```rust
use throttlecrab_client::ThrottleCrabClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = ThrottleCrabClient::connect("127.0.0.1:9090").await?;

    // Simple rate limit check
    let response = client
        .check_rate_limit("user:123", 10, 100, 60)
        .await?;

    println!("Allowed: {}", response.allowed);
    println!("Remaining: {}", response.remaining);
    Ok(())
}
```

### With Custom Quantity
```rust
// Consume 5 tokens at once
let response = client
    .check_rate_limit_with_quantity("api:bulk", 100, 1000, 60, 5)
    .await?;
```

### Connection Pool Tuning
```rust
use throttlecrab_client::ClientBuilder;
use std::time::Duration;

let client = ClientBuilder::new()
    .max_connections(50)  // Increase for high concurrency
    .min_idle_connections(10)  // Keep connections warm
    .idle_timeout(Duration::from_secs(300))  // 5 minutes
    .build("127.0.0.1:9090")
    .await?;
```

### Pre-warming Connections
```rust
// Pre-establish connections for lower latency
let client = ClientBuilder::new()
    .min_idle_connections(20)
    .build("127.0.0.1:9090")
    .await?;

// Connections are established immediately
```

More examples in the `examples/` directory:
- `basic.rs` - Getting started
- `concurrent.rs` - High concurrency patterns
- `custom_pool.rs` - Advanced pool configuration
- `benchmark.rs` - Performance testing

## Performance

The native protocol achieves exceptional performance:

### Benchmark Results
- **Throughput**: 500K+ requests/second
- **Latency (P99)**: <1ms
- **Connection Pooling**: 20-50x improvement over single connection
- **Memory Usage**: ~100 bytes per pending request

### Protocol Efficiency
- Fixed-size messages (88-byte request, 40-byte response)
- Zero-copy deserialization
- No dynamic allocations
- TCP_NODELAY enabled by default
- Pipelined requests support

## Error Handling

Comprehensive error handling with automatic recovery:

```rust
use throttlecrab_client::{ClientError, ThrottleCrabClient};

#[tokio::main]
async fn main() {
    let client = ThrottleCrabClient::connect("127.0.0.1:9090")
        .await
        .expect("Failed to connect");

    match client.check_rate_limit("key", 10, 100, 60).await {
        Ok(response) => {
            if response.allowed {
                process_request();
            } else {
                // Retry after the suggested time
                tokio::time::sleep(Duration::from_secs(response.retry_after as u64)).await;
            }
        }
        Err(ClientError::Timeout) => {
            eprintln!("Request timed out, consider increasing timeout");
        }
        Err(ClientError::ConnectionClosed) => {
            eprintln!("Connection lost, client will auto-reconnect");
        }
        Err(ClientError::PoolExhausted) => {
            eprintln!("Connection pool exhausted, increase max_connections");
        }
        Err(e) => {
            eprintln!("Unexpected error: {}", e);
        }
    }
}
```

### Retry Logic

The client automatically handles:
- Connection failures (reconnects)
- Transient network errors
- Server restarts

For application-level retries:
```rust
use tokio_retry::{Retry, strategy::ExponentialBackoff};

let strategy = ExponentialBackoff::from_millis(100)
    .max_delay(Duration::from_secs(2))
    .take(3);

let result = Retry::spawn(strategy, || async {
    client.check_rate_limit("key", 10, 100, 60).await
}).await?;
```

## License

Licensed under MIT license ([LICENSE-MIT](../LICENSE-MIT) or http://opensource.org/licenses/MIT)

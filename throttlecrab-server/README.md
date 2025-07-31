# throttlecrab-server

[![CI](https://github.com/lazureykis/throttlecrab/actions/workflows/ci.yml/badge.svg)](https://github.com/lazureykis/throttlecrab/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/throttlecrab-server.svg)](https://crates.io/crates/throttlecrab-server)
[![Documentation](https://docs.rs/throttlecrab-server/badge.svg)](https://docs.rs/throttlecrab-server)
[![License](https://img.shields.io/crates/l/throttlecrab-server.svg)](LICENSE-MIT)

A high-performance rate limiting server with multiple protocol support, built on [throttlecrab](https://crates.io/crates/throttlecrab).

## Features

- **Multiple protocols**: Native binary, HTTP (JSON), and gRPC
- **High performance**: Lock-free shared state with Tokio async runtime
- **Production ready**: Health checks, configurable logging, systemd support
- **Flexible deployment**: Docker, binary, or source installation
- **Shared rate limiter**: All protocols share the same store for consistent limits

## Installation

Install the server binary with cargo:

```bash
cargo install throttlecrab-server
```

Or build from source:

```bash
git clone https://github.com/lazureykis/throttlecrab
cd throttlecrab/throttlecrab-server
cargo build --release
./target/release/throttlecrab-server
```

## Usage

Start the throttlecrab server (at least one transport must be specified):

```bash
# Run with HTTP transport
throttlecrab-server --http

# Run with HTTP transport on custom port
throttlecrab-server --http --http-port 7070

# Run multiple transports simultaneously
throttlecrab-server --http --grpc

# Specify different hosts and ports for each transport
throttlecrab-server --http --http-host 0.0.0.0 --http-port 8080 \
                    --grpc --grpc-port 50051

# Configure store type and parameters
throttlecrab-server --http --store adaptive \
                    --store-min-interval 5 \
                    --store-max-interval 300 \
                    --store-max-operations 1000000

# Use periodic store with custom cleanup interval
throttlecrab-server --http --store periodic --store-cleanup-interval 600

# Use probabilistic store
throttlecrab-server --grpc --store probabilistic --store-cleanup-probability 5000

# Set custom buffer size and log level
throttlecrab-server --http --buffer-size 50000 --log-level debug
```

### Environment Variables

All CLI arguments can be configured via environment variables with the `THROTTLECRAB_` prefix:

```bash
# Transport configuration
export THROTTLECRAB_HTTP=true
export THROTTLECRAB_HTTP_HOST=0.0.0.0
export THROTTLECRAB_HTTP_PORT=8080

# Store configuration
export THROTTLECRAB_STORE=adaptive
export THROTTLECRAB_STORE_CAPACITY=200000
export THROTTLECRAB_STORE_MIN_INTERVAL=10

# General configuration
export THROTTLECRAB_BUFFER_SIZE=100000
export THROTTLECRAB_LOG_LEVEL=info

# CLI arguments override environment variables
THROTTLECRAB_HTTP_PORT=8080 throttlecrab-server --http --http-port 7070
# Server will use port 7070 (CLI takes precedence)
```

## Transport Comparison

| Transport | Protocol | Throughput | Latency (P99) | Latency (P50) | Use Case |
|-----------|----------|------------|---------------|---------------|----------|
| Native | Binary | 183K req/s | 263 μs | 170 μs | Maximum performance |
| HTTP | JSON | 173K req/s | 309 μs | 177 μs | Easy integration |
| gRPC | Protobuf | 163K req/s | 370 μs | 186 μs | Service mesh |

## Protocol Documentation

### Native Protocol (Recommended)

Fixed-size binary protocol with minimal overhead:
- Request: 88 bytes (including up to 64-byte key)
- Response: 40 bytes
- No serialization overhead
- Direct memory layout

For best performance, implement the native protocol in your application or use established HTTP clients with connection pooling.


### HTTP REST API

**Endpoint**: `POST /throttle`

**Request Body** (JSON):
```json
{
  "key": "user:123",
  "max_burst": 10,
  "count_per_period": 100,
  "period": 60,
  "quantity": 1,
  "timestamp": 1234567890123456789
}
```

Note: `timestamp` is optional (Unix nanoseconds). If not provided, the server uses the current time.

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

### gRPC Protocol

See `proto/throttlecrab.proto` for the service definition. Use any gRPC client library to connect.

## Client Integration

### Rust
Use established HTTP clients like `reqwest` for reliable production deployments:
```toml
[dependencies]
reqwest = { version = "0.12", features = ["json"] }
```

### Other Languages
- **Go**: Use gRPC with generated client
- **Python**: Use HTTP/JSON API
- **Node.js**: Use HTTP/JSON API
- **Java**: Use gRPC with generated client

See `examples/` directory for implementation examples.

## Running Benchmarks

### Criterion Benchmarks

The project includes several Criterion benchmarks that require running servers. Use the provided script:

```bash
# Run all benchmarks
./run-criterion-benchmarks.sh

# Run specific benchmark
./run-criterion-benchmarks.sh tcp_throughput
./run-criterion-benchmarks.sh connection_pool
./run-criterion-benchmarks.sh protocol_comparison
./run-criterion-benchmarks.sh grpc_throughput
```

The script will:
1. Build the server in release mode
2. Start required servers (native on port 9092, gRPC on port 9093)
3. Run the benchmarks
4. Clean up servers on exit

Results are saved in `target/criterion/`.

## Production Deployment

### Performance Tuning

```bash
# Optimal settings for production
throttlecrab-server \
    --native --native-port 9090 \
    --store adaptive \
    --store-capacity 1000000 \
    --buffer-size 100000 \
    --log-level warn
```

### Monitoring

- **Health endpoint**: `GET /health` (available on HTTP port)
- **Logs**: Structured logging with configurable levels
- **Metrics**: Performance metrics in debug/trace logs

### Store Configuration

| Store Type | Use Case | Cleanup Strategy |
|------------|----------|------------------|
| `standard` | Small datasets | Every operation |
| `periodic` | Predictable load | Fixed intervals |
| `probabilistic` | High throughput | Random sampling |
| `adaptive` | Variable load | Self-tuning |

### Example Configurations

#### High-throughput API
```bash
throttlecrab-server --native \
    --store adaptive \
    --store-capacity 5000000 \
    --buffer-size 500000
```

#### Web Service
```bash
throttlecrab-server --http --http-port 8080 \
    --store periodic \
    --store-cleanup-interval 300
```

#### Microservices
```bash
throttlecrab-server --grpc --native \
    --grpc-port 50051 \
    --native-port 9090 \
    --store probabilistic
```

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](../LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
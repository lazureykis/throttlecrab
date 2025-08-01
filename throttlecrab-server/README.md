# throttlecrab-server

[![CI](https://github.com/lazureykis/throttlecrab/actions/workflows/ci.yml/badge.svg)](https://github.com/lazureykis/throttlecrab/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/throttlecrab-server.svg)](https://crates.io/crates/throttlecrab-server)
[![Documentation](https://docs.rs/throttlecrab-server/badge.svg)](https://docs.rs/throttlecrab-server)
[![License](https://img.shields.io/crates/l/throttlecrab-server.svg)](../LICENSE)

A high-performance rate limiting server with multiple protocol support, built on [throttlecrab](https://crates.io/crates/throttlecrab).

## Features

- **Multiple protocols**: HTTP (JSON) and gRPC
- **High performance**: Lock-free shared state with Tokio async runtime
- **Production ready**: Health checks, metrics endpoint, configurable logging, systemd support
- **Flexible deployment**: Docker, binary, or source installation
- **Shared rate limiter**: All protocols share the same store for consistent limits
- **Observability**: Prometheus-compatible metrics for monitoring and alerting

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
|-----------|----------|------------|---------------|---------------|-----------|
| HTTP | JSON | 173K req/s | 309 μs | 177 μs | Easy integration |
| gRPC | Protobuf | 163K req/s | 370 μs | 186 μs | Service mesh |

## Protocol Documentation

### HTTP REST API

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

Note: `quantity` is optional (defaults to 1).

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

See [`proto/throttlecrab.proto`](proto/throttlecrab.proto) for the service definition. Use any gRPC client library to connect.

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
2. Start required servers (HTTP on port 9092, gRPC on port 9093)
3. Run the benchmarks
4. Clean up servers on exit

Results are saved in `target/criterion/`.

## Production Deployment

### Performance Tuning

```bash
# Optimal settings for production
throttlecrab-server \
    --http --http-port 8080 \
    --store adaptive \
    --store-capacity 1000000 \
    --buffer-size 100000 \
    --log-level warn
```

### Monitoring

- **Health endpoint**: `GET /health` (available on HTTP port)
- **Metrics endpoint**: `GET /metrics` (Prometheus format, available on HTTP port)
- **Logs**: Structured logging with configurable levels
- **Performance metrics**: Available via `/metrics` endpoint

#### Available Metrics

- `throttlecrab_uptime_seconds`: Server uptime
- `throttlecrab_requests_total`: Total requests processed
- `throttlecrab_requests_by_transport{transport="..."}`: Requests per transport
- `throttlecrab_requests_allowed`: Total allowed requests
- `throttlecrab_requests_denied`: Total denied requests
- `throttlecrab_connections_active{transport="..."}`: Active connections per transport
- `throttlecrab_request_duration_bucket`: Request latency histogram
- `throttlecrab_active_keys`: Number of active rate limit keys
- `throttlecrab_store_evictions`: Total key evictions

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
throttlecrab-server --http \
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
throttlecrab-server --grpc --http \
    --grpc-port 50051 \
    --http-port 8080 \
    --store probabilistic
```

## License

Licensed under either of:

- MIT license ([LICENSE](../LICENSE) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

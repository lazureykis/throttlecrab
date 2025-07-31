# throttlecrab-server

[![CI](https://github.com/lazureykis/throttlecrab/actions/workflows/ci.yml/badge.svg)](https://github.com/lazureykis/throttlecrab/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/throttlecrab-server.svg)](https://crates.io/crates/throttlecrab-server)
[![Documentation](https://docs.rs/throttlecrab-server/badge.svg)](https://docs.rs/throttlecrab-server)
[![License](https://img.shields.io/crates/l/throttlecrab-server.svg)](LICENSE-MIT)

A high-performance rate limiting server with multiple protocol support, built on [throttlecrab](https://crates.io/crates/throttlecrab).

## Features

- **Multiple protocols**: MessagePack (TCP), HTTP (JSON), and gRPC
- **High performance**: Actor-based concurrency model using Tokio
- **Production ready**: Health checks, metrics, and configurable parameters
- **Easy integration**: Client libraries and examples for all protocols

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
# Run with MessagePack transport
throttlecrab-server --msgpack

# Run with HTTP transport on custom port
throttlecrab-server --http --http-port 7070

# Run multiple transports simultaneously
throttlecrab-server --http --grpc --msgpack

# Specify different hosts and ports for each transport
throttlecrab-server --http --http-host 0.0.0.0 --http-port 8080 \
                    --grpc --grpc-port 50051 \
                    --msgpack --msgpack-port 9090

# Configure store type and parameters
throttlecrab-server --msgpack --store adaptive \
                    --store-min-interval 5 \
                    --store-max-interval 300 \
                    --store-max-operations 1000000

# Use periodic store with custom cleanup interval
throttlecrab-server --http --store periodic --store-cleanup-interval 600

# Use probabilistic store
throttlecrab-server --grpc --store probabilistic --store-cleanup-probability 5000

# Set custom buffer size and log level
throttlecrab-server --msgpack --buffer-size 50000 --log-level debug
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

| Transport | Use Case | Performance | Complexity |
|-----------|----------|-------------|------------|
| MessagePack/TCP | Internal services, maximum performance | Highest | Low |
| HTTP/JSON | Web APIs, easy integration | Good | Lowest |
| gRPC | Service mesh, type-safe clients | Good | Medium |

## Protocol Documentation

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

## Client Examples

See the `examples/` directory for client implementations in Rust.

## Scaling

ThrottleCrab is designed for single-instance performance, but can be scaled horizontally using client-side sharding. See the main repository README for scaling strategies.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](../LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
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
./target/release/throttlecrab
```

## Usage

Start the throttlecrab server:

```bash
# Run with default settings (MessagePack on 127.0.0.1:9090)
throttlecrab --server

# Or with custom address
throttlecrab --server --host 0.0.0.0 --port 8080

# Use different transports:
throttlecrab --server --http      # HTTP with JSON (REST API)
throttlecrab --server --grpc      # gRPC transport
throttlecrab --server             # MessagePack over TCP (default, most efficient)
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
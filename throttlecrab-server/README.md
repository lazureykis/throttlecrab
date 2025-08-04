# throttlecrab-server

[![CI](https://github.com/lazureykis/throttlecrab/actions/workflows/ci.yml/badge.svg)](https://github.com/lazureykis/throttlecrab/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/throttlecrab-server.svg)](https://crates.io/crates/throttlecrab-server)
[![Docker](https://img.shields.io/docker/v/lazureykis/throttlecrab?label=docker)](https://hub.docker.com/r/lazureykis/throttlecrab)
[![Documentation](https://docs.rs/throttlecrab-server/badge.svg)](https://docs.rs/throttlecrab-server)
[![License](https://img.shields.io/crates/l/throttlecrab-server.svg)](../LICENSE)

A high-performance rate limiting server with multiple protocol support, built on [throttlecrab](https://crates.io/crates/throttlecrab).

## Features

- **Multiple protocols**: HTTP (JSON), gRPC, and Redis/RESP
- **High performance**: Lock-free shared state with Tokio async runtime
- **Production ready**: Health checks, metrics endpoint, configurable logging, systemd support
- **Flexible deployment**: Docker, binary, or source installation
- **Shared rate limiter**: All protocols share the same store for consistent limits
- **Observability**: Prometheus-compatible metrics for monitoring and alerting

## Installation

```bash
cargo install throttlecrab-server
```

## Quick Start

Start the server and make rate-limited requests:

```bash
# Start the server with HTTP transport
throttlecrab-server --http --http-port 8080

# In another terminal, make requests with curl
# First request - allowed
curl -X POST http://localhost:8080/throttle \
  -H "Content-Type: application/json" \
  -d '{"key": "api-key-123", "max_burst": 3, "count_per_period": 10, "period": 60}'

# Response:
# {"allowed":true,"limit":3,"remaining":2,"reset_after":60,"retry_after":0}

# Make more requests to see rate limiting in action
curl -X POST http://localhost:8080/throttle \
  -H "Content-Type: application/json" \
  -d '{"key": "api-key-123", "max_burst": 3, "count_per_period": 10, "period": 60}'

# Response when rate limited:
# {"allowed":false,"limit":3,"remaining":0,"reset_after":58,"retry_after":6}
```

### Environment Variables

All CLI arguments can be configured via environment variables with the `THROTTLECRAB_` prefix:

```bash
# Transport configuration
export THROTTLECRAB_HTTP=true
export THROTTLECRAB_HTTP_HOST=0.0.0.0
export THROTTLECRAB_HTTP_PORT=8080
export THROTTLECRAB_REDIS=true
export THROTTLECRAB_REDIS_HOST=0.0.0.0
export THROTTLECRAB_REDIS_PORT=6379

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

## Transport Performance Comparison

| Transport | Protocol | Throughput | Latency (P99) | Latency (P50) |
|-----------|----------|------------|---------------|---------------|
| HTTP | JSON | 175K req/s | 327 μs | 176 μs |
| gRPC | Protobuf | 163K req/s | 377 μs | 188 μs |
| Redis | RESP | 184K req/s | 275 μs | 170 μs |

You can run tests on your hardware with `cd integration-tests && ./run-transport-test.sh -t all -T 32 -r 10000`

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

### Redis Protocol

The server implements Redis Serialization Protocol (RESP), making it compatible with any Redis client.

**Port**: Default 6379 (configurable with `--redis-port`)

**Commands**:
- `THROTTLE key max_burst count_per_period period [quantity]` - Check rate limit
- `PING` - Health check
- `QUIT` - Close connection

**Example using redis-cli**:
```bash
redis-cli -p 6379
> THROTTLE user:123 10 100 60
1) (integer) 1    # allowed (1=yes, 0=no)
2) (integer) 10   # limit
3) (integer) 9    # remaining
4) (integer) 60   # reset_after (seconds)
5) (integer) 0    # retry_after (seconds)
```

**Example using Redis client libraries**:
```python
import redis

r = redis.Redis(host='localhost', port=6379)
result = r.execute_command('THROTTLE', 'user:123', 10, 100, 60)
# result: [1, 10, 9, 60, 0]
```

## Client Integration

Use any HTTP client, gRPC client library, or Redis client to connect to throttlecrab-server. See `examples/` directory for implementation examples.

## Monitoring

- **Health endpoint**: `GET /health` (available on HTTP port)
- **Metrics endpoint**: `GET /metrics` (Prometheus format, available on HTTP port)
- **Logs**: Structured logging with configurable levels
- **Performance metrics**: Available via `/metrics` endpoint

#### Available Metrics

##### Core Metrics
- `throttlecrab_uptime_seconds`: Server uptime in seconds
- `throttlecrab_requests_total`: Total requests processed across all transports
- `throttlecrab_requests_by_transport{transport="http|grpc|redis"}`: Requests per transport
- `throttlecrab_requests_allowed`: Total allowed requests
- `throttlecrab_requests_denied`: Total denied requests
- `throttlecrab_requests_errors`: Total internal errors
- `throttlecrab_top_denied_keys{key="...",rank="1-100"}`: Top denied keys by count

#### Example Prometheus Queries

```promql
# Monitor denial rate
rate(throttlecrab_requests_denied[5m]) / rate(throttlecrab_requests_total[5m])

# Alert on high error rate
rate(throttlecrab_requests_errors[5m]) > 0.01
```

### Store Types

| Store Type | Use Case | Cleanup Strategy |
|------------|----------|------------------|
| `periodic` | Predictable load | Fixed intervals |
| `probabilistic` | High throughput | Random sampling |
| `adaptive` | Variable load | Self-tuning |

## License

[MIT](../LICENSE)

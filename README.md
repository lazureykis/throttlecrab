# ThrottleCrab

[![CI](https://github.com/lazureykis/throttlecrab/actions/workflows/ci.yml/badge.svg)](https://github.com/lazureykis/throttlecrab/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/throttlecrab.svg)](https://crates.io/crates/throttlecrab)
[![Docker](https://img.shields.io/docker/v/lazureykis/throttlecrab?label=docker)](https://hub.docker.com/r/lazureykis/throttlecrab)
[![Documentation](https://docs.rs/throttlecrab/badge.svg)](https://docs.rs/throttlecrab)
[![License](https://img.shields.io/crates/l/throttlecrab.svg)](LICENSE)

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

# Run with HTTP for easy integration
throttlecrab-server --http --http-port 8080

# Or run with Redis protocol for maximum performance
throttlecrab-server --redis --redis-port 6379

# Or run with multiple protocols
throttlecrab-server --http --grpc --redis

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

#### Redis Protocol

For maximum performance, use the Redis protocol with any Redis client:

```python
import redis

# Connect to ThrottleCrab Redis interface
r = redis.Redis(host='localhost', port=6379)

# Check rate limit using THROTTLE command
# THROTTLE key max_burst count_per_period period [quantity]
result = r.execute_command('THROTTLE', 'user:123', 10, 100, 60, 1)

# Result: [allowed, limit, remaining, reset_after, retry_after]
# Example: [1, 10, 9, 60, 0]
allowed = result[0] == 1
remaining = result[2]
```

For production applications, use connection pooling with your chosen protocol.

## Features

### Core Library (`throttlecrab`)
- **GCRA Algorithm**: Smooth rate limiting without sudden spikes or drops
- **Multiple Store Types**:
  - `AdaptiveStore`: Self-tuning cleanup intervals
  - `PeriodicStore`: Fixed interval cleanup
  - `ProbabilisticStore`: Random sampling cleanup
- **Zero Dependencies**: Pure Rust implementation
- **Thread-Safe**: Can be used with `Arc<Mutex<>>` for concurrent access

### Server (`throttlecrab-server`)
- **Multiple Protocols**:
  - **Redis/RESP**: Redis-compatible protocol for highest performance
  - **HTTP/JSON**: REST API for easy integration
  - **gRPC**: Service mesh and microservices
- **Shared State**: All protocols share the same rate limiter store
- **Production Ready**: Health checks, Prometheus metrics, configurable logging
- **Advanced Observability**: Comprehensive metrics including denial rates, capacity usage, and key distribution insights
- **Flexible Deployment**: Docker, systemd, or standalone binary

## Performance

### Store Type Performance

| Store Type | Best For | Cleanup Strategy | Memory Usage |
|------------|----------|------------------|---------------|
| Adaptive | Variable workloads | Self-tuning intervals | Dynamic |
| Periodic | Predictable load | Fixed intervals | Predictable |
| Probabilistic | High throughput | Random sampling | Efficient |

## When to Use

- **Library**: For Rust applications with embedded rate limiting
- **Server**: For distributed systems and microservices needing centralized rate limiting



## Getting Started

### Installation

```toml
# For library usage
[dependencies]
throttlecrab = "0.2"
```

### Running the Server

```bash
# Install
cargo install throttlecrab-server

# Run with HTTP protocol
throttlecrab-server --http --http-port 8080 --store adaptive

# Run with multiple protocols
throttlecrab-server --http --grpc \
    --http-port 8080 \
    --grpc-port 50051

# Run with custom configuration
throttlecrab-server --http \
    --store adaptive \
    --store-capacity 1000000 \
    --log-level info
```

### Docker Deployment

#### Using Pre-built Image

Docker images are automatically built and pushed to Docker Hub via GitHub Actions for every release.

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
ExecStart=/usr/local/bin/throttlecrab-server --http --http-port 8080 --store adaptive
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

## Protocol Documentation

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

**Key Length**: No restriction

**Recommendation**: Use the shortest keys possible for better performance:
- Shorter keys = less memory usage
- Faster hashing and comparison
- More keys fit in CPU cache
- Example: prefer `u:123` over `user:123` or `uid_123` over `user_id_123`


### gRPC Protocol

See [`throttlecrab-server/proto/throttlecrab.proto`](throttlecrab-server/proto/throttlecrab.proto) for the service definition.

**Key Length**: No restriction

**Recommendation**: Same as HTTP - use short, efficient keys



## Key Design Best Practices

While ThrottleCrab doesn't enforce key length limits,
following these practices will maximize performance:

### Use Short Keys
- **Good**: `u:123`, `ip:1.2.3.4`, `a:abc`
- **Avoid**: `user_id:123`, `ip_address:1.2.3.4`, `api_key:abc`

### Be Consistent
- Pick a naming scheme and stick to it
- Document your key format for your team

### Memory Impact
Each key is stored in memory with ~80-150 bytes overhead:
- 10-char key: ~90 bytes total
- 50-char key: ~130 bytes total
- 100-char key: ~180 bytes total

With 1 million keys:
- 10-char keys: ~90 MB
- 50-char keys: ~130 MB
- 100-char keys: ~180 MB

### Performance Impact
Shorter keys provide:
- 2-3x faster hash computation
- Better CPU cache utilization
- Lower network bandwidth
- Faster key comparisons

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## Production Deployment

### Performance Tuning

```bash
# Example production configuration
throttlecrab-server \
    --http --http-port 8080 \
    --store adaptive \
    --store-capacity 1000000 \
    --buffer-size 100000 \
    --log-level warn
```

### Monitoring

- **Health Check**: `GET /health` returns 200 OK
- **Metrics**: Prometheus-compatible metrics via `GET /metrics`
- **Resource Usage**: Monitor memory usage based on active keys

## Metrics and Observability

ThrottleCrab Server provides comprehensive metrics for monitoring rate limiter performance and behavior. All metrics are exposed in Prometheus format at the `/metrics` endpoint (HTTP only).

### Available Metrics

#### System Metrics
- `throttlecrab_uptime_seconds`: Server uptime in seconds
- `throttlecrab_requests_total`: Total number of requests processed
- `throttlecrab_requests_by_transport{transport="http|grpc|redis"}`: Requests by transport type
- `throttlecrab_connections_active{transport="http|grpc|redis"}`: Active connections per transport

#### Rate Limiting Metrics
- `throttlecrab_requests_allowed`: Total allowed requests across all keys
- `throttlecrab_requests_denied`: Total denied requests across all keys
- `throttlecrab_active_keys`: Number of active rate limit keys in the store
- `throttlecrab_store_evictions`: Total key evictions from the store

#### Performance Metrics
- `throttlecrab_request_duration_bucket`: Request latency histogram (in microseconds)
  - Buckets: 50μs, 100μs, 250μs, 500μs, 1ms, 2.5ms, 5ms, 10ms, 25ms, 50ms, 100ms
- `throttlecrab_request_duration_sum`: Total time spent processing requests
- `throttlecrab_request_duration_count`: Total number of requests in the histogram

#### Advanced Metrics (v0.4.0+)
- `throttlecrab_denial_rate`: Current denial rate (0.0-1.0)
- `throttlecrab_avg_remaining_ratio`: Average remaining capacity ratio across all keys
- `throttlecrab_requests_near_limit`: Number of keys using >90% of their capacity
- `throttlecrab_total_capacity_used`: Sum of all tokens consumed
- `throttlecrab_total_capacity_available`: Sum of all token limits
- `throttlecrab_key_distribution_bucket`: Distribution of request counts per key
  - Buckets: 1, 10, 100, 1K, 10K, 100K, 1M requests

### Prometheus Integration

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'throttlecrab'
    static_configs:
      - targets: ['localhost:8080']
    metrics_path: '/metrics'
    scrape_interval: 15s
```

### Grafana Dashboard

Key metrics to monitor:
1. **Request Rate**: `rate(throttlecrab_requests_total[1m])`
2. **Denial Rate**: `throttlecrab_denial_rate`
3. **P99 Latency**: `histogram_quantile(0.99, throttlecrab_request_duration_bucket)`
4. **Active Keys**: `throttlecrab_active_keys`
5. **Keys Near Limit**: `throttlecrab_requests_near_limit`

### Alerting Examples

```yaml
# Prometheus alerting rules
groups:
  - name: throttlecrab
    rules:
      - alert: HighDenialRate
        expr: throttlecrab_denial_rate > 0.1
        for: 5m
        annotations:
          summary: "High rate limit denial rate ({{ $value }})"
      
      - alert: ManyKeysNearLimit
        expr: throttlecrab_requests_near_limit > 1000
        for: 5m
        annotations:
          summary: "{{ $value }} keys are near their rate limit"
```

## Time Handling

ThrottleCrab uses server-side timestamps for all rate limiting decisions.

### Scaling Strategies

#### Vertical Scaling
A single instance can handle:
- 180K+ requests/second on modern CPUs (Redis protocol)
- 170K+ requests/second with HTTP
- 160K+ requests/second with gRPC
- Millions of unique keys in memory
- Sub-millisecond P99 latency

#### Horizontal Scaling
For extreme scale, deploy multiple instances and use client-side sharding based on the rate limit key.


## License

Licensed under the MIT license ([LICENSE](LICENSE)).

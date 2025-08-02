# ThrottleCrab 🦀

[![CI](https://github.com/lazureykis/throttlecrab/actions/workflows/ci.yml/badge.svg)](https://github.com/lazureykis/throttlecrab/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/throttlecrab.svg)](https://crates.io/crates/throttlecrab)
[![Docker](https://img.shields.io/docker/v/lazureykis/throttlecrab?label=docker)](https://hub.docker.com/r/lazureykis/throttlecrab)
[![Documentation](https://docs.rs/throttlecrab/badge.svg)](https://docs.rs/throttlecrab)
[![License](https://img.shields.io/crates/l/throttlecrab.svg)](LICENSE)

High-performance rate limiting for any application. Choose between an embedded Rust library or a standalone server supporting HTTP, gRPC, and Redis protocols.

## Choose Your Path

### 🚀 Need rate limiting in 30 seconds?

```bash
# Install and run the server
cargo install throttlecrab-server
throttlecrab-server --http --http-port 8080

# Test it with curl
curl -X POST http://localhost:8080/throttle \
  -H "Content-Type: application/json" \
  -d '{"key": "test", "max_burst": 3, "count_per_period": 10, "period": 60}'
```

### 📦 Building a Rust application?

```toml
[dependencies]
throttlecrab = "0.4"
```

```rust
use throttlecrab::{RateLimiter, AdaptiveStore};
use std::time::SystemTime;

let mut limiter = RateLimiter::new(AdaptiveStore::new());
let (allowed, result) = limiter
    .rate_limit("user:123", 10, 100, 60, 1, SystemTime::now())
    .unwrap();

if allowed {
    println!("✅ Request allowed! {} remaining", result.remaining);
} else {
    println!("❌ Rate limited! Retry in {}s", result.retry_after);
}
```

### 🐳 Want to use Docker?

```bash
docker run -d -p 8080:8080 lazureykis/throttlecrab:latest
```

## What is ThrottleCrab?

ThrottleCrab implements the **Generic Cell Rate Algorithm (GCRA)** for smooth, precise rate limiting without sudden bursts or unfair rejections. It's available as:

- **`throttlecrab`** - Embedded Rust library for in-process rate limiting
- **`throttlecrab-server`** - Standalone server supporting HTTP, gRPC, and Redis protocols

## Quick Examples

### HTTP API (Any Language)

```python
import requests

# Check rate limit
response = requests.post('http://localhost:8080/throttle', json={
    'key': 'user:123',
    'max_burst': 10,
    'count_per_period': 100,
    'period': 60
})

result = response.json()
if result['allowed']:
    print(f"✅ Request allowed! {result['remaining']} remaining")
else:
    print(f"❌ Rate limited! Retry in {result['retry_after']}s")
```

### Redis Protocol (Maximum Performance)

```python
import redis

r = redis.Redis(host='localhost', port=6379)
result = r.execute_command('THROTTLE', 'user:123', 10, 100, 60)
# Returns: [allowed (0/1), limit, remaining, reset_after, retry_after]
```

### JavaScript/Node.js

```javascript
const response = await fetch('http://localhost:8080/throttle', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
        key: 'user:123',
        max_burst: 10,
        count_per_period: 100,
        period: 60
    })
});

const result = await response.json();
console.log(result.allowed ? '✅ Allowed' : '❌ Rate limited');
```

## Performance

| Protocol | Throughput | P99 Latency | P50 Latency |
|----------|------------|-------------|-------------|
| Redis    | 184K req/s | 275 μs      | 170 μs      |
| HTTP     | 175K req/s | 327 μs      | 176 μs      |
| gRPC     | 163K req/s | 377 μs      | 188 μs      |

You can run the same benchmark yourself with `cd integration-tests && ./run-transport-test.sh -t all -T 32 -r 10000`

## Server Installation Options

### Binary
```bash
cargo install throttlecrab-server
throttlecrab-server --http --http-port 8080
```

### Docker
```bash
docker run -d -p 8080:8080 -p 6379:6379 \
  -e THROTTLECRAB_HTTP=true \
  -e THROTTLECRAB_REDIS=true \
  lazureykis/throttlecrab:latest
```

### Docker Compose
```yaml
version: '3.8'
services:
  throttlecrab:
    image: lazureykis/throttlecrab:latest
    ports:
      - "8080:8080"   # HTTP
      - "6379:6379"   # Redis
      - "50051:50051" # gRPC
    environment:
      THROTTLECRAB_LOG_LEVEL: info
    restart: unless-stopped
```

### Systemd
```ini
[Unit]
Description=ThrottleCrab Rate Limiting Server
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/throttlecrab-server --http --redis
Restart=always

[Install]
WantedBy=multi-user.target
```

## Configuration

All options can be set via CLI flags or environment variables:

```bash
# CLI flags
throttlecrab-server --http --http-port 8080 --store adaptive

# Environment variables
export THROTTLECRAB_HTTP=true
export THROTTLECRAB_HTTP_PORT=8080
export THROTTLECRAB_STORE=adaptive
throttlecrab-server
```

### Store Types

| Store | Best For | Cleanup Strategy |
|-------|----------|------------------|
| `adaptive` | Variable load (default) | Self-tuning |
| `periodic` | Predictable load | Fixed intervals |
| `probabilistic` | High throughput | Random sampling |

## Monitoring

- **Health**: `GET /health`
- **Metrics**: `GET /metrics` (Prometheus format)

Key metrics:
- `throttlecrab_requests_total` - Total requests
- `throttlecrab_denial_rate` - Current denial rate (0.0-1.0)
- `throttlecrab_request_duration_bucket` - Latency histogram

## Protocol Reference

### HTTP API
```bash
POST /throttle
{
  "key": "string",           # Unique identifier
  "max_burst": 10,          # Maximum burst capacity
  "count_per_period": 100,  # Allowed requests per period
  "period": 60,             # Period in seconds
  "quantity": 1             # Optional, default 1
}
```

### Redis Commands
```
THROTTLE key max_burst count_per_period period [quantity]
PING
QUIT
```

### gRPC
See [`throttlecrab-server/proto/throttlecrab.proto`](throttlecrab-server/proto/throttlecrab.proto)

## Advanced Topics

### Key Design
For best performance, use short keys:
- ✅ Good: `u:123`, `ip:1.2.3.4`
- ❌ Avoid: `user_id:123`, `ip_address:1.2.3.4`

### Memory Usage
Each entry stores:
- Key string: varies by length
- Value: i64 (8 bytes) + Option<SystemTime> (24 bytes) = 32 bytes
- HashMap overhead: ~32 bytes per entry

Total per entry: ~64 bytes + key length

With 1M keys:
- 10-char keys: ~74 MB
- 50-char keys: ~114 MB
- 100-char keys: ~164 MB

### Scaling
- **Vertical**: Single instance handles 180K+ req/s
- **Horizontal**: Use client-side sharding by key

## Contributing

Contributions welcome! Please feel free to submit a Pull Request.

## License

[MIT](LICENSE)

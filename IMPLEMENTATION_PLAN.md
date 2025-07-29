# ThrottleCrab Implementation Plan

## Overview
A standalone GCRA rate limiter service inspired by redis-cell, with multiple protocol support.

## Core Algorithm (GCRA)
Based on redis-cell's implementation:
- **No background processes** - all calculations done on-demand
- **Rolling time window** - smooth rate limiting without sudden resets
- **Configurable burst capacity** - handle traffic spikes gracefully

### Key Parameters
1. **Key**: Identifier for the rate limit (e.g., user ID, API key)
2. **Max Burst**: Maximum tokens available at once
3. **Rate**: Tokens replenished per time period
4. **Period**: Time window in seconds
5. **Quantity**: Tokens requested (default: 1)

### Response Data
1. **Allowed**: 0 (allowed) or 1 (blocked)
2. **Limit**: Total rate limit
3. **Remaining**: Tokens remaining
4. **Retry After**: Seconds until retry (if blocked)
5. **Reset After**: Seconds until full reset

## Architecture

### 1. Core Library (`src/lib.rs`)
```rust
// Core GCRA implementation
pub struct RateLimiter {
    // In-memory storage of rate limit states
    store: Arc<RwLock<HashMap<String, LimiterState>>>
}

pub struct LimiterState {
    tat: f64,  // Theoretical Arrival Time
    tau: f64,  // Emission interval
    burst: u32,
}
```

### 2. Storage Backends
- **In-Memory**: Default, using Arc<RwLock<HashMap>>
- **Future**: Redis, PostgreSQL, SQLite

### 3. Protocol Handlers

#### Option A: Redis Protocol Compatible
- **Pros**: Drop-in replacement for redis-cell
- **Command**: `CL.THROTTLE key burst rate period [quantity]`
- **Port**: 6379 (configurable)
- **Use Case**: Existing Redis clients work immediately

#### Option B: HTTP/REST API
- **Pros**: Universal, easy debugging, good for microservices
- **Endpoints**:
  ```
  POST /throttle
  {
    "key": "user123",
    "burst": 15,
    "rate": 30,
    "period": 60,
    "quantity": 1
  }
  ```
- **Port**: 8080 (configurable)
- **Features**: JSON response, health checks, metrics endpoint

#### Option C: gRPC
- **Pros**: High performance, type-safe, streaming support
- **Proto**:
  ```proto
  service RateLimiter {
    rpc Throttle(ThrottleRequest) returns (ThrottleResponse);
    rpc StreamThrottle(stream ThrottleRequest) returns (stream ThrottleResponse);
  }
  ```
- **Port**: 50051 (configurable)
- **Use Case**: Microservices, high-throughput systems

#### Option D: TCP + MessagePack
- **Pros**: Minimal overhead, binary efficient
- **Format**: MessagePack-encoded requests/responses
- **Port**: 9090 (configurable)
- **Use Case**: Performance-critical applications

#### Option E: Plain Text (Telnet-style)
- **Pros**: Human-readable, simple debugging
- **Format**: `THROTTLE key burst rate period quantity\r\n`
- **Response**: Space-separated values
- **Port**: 2323 (configurable)

## Implementation Phases

### Phase 1: Core Library
1. Implement GCRA algorithm (port from redis-cell)
2. In-memory storage with thread-safe access
3. Unit tests for algorithm correctness

### Phase 2: CLI Binary
1. Basic CLI for testing: `throttlecrab check --key user123 --burst 15 --rate 30 --period 60`
2. Configuration file support

### Phase 3: Protocol Support (Priority Order)
1. **Redis Protocol** - Easiest migration path
2. **HTTP/REST** - Most universal
3. **gRPC** - For performance needs
4. **TCP+MessagePack** - For extreme performance
5. **Plain Text** - For debugging/simplicity

### Phase 4: Production Features
1. Metrics (Prometheus format)
2. Distributed storage support
3. Clustering/HA
4. Performance optimizations

## Configuration
```toml
[server]
# Enable/disable protocols
redis_enabled = true
redis_port = 6379

http_enabled = true
http_port = 8080

grpc_enabled = false
grpc_port = 50051

tcp_msgpack_enabled = false
tcp_msgpack_port = 9090

plaintext_enabled = false
plaintext_port = 2323

[storage]
type = "memory"  # memory, redis, postgres
# connection settings...

[limits]
# Global defaults
default_burst = 10
default_rate = 60
default_period = 60
```

## Benchmarking Plan
- Compare performance across protocols
- Measure memory usage patterns
- Test concurrent access patterns
- Compare with redis-cell performance

## Decision Points
1. **Start with Redis protocol?** - Yes, for easy adoption
2. **Which protocol second?** - HTTP for broad compatibility
3. **Async runtime?** - Tokio (most mature)
4. **Serialization for HTTP?** - JSON (serde_json)
5. **Metrics format?** - Prometheus (industry standard)
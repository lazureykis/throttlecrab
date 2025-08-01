# ThrottleCrab Benchmark Results

Last Updated: 2025-08-01

## Executive Summary

ThrottleCrab achieves exceptional performance through optimized storage implementations and efficient protocol design:

- **Library Performance**: Up to 12.5M requests/second (21.6x faster than baseline)
- **Server Performance**: 185K requests/second with Redis protocol
- **Latency**: Sub-millisecond P99 latency across all protocols (249-381 μs)
- **Memory Efficiency**: ~100 bytes per active rate limit key

## Hardware Configuration

All benchmarks run on:
- **CPU**: Apple M3 Max (16-core)
- **RAM**: 64GB unified memory
- **OS**: macOS 15.5
- **Rust**: 1.88.0

## Library Benchmarks

### Store Performance Comparison

Testing 400K operations across 2K unique keys:

| Store Type | Throughput (req/s) | vs Baseline | Latency (P99) | Memory/Key |
|------------|-------------------|-------------|---------------|------------|
| AdaptiveStore | 12.5M | 21.6x | 75 ns | ~100 bytes |
| PeriodicStore | 11.4M | 19.7x | 85 ns | Predictable |
| ProbabilisticStore | 10.0M | 17.2x | 100 ns | Efficient |

### Access Pattern Performance

#### Sequential Access (API keys in order)
Best performer: **AdaptiveStore** (9.98M req/s)
- Adaptive cleanup intervals optimize for patterns
- 17.3x improvement over baseline

#### Random Access (Distributed keys)
Best performer: **AdaptiveStore** (8.46M req/s)
- Self-tuning excels with unpredictable access
- 16.0x improvement over baseline

#### Hot Keys (80/20 distribution)
Best performer: **AdaptiveStore** (12.0M req/s)
- Cleanup strategy adapts to hot key patterns
- 2.0x improvement over baseline

#### Sparse Keys (90% non-existent)
Best performer: **AdaptiveStore** (9.28M req/s)
- Adapts cleanup to high miss rate
- 15.2x improvement over baseline

## Server Benchmarks

### Transport Protocol Comparison

Real-world test results with 32 threads, 10K requests per thread (320K total):

| Protocol | Throughput | Latency P50 | Latency P90 | Latency P99 | Latency P99.9 |
|----------|------------|-------------|-------------|-------------|---------------|
| Redis/RESP | 185,582 req/s | 168 μs | 205 μs | 249 μs | 775 μs |
| HTTP/JSON | 173,544 req/s | 178 μs | 227 μs | 302 μs | 582 μs |
| gRPC | 161,591 req/s | 190 μs | 269 μs | 381 μs | 570 μs |

### Performance Insights

1. **Throughput**: All protocols achieve excellent throughput (161K-185K req/s)
2. **Latency**: Sub-millisecond P99 latency across all protocols (249-381 μs)
3. **Redis Performance**: Highest throughput with minimal protocol overhead
4. **Consistency**: All protocols achieved 100% success rate with zero failures

### Test Configuration

#### Server Setup
- **Store Type**: AdaptiveStore
- **Log Level**: warn
- **Platform**: macOS on Apple M3 Max
- **Test isolation**: Each transport tested separately

#### Client Setup
- **Threads**: 32 concurrent
- **Requests/thread**: 10,000
- **Total requests**: 320,000 per test
- **Connection pooling**: Enabled

## Protocol Efficiency

### Redis/RESP
- **Highest throughput**: 185K req/s with binary protocol
- **Lowest latency**: 168 μs P50, 249 μs P99
- **Redis compatibility**: Works with any Redis client library
- **Minimal overhead**: Binary protocol without HTTP headers

### HTTP/JSON
- **Standard tooling**: Works with curl, Postman, etc.
- **Connection pooling**: Reuse connections for efficiency
- **Universal compatibility**: Any language or platform
- **Excellent performance**: 173K req/s

### gRPC
- **Type safety**: Generated client/server code
- **Streaming support**: Built-in bidirectional streaming
- **Service mesh ready**: Works well in microservices
- **Good performance**: 161K req/s

## Recommendations

### Store Selection Guide

1. **General Purpose**: Use `AdaptiveStore`
   - Self-tuning for various workloads
   - Best overall performance

2. **Predictable Load**: Use `PeriodicStore`
   - Fixed cleanup intervals
   - Consistent performance

3. **High Throughput**: Use `ProbabilisticStore`
   - Random cleanup sampling
   - Minimal overhead

### Protocol Selection Guide

1. **Maximum Performance**: Redis/RESP
   - 185K requests/second
   - Binary protocol with minimal overhead
   - Compatible with standard Redis clients

2. **Easy Integration**: HTTP/JSON
   - Standard REST API
   - 173K requests/second
   - Works with any HTTP client

3. **Service Mesh**: gRPC
   - Type-safe clients
   - 161K requests/second
   - Built-in streaming support

### Scaling Recommendations

- **Single Instance**: Handles 160-185K req/s comfortably
- **Horizontal Scaling**: Use client-side sharding for higher throughput
- **Connection Pooling**: 32 connections work well for high concurrency
- **Store Capacity**: Plan for 100 bytes per active key

## Testing Methodology

All benchmarks use:
- Test script: `./run-transport-test.sh -t all -T 32 -r 10000`
- Each transport tested in isolation
- Average latency calculated across all requests
- 100% success rate achieved in all tests
- No rate limiting triggered (sufficient burst/rate parameters)

## Running Benchmarks

Reproduce these results:

```bash
# Library benchmarks
cd throttlecrab
cargo bench

# Server benchmarks
cd integration-tests
./run-transport-test.sh -t all -T 32 -r 10000
```

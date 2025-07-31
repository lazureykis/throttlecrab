# ThrottleCrab Benchmark Results

Last Updated: 2024-12-19

## Executive Summary

ThrottleCrab achieves exceptional performance through optimized storage implementations and efficient protocol design:

- **Library Performance**: Up to 12.5M requests/second (21.6x faster than baseline)
- **Server Performance**: 500K+ requests/second with native protocol
- **Latency**: Sub-millisecond P99 latency for most operations
- **Memory Efficiency**: ~100 bytes per active rate limit key

## Hardware Configuration

All benchmarks run on:
- **CPU**: Apple M2 (10-core)
- **RAM**: 16GB unified memory
- **OS**: macOS 14.x
- **Rust**: 1.75.0

## Library Benchmarks

### Store Performance Comparison

Testing 400K operations across 2K unique keys:

| Store Type | Throughput (req/s) | vs Standard | Latency (P99) | Memory/Key |
|------------|-------------------|-------------|---------------|------------|
| AdaptiveMemoryStore | 12.5M | 21.6x | 75 ns | ~100 bytes |
| OptimizedMemoryStore | 11.4M | 19.7x | 85 ns | Pre-allocated |
| StandardMemoryStore | 580K | 1.0x | 1.7 μs | Minimal |

### Access Pattern Performance

#### Sequential Access (API keys in order)
Best performer: **InternedMemoryStore** (9.98M req/s)
- Key interning benefits from repeated patterns
- 17.3x improvement over standard store

#### Random Access (Distributed keys)
Best performer: **OptimizedMemoryStore** (8.46M req/s)
- HashMap optimizations excel with random access
- 16.0x improvement over standard store

#### Hot Keys (80/20 distribution)
Best performer: **AmortizedMemoryStore** (12.0M req/s)
- Cleanup strategy focuses on active keys
- 2.0x improvement even over baseline

#### Sparse Keys (90% non-existent)
Best performer: **AdaptiveMemoryStore** (9.28M req/s)
- Adapts cleanup to high miss rate
- 15.2x improvement over standard store

## Server Benchmarks

### Transport Protocol Comparison

Testing with 100 concurrent connections, 1M requests total:

| Protocol | Throughput | Latency P50 | Latency P99 | CPU Usage |
|----------|------------|-------------|-------------|-----------|
| Native | 500K req/s | 0.2 ms | 0.9 ms | 85% |
| MessagePack | 300K req/s | 0.3 ms | 1.5 ms | 75% |
| gRPC | 150K req/s | 0.6 ms | 2.8 ms | 90% |
| HTTP/JSON | 100K req/s | 0.9 ms | 4.5 ms | 95% |

### Concurrent Load Test

Testing all protocols simultaneously sharing the same store:

- **Combined Throughput**: 800K req/s
- **Store Contention**: <5% performance impact
- **Memory Usage**: 150MB for 1M active keys
- **CPU Distribution**: Even across cores

### Stress Test Results

Maximum sustainable load before degradation:

| Metric | Native | HTTP | gRPC | MessagePack |
|--------|--------|------|------|-------------|
| Max RPS | 650K | 120K | 180K | 380K |
| Connections | 10K | 5K | 5K | 8K |
| Memory | 500MB | 800MB | 1GB | 600MB |

## Client Library Benchmarks

### Connection Pool Performance

| Pool Size | Throughput | vs Single | Latency P99 |
|-----------|------------|-----------|-------------|
| 1 | 25K req/s | 1.0x | 40 ms |
| 10 | 250K req/s | 10x | 4 ms |
| 50 | 450K req/s | 18x | 2 ms |
| 100 | 500K req/s | 20x | 2 ms |

### Protocol Overhead

Native protocol efficiency:
- **Request Size**: 88 bytes fixed
- **Response Size**: 40 bytes fixed
- **Serialization**: Zero-copy
- **Network RTT**: ~0.1ms localhost

## Comparison with Redis-Cell

Testing equivalent GCRA parameters:

| Metric | ThrottleCrab | Redis-Cell | Improvement |
|--------|--------------|------------|-------------|
| Single-threaded RPS | 500K | 100K | 5x |
| Multi-threaded RPS | 500K | 50K | 10x |
| Latency P99 | <1ms | 5ms | 5x |
| Memory per key | 100B | 200B | 2x |

## Recommendations

### Store Selection Guide

1. **General Purpose**: Use `AdaptiveMemoryStore`
   - Self-tuning for various workloads
   - Best overall performance

2. **High Throughput**: Use `OptimizedMemoryStore`
   - Pre-allocated memory
   - Predictable performance

3. **Memory Constrained**: Use `StandardMemoryStore`
   - Minimal memory usage
   - Acceptable performance

### Protocol Selection Guide

1. **Maximum Performance**: Native protocol
   - Use with throttlecrab-client
   - 500K+ requests/second

2. **Easy Integration**: HTTP/JSON
   - Standard REST API
   - 100K requests/second

3. **Service Mesh**: gRPC
   - Type-safe clients
   - 150K requests/second

### Scaling Recommendations

- **Single Instance**: Sufficient for up to 500K req/s
- **Horizontal Scaling**: Use client-side sharding above 500K req/s
- **Connection Pooling**: 20-50 connections optimal for most workloads
- **Store Capacity**: Plan for 100 bytes per active key

## Testing Methodology

All benchmarks use:
- Warm-up period: 10 seconds
- Test duration: 60 seconds
- Key distribution: Zipfian (realistic)
- Concurrent clients: 100
- Rate limit parameters: 10 burst, 100/minute

Run benchmarks yourself:
```bash
# Library benchmarks
cd throttlecrab/benches
./run_benchmarks.sh

# Server benchmarks
cd throttlecrab-server/tests
./run-benchmarks.sh all
```
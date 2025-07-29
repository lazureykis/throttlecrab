# ThrottleCrab Benchmarks

This directory contains benchmarks for measuring the performance of the ThrottleCrab rate limiter under various conditions.

## Running the Benchmarks

1. First, start the ThrottleCrab server:
   ```bash
   cargo run --features bin -- --server
   ```

2. In another terminal, run the benchmarks:
   ```bash
   cargo bench
   ```

## Benchmark Scenarios

### Single Thread Performance
- **sequential_requests**: Measures throughput of sequential requests from a single client

### Multi-Thread Performance
- Tests with 2, 4, 8, and 16 concurrent threads
- Each thread maintains its own TCP connection
- Measures total throughput across all threads

### Burst Pattern
- **burst_then_wait**: Simulates burst traffic patterns (10 requests, then pause)
- Tests how the rate limiter handles sudden traffic spikes

### Mixed Keys
- **rotating_keys**: Tests performance with a fixed set of keys
- Simulates multiple users/APIs sharing the rate limiter

## Key Metrics

- **Throughput**: Requests per second
- **Latency**: Time per request (including network round-trip)
- **Scalability**: Performance with multiple concurrent clients

## Performance Considerations

1. **Network Overhead**: These benchmarks include TCP round-trip time
2. **MessagePack Serialization**: Includes encoding/decoding overhead
3. **Actor Model**: All requests are processed sequentially by the actor

## Optimizing Performance

To improve performance:
1. Use connection pooling in production
2. Consider batching requests
3. Use Unix domain sockets for local communication
4. Implement pipelining for multiple requests
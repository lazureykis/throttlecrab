# ThrottleCrab Benchmarks

This directory contains benchmarks for measuring the performance of the ThrottleCrab rate limiter under various conditions.

## Running the Benchmarks

### TCP/MessagePack Benchmarks

1. First, start the ThrottleCrab server:
   ```bash
   cargo run --features bin -- --server
   ```

2. In another terminal, run the benchmarks:
   ```bash
   cargo bench
   ```

### Protocol Comparison Benchmarks

To compare different transport protocols, start multiple servers:
```bash
# Terminal 1: Standard MessagePack
cargo run --features bin -- --server --port 9090

# Terminal 2: Optimized MessagePack
cargo run --features bin -- --server --port 9091 --optimized

# Terminal 3: Compact binary protocol
cargo run --features bin -- --server --port 9092 --compact

# Terminal 4: gRPC (requires protoc installed)
cargo run --features bin -- --server --port 9093 --grpc
```

Then run the protocol comparison:
```bash
cargo bench protocol_comparison
```

### gRPC Throughput Benchmarks

1. Start the gRPC server:
   ```bash
   cargo run --features bin -- --server --port 9093 --grpc
   ```

2. Run the gRPC benchmarks:
   ```bash
   cargo bench grpc_throughput
   ```

## Benchmark Scenarios

### TCP Throughput Benchmarks
- **sequential_requests**: Measures throughput of sequential requests from a single client
- **concurrent_threads**: Tests with 2, 4, 8, and 16 concurrent threads
- **burst_then_wait**: Simulates burst traffic patterns (10 requests, then pause)
- **rotating_keys**: Tests performance with a fixed set of keys

### Protocol Comparison Benchmarks
- **single_request**: Measures latency for individual requests across protocols
- **batch_100**: Measures throughput for batches of 100 requests
- Compares: Standard MessagePack, Optimized MessagePack, Compact Binary, and gRPC

### gRPC Throughput Benchmarks
- **sequential**: Single client making sequential requests
- **concurrent_clients**: Tests with 1, 10, 50, and 100 concurrent gRPC clients
- **batch_requests**: Batch sizes of 10, 100, and 1000 on a single connection

## Key Metrics

- **Throughput**: Requests per second
- **Latency**: Time per request (including network round-trip)
- **Scalability**: Performance with multiple concurrent clients

## Performance Considerations

1. **Network Overhead**: These benchmarks include TCP/HTTP round-trip time
2. **Serialization**: 
   - MessagePack: Binary encoding/decoding overhead
   - gRPC: Protocol Buffers serialization overhead
3. **Actor Model**: All requests are processed sequentially by the actor
4. **Protocol Differences**:
   - MessagePack: Lower overhead, custom framing
   - Compact Binary: Minimal overhead, fixed-size messages
   - gRPC: HTTP/2 overhead, but better tooling and streaming support

## Optimizing Performance

To improve performance:
1. Use connection pooling in production
2. Consider batching requests
3. Use Unix domain sockets for local communication
4. Implement pipelining for multiple requests
5. For gRPC: Use streaming for bulk operations
6. For MessagePack: Use the optimized or compact protocol variants
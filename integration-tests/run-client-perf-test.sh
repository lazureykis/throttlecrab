#!/bin/bash

# Performance test for native client with connection pooling

set -e

echo "Building projects..."
cd .. && cargo build --release -p throttlecrab-server
cd integration-tests && cargo build --release

# Kill any existing server
lsof -ti:58072 | xargs kill -9 2>/dev/null || true

# Start server in background
echo -e "\nStarting ThrottleCrab server with native transport..."
../target/release/throttlecrab-server \
    --native --native-port 58072 \
    --store adaptive \
    --store-capacity 1000000 \
    --buffer-size 1000000 \
    --log-level warn &

SERVER_PID=$!
echo "Server started with PID: $SERVER_PID"

# Wait for server to start
sleep 2

# Check if server is running
if ! kill -0 $SERVER_PID 2>/dev/null; then
    echo "ERROR: Server failed to start"
    exit 1
fi

# Run client performance test with different configurations
echo -e "\n=== Running Client Performance Tests ==="

# Test 1: Default configuration
echo -e "\n1. Default configuration (20 threads, 5000 requests each, pool size 10)"
../target/release/throttlecrab-integration-tests client-perf-test \
    --threads 20 \
    --requests 5000 \
    --port 58072 \
    --pool-size 10

# Test 2: High concurrency
echo -e "\n2. High concurrency (50 threads, 2000 requests each, pool size 20)"
../target/release/throttlecrab-integration-tests client-perf-test \
    --threads 50 \
    --requests 2000 \
    --port 58072 \
    --pool-size 20

# Test 3: Small pool, high load
echo -e "\n3. Small pool with high load (40 threads, 2500 requests each, pool size 5)"
../target/release/throttlecrab-integration-tests client-perf-test \
    --threads 40 \
    --requests 2500 \
    --port 58072 \
    --pool-size 5

# Test 4: Large pool
echo -e "\n4. Large pool (30 threads, 3000 requests each, pool size 50)"
../target/release/throttlecrab-integration-tests client-perf-test \
    --threads 30 \
    --requests 3000 \
    --port 58072 \
    --pool-size 50

# Stop server
echo -e "\nStopping server..."
kill $SERVER_PID
wait $SERVER_PID 2>/dev/null || true

echo -e "\nAll tests completed!"
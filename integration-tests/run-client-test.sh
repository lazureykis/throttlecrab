#!/bin/bash

# Simple throttlecrab-client integration test

set -e

# Default values
THREADS=${1:-10}
REQUESTS=${2:-1000}
PORT=${3:-9090}
POOL_SIZE=${4:-10}

echo "=== ThrottleCrab Client Integration Test ==="
echo "Threads: $THREADS"
echo "Requests per thread: $REQUESTS"
echo "Total requests: $((THREADS * REQUESTS))"
echo "Port: $PORT"
echo "Pool size: $POOL_SIZE"
echo ""

# Build if needed
if [ ! -f "../target/release/throttlecrab-server" ] || [ ! -f "../target/release/throttlecrab-integration-tests" ]; then
    echo "Building projects..."
    cd .. && cargo build --release -p throttlecrab-server
    cd integration-tests && cargo build --release
fi

# Kill any existing server on the port
lsof -ti:$PORT | xargs kill -9 2>/dev/null || true

# Start server with native protocol
echo "Starting server with native protocol..."
../target/release/throttlecrab-server \
    --native --native-port $PORT \
    --store adaptive \
    --log-level warn &

SERVER_PID=$!
echo "Server started with PID: $SERVER_PID"

# Wait for server
sleep 2

# Run client test
echo -e "\nRunning client performance test..."
../target/release/throttlecrab-integration-tests client-perf-test \
    --threads $THREADS \
    --requests $REQUESTS \
    --port $PORT \
    --pool-size $POOL_SIZE

# Cleanup
echo -e "\nStopping server..."
kill $SERVER_PID
wait $SERVER_PID 2>/dev/null || true

echo "Test completed!"
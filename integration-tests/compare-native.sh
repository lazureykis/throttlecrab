#!/bin/bash

# Compare native protocol performance: direct vs client library

set -e

PORT=${1:-9090}
THREADS=${2:-32}
REQUESTS=${3:-10000}

echo "=== Native Protocol Performance Comparison ==="
echo "Port: $PORT"
echo "Threads: $THREADS"
echo "Requests per thread: $REQUESTS"
echo "Total requests: $((THREADS * REQUESTS))"
echo ""

# Build if needed
if [ ! -f "../target/release/throttlecrab-server" ] || [ ! -f "../target/release/throttlecrab-integration-tests" ]; then
    echo "Building projects..."
    cd .. && cargo build --release -p throttlecrab-server
    cd integration-tests && cargo build --release
fi

# Kill any existing server
lsof -ti:$PORT | xargs kill -9 2>/dev/null || true

# Start server
echo "Starting server..."
../target/release/throttlecrab-server \
    --native --native-port $PORT \
    --store adaptive \
    --log-level warn &

SERVER_PID=$!
sleep 2

echo -e "\n1. Testing direct native protocol (no connection pool)..."
../target/release/throttlecrab-integration-tests direct-native-test \
    --threads $THREADS \
    --requests $REQUESTS \
    --port $PORT

echo -e "\n2. Testing client library (optimized pool)..."
../target/release/throttlecrab-integration-tests client-v2-perf-test \
    --threads $THREADS \
    --requests $REQUESTS \
    --port $PORT

# Cleanup
kill $SERVER_PID
wait $SERVER_PID 2>/dev/null || true

echo -e "\nComparison completed!"
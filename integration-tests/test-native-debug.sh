#!/bin/bash

# Debug test for native protocol

echo "Starting native protocol debug test..."

# Kill any existing server
lsof -ti:58072 | xargs kill -9 2>/dev/null || true

# Start server with debug logging
echo "Starting server with debug logging..."
../target/release/throttlecrab-server \
    --native --native-port 58072 \
    --store adaptive \
    --store-capacity 1000000 \
    --buffer-size 1000000 \
    --log-level debug &

SERVER_PID=$!
sleep 2

# Run a small test with debug output
echo -e "\nRunning small test (1 thread, 10 requests)..."
RUST_LOG=debug ../target/release/throttlecrab-integration-tests perf-test \
    --threads 1 \
    --requests 10 \
    --port 58072 \
    --transport native

# Cleanup
kill $SERVER_PID
wait $SERVER_PID 2>/dev/null || true

echo "Debug test completed!"
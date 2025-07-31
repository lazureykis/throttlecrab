#!/bin/bash

# Heavy load test for ThrottleCrab

set -e

echo "Building projects..."
cd .. && cargo build --release -p throttlecrab-server
cd integration-tests && cargo build --release

# Kill any existing server
lsof -ti:58080 | xargs kill -9 2>/dev/null || true

# Start server in background
echo -e "\nStarting ThrottleCrab server..."
../target/release/throttlecrab-server \
    --http --http-port 58080 \
    --store adaptive \
    --store-capacity 1000000 \
    --buffer-size 1000000 \
    --log-level warn &

SERVER_PID=$!
echo "Server started with PID: $SERVER_PID"

# Wait for server to start
sleep 2

# Run heavy performance test
echo -e "\nRunning heavy load test..."
echo "50 threads x 20,000 requests = 1,000,000 total requests"
../target/release/throttlecrab-integration-tests perf-test \
    --threads 50 \
    --requests 20000 \
    --port 58080

# Stop server
echo -e "\nStopping server..."
kill $SERVER_PID
wait $SERVER_PID 2>/dev/null || true

echo "Test completed!"
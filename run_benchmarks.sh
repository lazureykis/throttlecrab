#!/bin/bash

echo "Starting ThrottleCrab server..."
cargo run --features bin -- --server &
SERVER_PID=$!

# Give the server time to start
sleep 2

# Check if server is running
if ! kill -0 $SERVER_PID 2>/dev/null; then
    echo "Failed to start server"
    exit 1
fi

echo "Server started with PID: $SERVER_PID"
echo "Running benchmarks..."

# Run the benchmarks
cargo bench

# Kill the server
echo "Stopping server..."
kill $SERVER_PID
wait $SERVER_PID 2>/dev/null

echo "Benchmarks complete!"
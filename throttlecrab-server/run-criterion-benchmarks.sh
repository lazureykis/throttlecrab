#!/bin/bash

# Script to run Criterion benchmarks with required servers

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}ThrottleCrab Criterion Benchmarks${NC}"
echo -e "${BLUE}==================================${NC}"
echo ""

# Function to cleanup servers on exit
cleanup() {
    echo -e "\n${YELLOW}Cleaning up servers...${NC}"
    if [ ! -z "$NATIVE_PID" ]; then
        kill $NATIVE_PID 2>/dev/null || true
    fi
    if [ ! -z "$GRPC_PID" ]; then
        kill $GRPC_PID 2>/dev/null || true
    fi
    # Kill any servers on the benchmark ports
    lsof -ti:9092 | xargs kill -9 2>/dev/null || true
    lsof -ti:9093 | xargs kill -9 2>/dev/null || true
}

# Set trap to cleanup on exit
trap cleanup EXIT

# Build the server in release mode
echo -e "${BLUE}Building server in release mode...${NC}"
cargo build --release -p throttlecrab-server

# Start servers required for benchmarks
echo -e "\n${BLUE}Starting servers for benchmarks...${NC}"

# Start native server on port 9092
echo -e "${YELLOW}Starting native server on port 9092...${NC}"
../target/release/throttlecrab-server --native --native-port 9092 --store adaptive --log-level error 2>/dev/null &
NATIVE_PID=$!
echo "Native server PID: $NATIVE_PID"

# Start gRPC server on port 9093
echo -e "${YELLOW}Starting gRPC server on port 9093...${NC}"
../target/release/throttlecrab-server --grpc --grpc-port 9093 --store adaptive --log-level error 2>/dev/null &
GRPC_PID=$!
echo "gRPC server PID: $GRPC_PID"

# Wait for servers to start
echo -e "\n${YELLOW}Waiting for servers to be ready...${NC}"
sleep 3

# Check if servers are running
if ! kill -0 $NATIVE_PID 2>/dev/null; then
    echo -e "${RED}ERROR: Native server failed to start${NC}"
    exit 1
fi

if ! kill -0 $GRPC_PID 2>/dev/null; then
    echo -e "${RED}ERROR: gRPC server failed to start${NC}"
    exit 1
fi

echo -e "${GREEN}Servers started successfully!${NC}"

# Run the benchmarks
echo -e "\n${BLUE}Running Criterion benchmarks...${NC}"
echo ""

# Check which benchmark to run
BENCH_NAME="${1:-all}"

case $BENCH_NAME in
    tcp_throughput)
        echo -e "${YELLOW}Running TCP throughput benchmark...${NC}"
        cargo bench --bench tcp_throughput
        ;;
    connection_pool)
        echo -e "${YELLOW}Running connection pool benchmark...${NC}"
        cargo bench --bench connection_pool
        ;;
    protocol_comparison)
        echo -e "${YELLOW}Running protocol comparison benchmark...${NC}"
        cargo bench --bench protocol_comparison
        ;;
    grpc_throughput)
        echo -e "${YELLOW}Running gRPC throughput benchmark...${NC}"
        cargo bench --bench grpc_throughput
        ;;
    all|"")
        echo -e "${YELLOW}Running all benchmarks...${NC}"
        cargo bench
        ;;
    *)
        echo -e "${RED}Unknown benchmark: $BENCH_NAME${NC}"
        echo "Available benchmarks: tcp_throughput, connection_pool, protocol_comparison, grpc_throughput, all"
        exit 1
        ;;
esac

echo -e "\n${GREEN}Benchmarks completed successfully!${NC}"
echo -e "${BLUE}Results saved in target/criterion/${NC}"

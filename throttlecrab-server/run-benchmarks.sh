#!/bin/bash

# ThrottleCrab Unified Benchmark Runner
# This script runs both Criterion and integration benchmarks

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default values
TYPE="criterion"
BENCH_NAME="all"

# Function to print usage
usage() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "OPTIONS:"
    echo "  -t, --type TYPE          Benchmark type: criterion or integration (default: criterion)"
    echo "  -b, --bench NAME         Specific benchmark to run (default: all)"
    echo "  -h, --help               Show this help message"
    echo ""
    echo "Criterion benchmarks:"
    echo "  tcp_throughput          - TCP protocol throughput"
    echo "  connection_pool         - Connection pooling performance"
    echo "  protocol_comparison     - Compare all protocols"
    echo "  grpc_throughput        - gRPC protocol throughput"
    echo "  all                    - Run all Criterion benchmarks"
    echo ""
    echo "Integration benchmarks:"
    echo "  Will run the full integration test suite"
    echo ""
    echo "Examples:"
    echo "  $0                                  # Run all Criterion benchmarks"
    echo "  $0 -t criterion -b tcp_throughput   # Run specific Criterion benchmark"
    echo "  $0 -t integration                   # Run integration benchmarks"
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -t|--type)
            TYPE="$2"
            shift 2
            ;;
        -b|--bench)
            BENCH_NAME="$2"
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            usage
            exit 1
            ;;
    esac
done

# Ensure we're in the right directory
cd "$(dirname "$0")"

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

# Build the server in release mode
echo -e "${BLUE}Building server in release mode...${NC}"
cargo build --release -p throttlecrab-server

if [ "$TYPE" = "criterion" ]; then
    # Set trap to cleanup on exit
    trap cleanup EXIT
    
    echo -e "\n${BLUE}Running Criterion benchmarks...${NC}"
    
    # Start servers required for benchmarks
    echo -e "\n${BLUE}Starting servers for benchmarks...${NC}"
    
    # Start native server on port 9092
    echo -e "${YELLOW}Starting native server on port 9092...${NC}"
    ../target/release/throttlecrab-server --native --native-port 9092 --store adaptive --log-level error 2>/dev/null &
    NATIVE_PID=$!
    
    # Start gRPC server on port 9093
    echo -e "${YELLOW}Starting gRPC server on port 9093...${NC}"
    ../target/release/throttlecrab-server --grpc --grpc-port 9093 --store adaptive --log-level error 2>/dev/null &
    GRPC_PID=$!
    
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
        all)
            echo -e "${YELLOW}Running all benchmarks...${NC}"
            cargo bench
            ;;
        *)
            echo -e "${RED}Unknown benchmark: $BENCH_NAME${NC}"
            echo "Available benchmarks: tcp_throughput, connection_pool, protocol_comparison, grpc_throughput, all"
            exit 1
            ;;
    esac
    
    echo -e "\n${GREEN}Criterion benchmarks completed!${NC}"
    echo -e "${BLUE}Results saved in target/criterion/${NC}"
    
elif [ "$TYPE" = "integration" ]; then
    echo -e "\n${BLUE}Running integration benchmarks...${NC}"
    
    # Build and run integration tests
    cargo test --release -p throttlecrab-server --test '*' -- --nocapture
    
    echo -e "\n${GREEN}Integration benchmarks completed!${NC}"
else
    echo -e "${RED}Unknown benchmark type: $TYPE${NC}"
    echo "Valid types: criterion, integration"
    exit 1
fi
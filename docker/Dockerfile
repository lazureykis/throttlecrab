# Build stage
FROM rust:1.83-slim AS builder

# Install dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /usr/src/app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY throttlecrab/Cargo.toml ./throttlecrab/
COPY throttlecrab-server/Cargo.toml ./throttlecrab-server/
COPY integration-tests/Cargo.toml ./integration-tests/

# Create dummy source files to cache dependencies
RUN mkdir -p throttlecrab/src throttlecrab-server/src integration-tests/src && \
    echo "fn main() {}" > throttlecrab/src/lib.rs && \
    echo "fn main() {}" > throttlecrab-server/src/main.rs && \
    echo "fn main() {}" > integration-tests/src/lib.rs

# Build dependencies
RUN cargo build --release -p throttlecrab-server

# Remove dummy source files
RUN rm -rf throttlecrab/src throttlecrab-server/src integration-tests/src

# Copy actual source code
COPY throttlecrab/src ./throttlecrab/src
COPY throttlecrab-server/src ./throttlecrab-server/src
COPY throttlecrab-server/proto ./throttlecrab-server/proto
COPY throttlecrab-server/build.rs ./throttlecrab-server/

# Build the application
RUN touch throttlecrab/src/lib.rs throttlecrab-server/src/main.rs && \
    cargo build --release -p throttlecrab-server

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 throttlecrab

# Copy binary from builder
COPY --from=builder /usr/src/app/target/release/throttlecrab-server /usr/local/bin/throttlecrab-server

# Change ownership
RUN chown throttlecrab:throttlecrab /usr/local/bin/throttlecrab-server

# Switch to non-root user
USER throttlecrab

# Expose ports (HTTP, gRPC, Native)
EXPOSE 8080 50051 8072

# Set default environment variables
ENV THROTTLECRAB_LOG_LEVEL=info

# Default command - start with all protocols enabled
CMD ["throttlecrab-server", "--http", "--grpc", "--native"]
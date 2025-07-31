# Build stage
FROM rust:alpine AS builder

# Install build dependencies
RUN apk add --no-cache musl-dev protobuf-dev

WORKDIR /build

# Copy everything
COPY . .

RUN sed -i 's/"integration-tests",//' Cargo.toml

# Build the binary
RUN cargo build --release -p throttlecrab-server

# Runtime stage
FROM alpine:latest

# Install runtime dependencies
RUN apk add --no-cache ca-certificates

# Copy binary from builder
COPY --from=builder /build/target/release/throttlecrab-server /usr/local/bin/throttlecrab-server

# Create non-root user
RUN adduser -D -u 1000 throttlecrab

# Switch to non-root user
USER throttlecrab

# Expose ports (HTTP, gRPC, Native)
EXPOSE 8080 50051 8072

# Run the server
CMD ["throttlecrab-server", "--http", "--grpc", "--native"]

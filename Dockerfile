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

# Expose ports (HTTP, gRPC)
EXPOSE 8080 50051

# Set default environment variables
ENV THROTTLECRAB_HTTP=true
ENV THROTTLECRAB_GRPC=true
ENV THROTTLECRAB_LOG_LEVEL=info

# Run the server
CMD ["throttlecrab-server"]
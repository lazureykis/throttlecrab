# ThrottleCrab Server Dockerfile
#
# Minimal scratch-based image for maximum security
# Supports multiple protocols:
# - HTTP (port 8080) - REST API
# - gRPC (port 50051) - Service mesh integration
# - Redis (port 6379) - Redis-compatible RESP protocol
#
# Enable protocols via environment variables:
# - THROTTLECRAB_HTTP=true
# - THROTTLECRAB_GRPC=true
# - THROTTLECRAB_REDIS=true

FROM scratch

# Use build argument to determine architecture
ARG TARGETARCH

# Copy pre-built static binary for the target architecture
COPY target/${TARGETARCH}/throttlecrab-server /throttlecrab-server

# Expose ports (HTTP, gRPC, Redis)
EXPOSE 8080 50051 6379

# Set default environment variables
ENV THROTTLECRAB_HTTP=true
ENV THROTTLECRAB_GRPC=true
ENV THROTTLECRAB_REDIS=true
ENV THROTTLECRAB_LOG_LEVEL=info

# Run the server
CMD ["/throttlecrab-server"]

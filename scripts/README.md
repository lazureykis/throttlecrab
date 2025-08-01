# ThrottleCrab Scripts

This directory contains utility scripts for building and managing ThrottleCrab.

## Docker Build Script

### docker-build.sh

Multi-platform Docker build script that supports both local builds and pushing to Docker Hub.

```bash
# Build locally (current platform only)
./scripts/docker-build.sh

# Build and push multi-platform images to Docker Hub
./scripts/docker-build.sh --push
```

Features:
- Multi-platform support (linux/amd64, linux/arm64, linux/arm/v7)
- Automatic version detection from `Cargo.toml`
- Docker buildx management
- Login verification before push
- Tags images as both `version` and `latest`

Supported platforms:
- `linux/amd64` - Standard x86_64 servers and cloud instances
- `linux/arm64` - Apple Silicon, AWS Graviton, modern ARM servers
- `linux/arm/v7` - Raspberry Pi, IoT devices, embedded systems

## Environment Variables

- `DOCKER_USERNAME`: Override the default Docker Hub username (default: lazureykis)
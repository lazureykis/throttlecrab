# ThrottleCrab Scripts

This directory contains utility scripts for building and managing ThrottleCrab.

## Docker Build Script

### docker-push.sh

Multi-platform Docker build script that supports both local builds and pushing to Docker Hub.

```bash
# Build locally (current platform only)
./scripts/docker-push.sh

# Build and push multi-platform images to Docker Hub
./scripts/docker-push.sh --push
```

Features:
- Multi-platform support (linux/amd64, linux/arm64)
- Automatic version detection from `Cargo.toml`
- Docker buildx management
- Login verification before push
- Tags images as both `version` and `latest`

## Environment Variables

- `DOCKER_USERNAME`: Override the default Docker Hub username (default: lazureykis)
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Releases are cut by the `Release` workflow (see [RELEASING.md](RELEASING.md)),
which generates per-release notes on the
[GitHub releases page](https://github.com/lazureykis/throttlecrab/releases)
from the commit log. This file is the curated view: it records changes that
affect users of the `throttlecrab` and `throttlecrab-server` crates or the
published Docker image, and omits dependency bumps and CI-only changes.

## [0.4.5] - [0.4.39] - 2025-08 – 2026-07

Backfilled from git history. Most releases in this range were dependency
updates and release-automation work with no user-facing change; the entries
below are everything that altered runtime behavior or the published image.

### Changed

- **The server now binds to `0.0.0.0` by default instead of `127.0.0.1`** (0.4.30).
  This applies to the HTTP, gRPC, and Redis listeners, and means a default
  configuration is reachable from outside the host. Set `THROTTLECRAB_HTTP_HOST`
  (or the corresponding per-protocol variable / CLI flag) back to `127.0.0.1` to
  restore loopback-only binding.
- The Docker image enables the Redis protocol by default (0.4.24).
- The Docker image is built `FROM scratch` around a static musl binary (0.4.25).
- Release images are published to `ghcr.io/lazureykis/throttlecrab` (0.4.38).

### Added

- Graceful shutdown on `SIGTERM`/`SIGINT` so containers stop cleanly rather than
  being killed after the runtime's grace period (0.4.31).

## [0.4.4] - 2025-08-23

### Added
- Configurable maximum denied keys tracking via `--max-denied-keys` CLI flag and `THROTTLECRAB_MAX_DENIED_KEYS` environment variable
- Ability to completely disable denied keys tracking by setting max-denied-keys to 0 for maximum performance
- Builder pattern for Metrics configuration allowing future extensibility
- Safety limit of 10,000 keys maximum to prevent excessive memory usage

### Changed
- Refactored Metrics to use builder pattern for cleaner and more extensible configuration
- Made top_denied_keys field optional (Option<Mutex<TopDeniedKeys>>) to eliminate overhead when disabled
- Improved performance when denied keys tracking is disabled (no mutex locks, no string allocations, no HashMap operations)

### Fixed
- Added proper validation and error messages for max-denied-keys configuration values

## [0.4.3] - 2025-08-04

### Changed
- Simplified metrics collection by removing non-working and unnecessary metrics (#49)
- Kept only essential metrics: uptime, request counts, transport breakdown, allow/deny counts, errors, and top denied keys
- Removed connection tracking, latency histograms, store metrics, and advanced metrics that were not functioning properly

## [0.4.0] - 2025-08-01

### Added
- Advanced metrics collection and observability for rate limiter insights (#40)
- Redis/RESP protocol support for high-performance rate limiting (#38)
- Comprehensive metrics system with performance monitoring (#36)
- Local Docker build script replacing GitHub workflow (#39)

### Changed
- Simplified project structure and reduced redundancy (#34)
- Updated documentation to fix broken links and outdated references (#33)
- Added key length limitations and best practices documentation (#35)

### Removed
- Native transport protocol in favor of standardized protocols (#37)

## [0.3.0] - 2025-08-01

### Added
- Comprehensive test coverage for quantity variations and token replenishment scenarios
- Tests for edge cases including zero quantity, negative quantity, and burst limit handling
- Tests for fractional token accumulation and time jitter handling

### Fixed
- Fixed remaining token count calculation that was always showing full capacity
- Corrected the TAT (Theoretical Arrival Time) distance calculation in rate limiter
- Added division by zero protection in remaining count calculation

### Changed
- Removed client timestamps from all protocols (HTTP, gRPC) - server now always uses its own timestamps
- Simplified API by removing timestamp parameters from client requests
- Updated all documentation to reflect server-side timestamp usage

### Removed
- Client timestamp fields from all protocol definitions
- DST-related tests that were no longer relevant with server-side timestamps

## [0.2.5] - Previous version

### Changed
- Documentation updates to use HTTP as default transport
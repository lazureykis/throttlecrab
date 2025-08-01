# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
# throttlecrab

[![CI](https://github.com/lazureykis/throttlecrab/actions/workflows/ci.yml/badge.svg)](https://github.com/lazureykis/throttlecrab/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/throttlecrab.svg)](https://crates.io/crates/throttlecrab)
[![Documentation](https://docs.rs/throttlecrab/badge.svg)](https://docs.rs/throttlecrab)
[![License](https://img.shields.io/crates/l/throttlecrab.svg)](../LICENSE)

A high-performance GCRA (Generic Cell Rate Algorithm) rate limiter library for Rust.

## Features

- **Pure Rust**: Zero-dependency GCRA rate limiter implementation
- **GCRA algorithm**: Implements the Generic Cell Rate Algorithm for smooth and predictable rate limiting
- **High performance**: Lock-free design with minimal overhead
- **Flexible parameters**: Different rate limits per key with dynamic configuration
- **TTL support**: Automatic cleanup of expired entries
- **Multiple store implementations**: Choose the right storage strategy for your use case

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
throttlecrab = "0.4"
```

## Usage

```rust
use std::time::SystemTime;
use throttlecrab::{RateLimiter, PeriodicStore};

fn main() {
    // Create a rate limiter with an in-memory store
    let mut limiter = RateLimiter::new(PeriodicStore::new());

    // Check if a request is allowed
    // Parameters: key, max_burst, count_per_period, period (seconds), quantity, timestamp
    let (allowed, result) = limiter
        .rate_limit("api_key_123", 10, 100, 60, 1, SystemTime::now())
        .unwrap();

    if allowed {
        println!("Request allowed! Remaining: {}", result.remaining);
    } else {
        println!("Rate limit exceeded! Retry after: {:?}", result.retry_after);
    }
}
```

## Store Implementations

The library provides several store implementations optimized for different use cases:

- **PeriodicStore**: Cleans up expired entries at regular intervals (default)
- **AdaptiveStore**: Dynamically adapts cleanup frequency based on usage patterns
- **ProbabilisticStore**: Each operation has a probability of triggering cleanup

## What is GCRA?

The [Generic Cell Rate Algorithm (GCRA)](https://en.wikipedia.org/wiki/Generic_cell_rate_algorithm) is a rate limiting algorithm that provides:
- **Smooth traffic shaping**: No sudden bursts followed by long waits
- **Precise rate limiting**: Exact control over request rates
- **Fairness**: All clients get predictable access to resources
- **Memory efficiency**: O(1) space per key

GCRA works by tracking the "Theoretical Arrival Time" (TAT) of requests, ensuring consistent spacing between allowed requests while permitting controlled bursts.

## Server

Looking for a standalone rate limiting server? Check out [throttlecrab-server](https://crates.io/crates/throttlecrab-server) which provides HTTP and gRPC interfaces.

## License

MIT(./LICENSE)

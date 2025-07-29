# throttlecrab

A high-performance GCRA (Generic Cell Rate Algorithm) rate limiter for Rust.

## Features

- **GCRA-based rate limiting**: Implements the Generic Cell Rate Algorithm for smooth and fair rate limiting
- **High performance**: Optimized for minimal overhead
- **Thread-safe**: Safe to use across multiple threads
- **Async support**: Works seamlessly with async Rust applications
- **Flexible configuration**: Customizable rate limits and burst capacity

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
throttlecrab = "0.1.0"
```

## Usage

```rust
use throttlecrab::RateLimiter;
use std::time::Duration;

fn main() {
    // Create a rate limiter that allows 10 requests per second
    let limiter = RateLimiter::new(10, Duration::from_secs(1));
    
    // Check if a request is allowed
    if limiter.check() {
        println!("Request allowed!");
    } else {
        println!("Rate limit exceeded!");
    }
}
```

## What is GCRA?

The Generic Cell Rate Algorithm (GCRA) is a rate limiting algorithm that provides:
- Smooth traffic shaping
- Burst tolerance
- Fair resource allocation
- Predictable behavior

Unlike token bucket or leaky bucket algorithms, GCRA provides more consistent rate limiting without the "burst then wait" patterns.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
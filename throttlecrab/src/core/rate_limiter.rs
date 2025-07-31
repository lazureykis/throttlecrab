//! GCRA (Generic Cell Rate Algorithm) rate limiter implementation
//!
//! This module provides the main [`RateLimiter`] struct which implements
//! the GCRA algorithm for smooth, fair rate limiting with burst support.

use super::{CellError, Rate, store::Store};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Result of a rate limit check
///
/// Contains information about the current state of the rate limiter for a given key.
#[derive(Debug, Clone)]
pub struct RateLimitResult {
    /// The maximum number of requests allowed in a burst
    pub limit: i64,
    /// The number of requests remaining in the current window
    pub remaining: i64,
    /// Time until the rate limit resets to full capacity
    pub reset_after: Duration,
    /// Time to wait before the next request will be allowed (0 if request was allowed)
    pub retry_after: Duration,
}

/// GCRA (Generic Cell Rate Algorithm) Rate Limiter
///
/// This rate limiter implements the GCRA algorithm, providing smooth and fair rate limiting
/// with support for bursts. It requires a [`Store`] implementation to manage rate limit data.
///
/// # Example
///
/// ```
/// use throttlecrab::{RateLimiter, PeriodicStore};
/// use std::time::SystemTime;
///
/// let mut limiter = RateLimiter::new(PeriodicStore::new());
///
/// // Allow 100 requests per minute with a burst of 10
/// let (allowed, result) = limiter
///     .rate_limit("api_key", 10, 100, 60, 1, SystemTime::now())
///     .unwrap();
/// ```
pub struct RateLimiter<S: Store> {
    store: S,
}

impl<S: Store> RateLimiter<S> {
    /// Create a new rate limiter with the specified store
    ///
    /// # Example
    ///
    /// ```
    /// use throttlecrab::{RateLimiter, AdaptiveStore};
    ///
    /// let limiter = RateLimiter::new(AdaptiveStore::new());
    /// ```
    pub fn new(store: S) -> Self {
        RateLimiter { store }
    }

    /// Check if a request is allowed under the rate limit
    ///
    /// # Parameters
    ///
    /// - `key`: Unique identifier for the rate limit (e.g., user ID, API key)
    /// - `max_burst`: Maximum number of requests allowed in a burst
    /// - `count_per_period`: Total number of requests allowed per time period
    /// - `period`: Time period in seconds
    /// - `quantity`: Number of tokens to consume (typically 1)
    /// - `now`: Current time for the rate limit check
    ///
    /// # Returns
    ///
    /// Returns a tuple of:
    /// - `bool`: Whether the request is allowed
    /// - [`RateLimitResult`]: Current state of the rate limiter
    ///
    /// # Errors
    ///
    /// - [`CellError::NegativeQuantity`]: If quantity is negative
    /// - [`CellError::InvalidRateLimit`]: If rate limit parameters are invalid
    /// - [`CellError::Internal`]: If there's an internal error
    ///
    /// # Example
    ///
    /// ```
    /// use throttlecrab::{RateLimiter, PeriodicStore};
    /// use std::time::SystemTime;
    ///
    /// let mut limiter = RateLimiter::new(PeriodicStore::new());
    ///
    /// // Check if user can make a request (10 burst, 100 per minute)
    /// match limiter.rate_limit("user:123", 10, 100, 60, 1, SystemTime::now()) {
    ///     Ok((true, result)) => {
    ///         println!("Request allowed! {} remaining", result.remaining);
    ///     }
    ///     Ok((false, result)) => {
    ///         println!("Rate limited! Retry after {} seconds", result.retry_after.as_secs());
    ///     }
    ///     Err(e) => eprintln!("Error: {}", e),
    /// }
    /// ```
    pub fn rate_limit(
        &mut self,
        key: &str,
        max_burst: i64,
        count_per_period: i64,
        period: i64,
        quantity: i64,
        now: SystemTime,
    ) -> Result<(bool, RateLimitResult), CellError> {
        if quantity < 0 {
            return Err(CellError::NegativeQuantity(quantity));
        }

        if max_burst <= 0 || count_per_period <= 0 || period <= 0 {
            return Err(CellError::InvalidRateLimit);
        }

        // Calculate rate parameters
        let rate = Rate::from_count_and_period(count_per_period, period);
        let emission_interval = rate.period();
        let delay_variation_tolerance = emission_interval * (max_burst - 1) as u32;
        let limit = max_burst;

        // Convert time to nanoseconds, handling potential errors gracefully
        let now_ns = match now.duration_since(UNIX_EPOCH) {
            Ok(duration) => duration.as_nanos() as i64,
            Err(e) => {
                // Time went backwards - use a fallback approach
                // Calculate a reasonable fallback time based on the period
                // This allows the system to continue operating with a fresh window
                match SystemTime::now().duration_since(UNIX_EPOCH) {
                    Ok(current) => {
                        // Use current time minus the period as a safe fallback
                        let period_ns = (period as u64).saturating_mul(1_000_000_000);
                        current.as_nanos().saturating_sub(period_ns as u128) as i64
                    }
                    Err(_) => {
                        // If we still can't get a valid time, return an error
                        return Err(CellError::Internal(format!("System time error: {e}")));
                    }
                }
            }
        };

        // Retry loop with limit to prevent stack overflow
        const MAX_RETRIES: u32 = 10;
        let mut retries = 0;

        loop {
            let tat_val = self.store.get(key, now).map_err(CellError::Internal)?;

            // Calculate the theoretical arrival time for this request
            let emission_interval_ns = emission_interval.as_nanos() as i64;
            let delay_variation_tolerance_ns = delay_variation_tolerance.as_nanos() as i64;

            // Initialize TAT or get from store
            let tat = if let Some(stored_tat) = tat_val {
                // Use stored TAT but ensure it's not too far in the past
                let min_tat = now_ns.saturating_sub(delay_variation_tolerance_ns);
                stored_tat.max(min_tat)
            } else {
                // First request - start with TAT = now - emission_interval
                // This accounts for the token we're about to use
                now_ns.saturating_sub(emission_interval_ns)
            };

            // Calculate new TAT if this request is allowed
            // Use saturating_mul to prevent overflow
            let increment = emission_interval_ns.saturating_mul(quantity);
            let new_tat = tat.saturating_add(increment);

            // Check if request is allowed
            let allow_at = new_tat.saturating_sub(delay_variation_tolerance_ns);
            let allowed = now_ns >= allow_at;

            if allowed {
                // Update the store with new TAT
                let ttl = Duration::from_nanos(
                    new_tat
                        .saturating_sub(now_ns)
                        .saturating_add(delay_variation_tolerance_ns) as u64,
                );

                // Try to update - if it fails due to race condition, retry
                let success = if let Some(old_tat) = tat_val {
                    self.store
                        .compare_and_swap_with_ttl(key, old_tat, new_tat, ttl, now)
                        .map_err(CellError::Internal)?
                } else {
                    // First time seeing this key
                    self.store
                        .set_if_not_exists_with_ttl(key, new_tat, ttl, now)
                        .map_err(CellError::Internal)?
                };

                if !success {
                    // Race condition - retry with limit
                    retries += 1;
                    if retries >= MAX_RETRIES {
                        return Err(CellError::Internal("Max retries exceeded".into()));
                    }
                    continue;
                }
            }

            // Calculate result
            let current_tat = if allowed { new_tat } else { tat };

            // Calculate remaining tokens AFTER this request
            // Remaining = how many more tokens we can use before hitting the limit
            // When TAT = now + tolerance, we've used all burst capacity
            // When TAT = now - tolerance, we have full burst capacity
            let tat_from_now = current_tat.saturating_sub(now_ns);

            // Calculate how many tokens we can still use
            let remaining = if tat_from_now >= delay_variation_tolerance_ns {
                // TAT is at or beyond the limit
                0
            } else {
                // How much room we have until we hit the limit
                let room_ns = delay_variation_tolerance_ns.saturating_sub(tat_from_now);
                // This gives us how many more emission intervals we can fit
                let remaining_exact = room_ns / emission_interval_ns;
                remaining_exact.max(0)
            };

            let reset_after = Duration::from_nanos(
                current_tat
                    .saturating_sub(now_ns)
                    .saturating_add(delay_variation_tolerance_ns)
                    .max(0) as u64,
            );

            let retry_after = if allowed {
                Duration::ZERO
            } else {
                Duration::from_nanos(allow_at.saturating_sub(now_ns).max(0) as u64)
            };

            return Ok((
                allowed,
                RateLimitResult {
                    limit,
                    remaining,
                    reset_after,
                    retry_after,
                },
            ));
        }
    }
}

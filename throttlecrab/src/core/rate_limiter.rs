use super::{CellError, Rate, store::Store};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Result of a rate limit check
#[derive(Debug, Clone)]
pub struct RateLimitResult {
    pub limit: i64,
    pub remaining: i64,
    pub reset_after: Duration,
    pub retry_after: Duration,
}

/// GCRA Rate Limiter implementation (similar to redis-cell)
pub struct RateLimiter<S: Store> {
    store: S,
}

impl<S: Store> RateLimiter<S> {
    /// Create a new rate limiter with a store
    pub fn new(store: S) -> Self {
        RateLimiter { store }
    }

    /// Check if a request is allowed and update state
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

        let tat_val = self.store.get(key, now).map_err(CellError::Internal)?;

        let now_ns = now.duration_since(UNIX_EPOCH).unwrap().as_nanos() as i64;

        // Calculate the theoretical arrival time for this request
        let emission_interval_ns = emission_interval.as_nanos() as i64;
        let delay_variation_tolerance_ns = delay_variation_tolerance.as_nanos() as i64;

        // Initialize TAT or get from store
        let tat = if let Some(stored_tat) = tat_val {
            // Use stored TAT but ensure it's not too far in the past
            let min_tat = now_ns - delay_variation_tolerance_ns;
            stored_tat.max(min_tat)
        } else {
            // First request - start with TAT = now - emission_interval
            // This accounts for the token we're about to use
            now_ns - emission_interval_ns
        };

        // Calculate new TAT if this request is allowed
        let increment = emission_interval_ns * quantity;
        let new_tat = tat + increment;

        // Check if request is allowed
        let allow_at = new_tat - delay_variation_tolerance_ns;
        let allowed = now_ns >= allow_at;

        if allowed {
            // Update the store with new TAT
            let ttl =
                Duration::from_nanos((new_tat - now_ns + delay_variation_tolerance_ns) as u64);

            // Try to update - if it fails due to race condition, recalculate
            if let Some(old_tat) = tat_val {
                let success = self
                    .store
                    .compare_and_swap_with_ttl(key, old_tat, new_tat, ttl, now)
                    .map_err(CellError::Internal)?;

                if !success {
                    // Race condition - retry
                    return self.rate_limit(
                        key,
                        max_burst,
                        count_per_period,
                        period,
                        quantity,
                        now,
                    );
                }
            } else {
                // First time seeing this key
                let success = self
                    .store
                    .set_if_not_exists_with_ttl(key, new_tat, ttl, now)
                    .map_err(CellError::Internal)?;

                if !success {
                    // Race condition - retry
                    return self.rate_limit(
                        key,
                        max_burst,
                        count_per_period,
                        period,
                        quantity,
                        now,
                    );
                }
            }
        }

        // Calculate result
        let current_tat = if allowed { new_tat } else { tat };

        // Calculate remaining tokens AFTER this request
        // Remaining = how many more tokens we can use before hitting the limit
        // When TAT = now + tolerance, we've used all burst capacity
        // When TAT = now - tolerance, we have full burst capacity
        let tat_from_now = current_tat - now_ns;

        // Calculate how many tokens we can still use
        let remaining = if tat_from_now >= delay_variation_tolerance_ns {
            // TAT is at or beyond the limit
            0
        } else {
            // How much room we have until we hit the limit
            let room_ns = delay_variation_tolerance_ns - tat_from_now;
            // This gives us how many more emission intervals we can fit
            let remaining_exact = room_ns / emission_interval_ns;
            remaining_exact.max(0)
        };

        let reset_after = Duration::from_nanos(
            (current_tat.saturating_sub(now_ns) + delay_variation_tolerance_ns).max(0) as u64,
        );

        let retry_after = if allowed {
            Duration::ZERO
        } else {
            Duration::from_nanos((allow_at - now_ns).max(0) as u64)
        };

        Ok((
            allowed,
            RateLimitResult {
                limit,
                remaining,
                reset_after,
                retry_after,
            },
        ))
    }
}

use super::{store::Store, CellError, Rate};
use std::time::{Duration, UNIX_EPOCH};

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
    /// Also known as "burst"
    delay_variation_tolerance: Duration,
    /// The rate at which tokens are added
    emission_interval: Duration,
    /// Maximum number of tokens (burst size)
    limit: i64,
}

impl<S: Store> RateLimiter<S> {
    /// Create a new rate limiter from burst and rate parameters
    pub fn new_from_parameters(
        store: S,
        max_burst: i64,
        count_per_period: i64,
        period_seconds: i64,
    ) -> Result<Self, CellError> {
        if max_burst <= 0 || count_per_period <= 0 || period_seconds <= 0 {
            return Err(CellError::InvalidRateLimit);
        }

        let rate = Rate::from_count_and_period(count_per_period, period_seconds);
        let emission_interval = rate.period();
        
        // delay_variation_tolerance = (burst - 1) * emission_interval
        let delay_variation_tolerance = emission_interval * (max_burst - 1) as u32;

        Ok(RateLimiter {
            store,
            delay_variation_tolerance,
            emission_interval,
            limit: max_burst,
        })
    }

    /// Check if a request is allowed and update state
    pub fn rate_limit(&mut self, key: &str, quantity: i64) -> Result<(bool, RateLimitResult), CellError> {
        if quantity < 0 {
            return Err(CellError::NegativeQuantity(quantity));
        }

        let (tat_val, now) = self.store.get_with_time(key)
            .map_err(|e| CellError::Internal(e))?;
        
        let now_ns = now.duration_since(UNIX_EPOCH).unwrap().as_nanos() as i64;
        
        tracing::debug!("rate_limit: key={}, quantity={}, stored_tat={:?}, now_ns={}", 
            key, quantity, tat_val, now_ns);
        
        // Calculate the theoretical arrival time for this request
        let emission_interval_ns = self.emission_interval.as_nanos() as i64;
        let delay_variation_tolerance_ns = self.delay_variation_tolerance.as_nanos() as i64;
        
        // Initialize TAT or get from store
        let tat = if let Some(stored_tat) = tat_val {
            // Use stored TAT but ensure it's not too far in the past
            let min_tat = now_ns - delay_variation_tolerance_ns;
            stored_tat.max(min_tat)
        } else {
            // First request - start with TAT that gives full burst capacity
            now_ns - delay_variation_tolerance_ns
        };
        
        // Calculate new TAT if this request is allowed
        let increment = emission_interval_ns * quantity;
        let new_tat = tat + increment;
        
        // Check if request is allowed
        let allow_at = new_tat - delay_variation_tolerance_ns;
        let allowed = now_ns >= allow_at;
        
        if allowed {
            // Update the store with new TAT
            let ttl = Duration::from_nanos((new_tat - now_ns + delay_variation_tolerance_ns) as u64);
            
            // Try to update - if it fails due to race condition, recalculate
            if let Some(old_tat) = tat_val {
                let success = self.store.compare_and_swap_with_ttl(key, old_tat, new_tat, ttl)
                    .map_err(|e| CellError::Internal(e))?;
                    
                if !success {
                    // Race condition - retry
                    return self.rate_limit(key, quantity);
                }
            } else {
                // First time seeing this key
                let success = self.store.set_if_not_exists_with_ttl(key, new_tat, ttl)
                    .map_err(|e| CellError::Internal(e))?;
                    
                if !success {
                    // Race condition - retry
                    return self.rate_limit(key, quantity);
                }
            }
        }
        
        // Calculate result
        let current_tat = if allowed { new_tat } else { tat };
        
        // Calculate remaining tokens AFTER this request
        // This shows how many more requests can be made
        let tat_distance = current_tat.saturating_sub(now_ns);
        let max_distance = delay_variation_tolerance_ns;
        
        // The closer TAT is to now, the more tokens we have available
        let remaining = if tat_distance >= max_distance {
            0
        } else {
            // Calculate how many tokens are available after this request
            let available_ns = max_distance - tat_distance;
            let remaining_exact = available_ns / emission_interval_ns;
            remaining_exact.min(self.limit - 1) // Can't exceed burst - 1
        };
        
        let reset_after = Duration::from_nanos(
            (current_tat.saturating_sub(now_ns) + delay_variation_tolerance_ns).max(0) as u64
        );
        
        let retry_after = if allowed {
            Duration::ZERO
        } else {
            Duration::from_nanos((allow_at - now_ns).max(0) as u64)
        };
        
        Ok((allowed, RateLimitResult {
            limit: self.limit,
            remaining,
            reset_after,
            retry_after,
        }))
    }
}
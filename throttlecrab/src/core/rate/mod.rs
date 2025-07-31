//! Rate calculation for the GCRA algorithm
//!
//! This module provides the [`Rate`] type which represents emission intervals
//! for token-based rate limiting. It converts human-friendly rate specifications
//! (e.g., "100 requests per second") into precise emission intervals.

use std::time::Duration;

#[cfg(test)]
mod tests;

/// Rate defines the emission interval for the rate limiter
///
/// The `Rate` type represents how frequently tokens are replenished in the
/// rate limiter. It encapsulates the concept of "N requests per time period"
/// as a duration between each token emission.
///
/// # Examples
///
/// ```
/// use throttlecrab::Rate;
/// use std::time::Duration;
///
/// // 10 requests per second
/// let rate = Rate::per_second(10);
/// assert_eq!(rate.period(), Duration::from_millis(100));
///
/// // 60 requests per minute (1 per second)
/// let rate = Rate::per_minute(60);
/// assert_eq!(rate.period(), Duration::from_secs(1));
///
/// // Custom rate: 1 request every 2.5 seconds
/// let rate = Rate::new(Duration::from_millis(2500));
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Rate {
    period: Duration,
}

impl Rate {
    /// Creates a new rate with a custom period between token emissions
    ///
    /// # Parameters
    ///
    /// - `period`: Duration between each token emission
    ///
    /// # Example
    ///
    /// ```
    /// use throttlecrab::Rate;
    /// use std::time::Duration;
    ///
    /// // One token every 500ms
    /// let rate = Rate::new(Duration::from_millis(500));
    /// ```
    pub fn new(period: Duration) -> Self {
        Rate { period }
    }

    /// Creates a rate of n requests per second
    ///
    /// # Parameters
    ///
    /// - `n`: Number of requests allowed per second
    ///
    /// # Example
    ///
    /// ```
    /// use throttlecrab::Rate;
    ///
    /// // 100 requests per second
    /// let rate = Rate::per_second(100);
    /// ```
    pub fn per_second(n: u64) -> Self {
        Rate {
            period: Duration::from_secs(1) / n as u32,
        }
    }

    /// Creates a rate of n requests per minute
    ///
    /// # Parameters
    ///
    /// - `n`: Number of requests allowed per minute
    ///
    /// # Example
    ///
    /// ```
    /// use throttlecrab::Rate;
    ///
    /// // 1000 requests per minute
    /// let rate = Rate::per_minute(1000);
    /// ```
    pub fn per_minute(n: u64) -> Self {
        Rate {
            period: Duration::from_secs(60) / n as u32,
        }
    }

    /// Creates a rate of n requests per hour
    ///
    /// # Parameters
    ///
    /// - `n`: Number of requests allowed per hour
    ///
    /// # Example
    ///
    /// ```
    /// use throttlecrab::Rate;
    ///
    /// // 10,000 requests per hour
    /// let rate = Rate::per_hour(10_000);
    /// ```
    pub fn per_hour(n: u64) -> Self {
        Rate {
            period: Duration::from_secs(3600) / n as u32,
        }
    }

    /// Creates a rate of n requests per day
    ///
    /// # Parameters
    ///
    /// - `n`: Number of requests allowed per day
    ///
    /// # Example
    ///
    /// ```
    /// use throttlecrab::Rate;
    ///
    /// // 100,000 requests per day
    /// let rate = Rate::per_day(100_000);
    /// ```
    pub fn per_day(n: u64) -> Self {
        Rate {
            period: Duration::from_secs(86400) / n as u32,
        }
    }

    /// Creates a rate from count and period in seconds
    ///
    /// This method calculates the emission interval based on the total number
    /// of requests allowed over a given time period.
    ///
    /// # Parameters
    ///
    /// - `count`: Total number of requests allowed in the period
    /// - `period_seconds`: Time period in seconds
    ///
    /// # Returns
    ///
    /// A `Rate` with the calculated emission interval. If invalid parameters
    /// are provided (count <= 0 or period_seconds <= 0), returns a very slow
    /// rate (effectively blocking all requests).
    ///
    /// # Example
    ///
    /// ```
    /// use throttlecrab::Rate;
    ///
    /// // 100 requests per 60 seconds = 1 request every 0.6 seconds
    /// let rate = Rate::from_count_and_period(100, 60);
    /// ```
    pub fn from_count_and_period(count: i64, period_seconds: i64) -> Self {
        if count <= 0 || period_seconds <= 0 {
            // Return a very slow rate if invalid
            return Rate {
                period: Duration::from_secs(u64::MAX),
            };
        }

        let period_ns = (period_seconds as f64 * 1_000_000_000.0 / count as f64) as u64;
        Rate {
            period: Duration::from_nanos(period_ns),
        }
    }

    /// Returns the emission interval (period) of this rate
    ///
    /// This is the duration between each token emission in the rate limiter.
    ///
    /// # Example
    ///
    /// ```
    /// use throttlecrab::Rate;
    /// use std::time::Duration;
    ///
    /// let rate = Rate::per_second(10);
    /// assert_eq!(rate.period(), Duration::from_millis(100));
    /// ```
    pub fn period(&self) -> Duration {
        self.period
    }
}

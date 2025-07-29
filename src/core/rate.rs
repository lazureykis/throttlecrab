use std::time::Duration;

/// Rate defines the speed of the rate limiter.
#[derive(Debug, Clone, Copy)]
pub struct Rate {
    period: Duration,
}

impl Rate {
    /// Creates a new rate with a custom period
    pub fn new(period: Duration) -> Self {
        Rate { period }
    }

    /// Creates a rate per second
    pub fn per_second(n: u64) -> Self {
        Rate {
            period: Duration::from_secs(1) / n as u32,
        }
    }

    /// Creates a rate per minute
    pub fn per_minute(n: u64) -> Self {
        Rate {
            period: Duration::from_secs(60) / n as u32,
        }
    }

    /// Creates a rate per hour
    pub fn per_hour(n: u64) -> Self {
        Rate {
            period: Duration::from_secs(3600) / n as u32,
        }
    }

    /// Creates a rate per day
    pub fn per_day(n: u64) -> Self {
        Rate {
            period: Duration::from_secs(86400) / n as u32,
        }
    }

    /// Creates a rate from count and period (matching redis-cell API)
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

    /// Returns the period of this rate
    pub fn period(&self) -> Duration {
        self.period
    }
}
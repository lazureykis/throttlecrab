use anyhow::Result;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::time::sleep;

#[derive(Debug, Clone)]
pub enum WorkloadPattern {
    /// Constant rate
    Steady { rps: u64 },
    /// Burst pattern: high rate for burst_duration, then low rate
    Burst {
        high_rps: u64,
        low_rps: u64,
        burst_duration: Duration,
        quiet_duration: Duration,
    },
    /// Gradually increase load
    Ramp {
        start_rps: u64,
        end_rps: u64,
        duration: Duration,
    },
    /// Random rate within bounds
    Random { min_rps: u64, max_rps: u64 },
    /// Sine wave pattern
    Wave {
        base_rps: u64,
        amplitude: u64,
        period: Duration,
    },
}

#[derive(Debug, Clone)]
pub struct WorkloadConfig {
    pub pattern: WorkloadPattern,
    pub duration: Duration,
    pub key_space_size: usize,
    pub key_pattern: KeyPattern,
}

#[derive(Debug, Clone)]
pub enum KeyPattern {
    /// Sequential keys: key_0, key_1, ...
    Sequential,
    /// Random keys from a fixed pool
    Random { pool_size: usize },
    /// Zipfian distribution (hot keys)
    Zipfian { alpha: f64 },
    /// User ID pattern: user_<id>_resource_<resource>
    UserResource { users: usize, resources: usize },
}

pub struct WorkloadStats {
    pub total_requests: AtomicU64,
    pub successful_requests: AtomicU64,
    pub failed_requests: AtomicU64,
    pub rate_limited: AtomicU64,
    pub latencies: parking_lot::RwLock<Vec<Duration>>,
}

impl WorkloadStats {
    pub fn new() -> Self {
        Self {
            total_requests: AtomicU64::new(0),
            successful_requests: AtomicU64::new(0),
            failed_requests: AtomicU64::new(0),
            rate_limited: AtomicU64::new(0),
            latencies: parking_lot::RwLock::new(Vec::with_capacity(100_000)),
        }
    }

    pub fn record_request(&self, latency: Duration, rate_limited: bool) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);

        if rate_limited {
            self.rate_limited.fetch_add(1, Ordering::Relaxed);
        } else {
            self.successful_requests.fetch_add(1, Ordering::Relaxed);
        }

        if let Some(mut latencies) = self.latencies.try_write() {
            if latencies.len() < latencies.capacity() {
                latencies.push(latency);
            }
        }
    }

    pub fn record_failure(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.failed_requests.fetch_add(1, Ordering::Relaxed);
    }

    pub fn get_summary(&self) -> WorkloadSummary {
        let latencies = self.latencies.read();
        let mut sorted_latencies = latencies.clone();
        sorted_latencies.sort();

        let p50 = percentile(&sorted_latencies, 0.5);
        let p90 = percentile(&sorted_latencies, 0.9);
        let p95 = percentile(&sorted_latencies, 0.95);
        let p99 = percentile(&sorted_latencies, 0.99);
        let p999 = percentile(&sorted_latencies, 0.999);

        WorkloadSummary {
            total_requests: self.total_requests.load(Ordering::Relaxed),
            successful_requests: self.successful_requests.load(Ordering::Relaxed),
            failed_requests: self.failed_requests.load(Ordering::Relaxed),
            rate_limited: self.rate_limited.load(Ordering::Relaxed),
            latency_p50: p50,
            latency_p90: p90,
            latency_p95: p95,
            latency_p99: p99,
            latency_p999: p999,
        }
    }
}

#[derive(Debug)]
pub struct WorkloadSummary {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub rate_limited: u64,
    pub latency_p50: Duration,
    pub latency_p90: Duration,
    pub latency_p95: Duration,
    pub latency_p99: Duration,
    pub latency_p999: Duration,
}

impl WorkloadSummary {
    pub fn print_summary(&self, duration: Duration) {
        let rps = self.total_requests as f64 / duration.as_secs_f64();
        let success_rate = self.successful_requests as f64 / self.total_requests as f64 * 100.0;
        let rate_limit_rate = self.rate_limited as f64 / self.total_requests as f64 * 100.0;

        println!("\n=== Workload Summary ===");
        println!("Duration: {:?}", duration);
        println!("Total requests: {}", self.total_requests);
        println!("Requests/sec: {:.2}", rps);
        println!(
            "Successful: {} ({:.2}%)",
            self.successful_requests, success_rate
        );
        println!(
            "Rate limited: {} ({:.2}%)",
            self.rate_limited, rate_limit_rate
        );
        println!("Failed: {}", self.failed_requests);
        println!("\nLatency percentiles:");
        println!("  P50: {:?}", self.latency_p50);
        println!("  P90: {:?}", self.latency_p90);
        println!("  P95: {:?}", self.latency_p95);
        println!("  P99: {:?}", self.latency_p99);
        println!("  P99.9: {:?}", self.latency_p999);
    }
}

pub struct WorkloadGenerator {
    config: WorkloadConfig,
    stats: Arc<WorkloadStats>,
}

impl WorkloadGenerator {
    pub fn new(config: WorkloadConfig) -> Self {
        Self {
            config,
            stats: Arc::new(WorkloadStats::new()),
        }
    }

    pub fn stats(&self) -> Arc<WorkloadStats> {
        self.stats.clone()
    }

    pub async fn run<F, Fut>(&self, request_fn: F) -> Result<()>
    where
        F: FnMut(String) -> Fut + Send + Clone + 'static,
        Fut: std::future::Future<Output = Result<bool>> + Send,
    {
        let start = Instant::now();
        let mut request_count = 0u64;

        while start.elapsed() < self.config.duration {
            let current_rps = self.calculate_current_rps(start.elapsed());
            let delay = Duration::from_secs_f64(1.0 / current_rps as f64);

            // Generate key
            let key = self.generate_key(request_count);

            // Clone for async task
            let stats = self.stats.clone();
            let mut req_fn = request_fn.clone();

            // Spawn request
            tokio::spawn(async move {
                let request_start = Instant::now();
                match req_fn(key).await {
                    Ok(rate_limited) => {
                        let latency = request_start.elapsed();
                        stats.record_request(latency, rate_limited);
                    }
                    Err(_) => {
                        stats.record_failure();
                    }
                }
            });

            request_count += 1;
            sleep(delay).await;
        }

        Ok(())
    }

    fn calculate_current_rps(&self, elapsed: Duration) -> u64 {
        match &self.config.pattern {
            WorkloadPattern::Steady { rps } => *rps,

            WorkloadPattern::Burst {
                high_rps,
                low_rps,
                burst_duration,
                quiet_duration,
            } => {
                let cycle_duration = burst_duration.as_secs_f64() + quiet_duration.as_secs_f64();
                let cycle_position = elapsed.as_secs_f64() % cycle_duration;

                if cycle_position < burst_duration.as_secs_f64() {
                    *high_rps
                } else {
                    *low_rps
                }
            }

            WorkloadPattern::Ramp {
                start_rps,
                end_rps,
                duration,
            } => {
                let progress = (elapsed.as_secs_f64() / duration.as_secs_f64()).min(1.0);
                let rps = start_rps + ((end_rps - start_rps) as f64 * progress) as u64;
                rps
            }

            WorkloadPattern::Random { min_rps, max_rps } => {
                use rand::Rng;
                rand::thread_rng().gen_range(*min_rps..=*max_rps)
            }

            WorkloadPattern::Wave {
                base_rps,
                amplitude,
                period,
            } => {
                let phase =
                    (elapsed.as_secs_f64() / period.as_secs_f64()) * 2.0 * std::f64::consts::PI;
                let wave = phase.sin();
                let rps = *base_rps as f64 + (*amplitude as f64 * wave);
                rps.max(1.0) as u64
            }
        }
    }

    fn generate_key(&self, request_num: u64) -> String {
        match &self.config.key_pattern {
            KeyPattern::Sequential => format!("key_{}", request_num),

            KeyPattern::Random { pool_size } => {
                use rand::Rng;
                let key_id = rand::thread_rng().gen_range(0..*pool_size);
                format!("key_{}", key_id)
            }

            KeyPattern::Zipfian { alpha } => {
                // Simple zipfian approximation
                use rand::Rng;
                let mut rng = rand::thread_rng();
                let u: f64 = rng.r#gen::<f64>();
                let key_id = ((self.config.key_space_size as f64) * u.powf(-1.0 / alpha)) as usize;
                format!("key_{}", key_id.min(self.config.key_space_size - 1))
            }

            KeyPattern::UserResource { users, resources } => {
                use rand::Rng;
                let mut rng = rand::thread_rng();
                let user_id = rng.gen_range(0..*users);
                let resource_id = rng.gen_range(0..*resources);
                format!("user_{}_resource_{}", user_id, resource_id)
            }
        }
    }
}

fn percentile(sorted_values: &[Duration], p: f64) -> Duration {
    if sorted_values.is_empty() {
        return Duration::ZERO;
    }

    let index = ((sorted_values.len() as f64 - 1.0) * p) as usize;
    sorted_values[index]
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_workload_patterns() {
        let config = WorkloadConfig {
            pattern: WorkloadPattern::Steady { rps: 100 },
            duration: Duration::from_secs(1),
            key_space_size: 1000,
            key_pattern: KeyPattern::Sequential,
        };

        let generator = WorkloadGenerator::new(config);
        assert_eq!(
            generator.calculate_current_rps(Duration::from_millis(500)),
            100
        );
    }

    #[test]
    fn test_key_generation() {
        let config = WorkloadConfig {
            pattern: WorkloadPattern::Steady { rps: 100 },
            duration: Duration::from_secs(1),
            key_space_size: 1000,
            key_pattern: KeyPattern::Sequential,
        };

        let generator = WorkloadGenerator::new(config);
        assert_eq!(generator.generate_key(0), "key_0");
        assert_eq!(generator.generate_key(42), "key_42");
    }
}

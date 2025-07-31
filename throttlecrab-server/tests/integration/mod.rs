pub mod benchmark_suite;
pub mod multi_transport;
pub mod store_comparison;
pub mod transport_tests;
pub mod workload;

use std::time::Duration;

pub const DEFAULT_TEST_DURATION: Duration = Duration::from_secs(10);
pub const DEFAULT_WARMUP_DURATION: Duration = Duration::from_secs(2);
pub const DEFAULT_SAMPLE_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone)]
pub struct TestConfig {
    pub server_binary: String,
    pub test_duration: Duration,
    pub warmup_duration: Duration,
    pub sample_interval: Duration,
    pub output_dir: String,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            server_binary: "target/release/throttlecrab".to_string(),
            test_duration: DEFAULT_TEST_DURATION,
            warmup_duration: DEFAULT_WARMUP_DURATION,
            sample_interval: DEFAULT_SAMPLE_INTERVAL,
            output_dir: "test-results".to_string(),
        }
    }
}

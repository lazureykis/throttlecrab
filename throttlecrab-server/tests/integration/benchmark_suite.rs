use std::time::Duration;
use anyhow::Result;
use clap::{Parser, ValueEnum};

use super::workload::{WorkloadConfig, WorkloadPattern, KeyPattern};
use super::transport_tests::{Transport, run_transport_benchmark};
use super::store_comparison::run_store_comparison;
use super::multi_transport::run_multi_transport_test;

#[derive(Parser, Debug)]
#[command(
    name = "throttlecrab-benchmark",
    about = "Comprehensive benchmark suite for ThrottleCrab server"
)]
pub struct BenchmarkArgs {
    #[arg(long, value_enum, help = "Benchmark suite to run")]
    pub suite: Option<BenchmarkSuite>,
    
    #[arg(long, help = "Test duration in seconds", default_value = "30")]
    pub duration: u64,
    
    #[arg(long, help = "Warmup duration in seconds", default_value = "5")]
    pub warmup: u64,
    
    #[arg(long, help = "Output directory for results", default_value = "benchmark-results")]
    pub output_dir: String,
    
    #[arg(long, help = "Target requests per second", default_value = "10000")]
    pub target_rps: u64,
    
    #[arg(long, help = "Save detailed results as JSON")]
    pub json: bool,
    
    #[arg(long, help = "Compare results with previous run")]
    pub compare: Option<String>,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum BenchmarkSuite {
    /// Test all transports with steady load
    Transports,
    /// Compare different store types
    Stores,
    /// Test various workload patterns
    Workloads,
    /// Stress test with increasing load
    Stress,
    /// Test multiple transports concurrently
    MultiTransport,
    /// Run all benchmark suites
    All,
}

pub struct BenchmarkRunner {
    args: BenchmarkArgs,
}

impl BenchmarkRunner {
    pub fn new(args: BenchmarkArgs) -> Self {
        Self { args }
    }

    pub async fn run(&self) -> Result<()> {
        // Create output directory
        std::fs::create_dir_all(&self.args.output_dir)?;
        
        let suite = self.args.suite.clone().unwrap_or(BenchmarkSuite::All);
        
        match suite {
            BenchmarkSuite::Transports => self.run_transport_benchmarks().await?,
            BenchmarkSuite::Stores => self.run_store_benchmarks().await?,
            BenchmarkSuite::Workloads => self.run_workload_benchmarks().await?,
            BenchmarkSuite::Stress => self.run_stress_test().await?,
            BenchmarkSuite::MultiTransport => self.run_multi_transport_benchmark().await?,
            BenchmarkSuite::All => {
                println!("Running all benchmark suites...\n");
                self.run_transport_benchmarks().await?;
                self.run_store_benchmarks().await?;
                self.run_workload_benchmarks().await?;
                self.run_multi_transport_benchmark().await?;
                self.run_stress_test().await?;
            }
        }
        
        if let Some(compare_with) = &self.args.compare {
            self.compare_results(compare_with)?;
        }
        
        Ok(())
    }

    async fn run_transport_benchmarks(&self) -> Result<()> {
        println!("\n🚀 Running Transport Benchmarks\n");
        
        let workload = WorkloadConfig {
            pattern: WorkloadPattern::Steady { rps: self.args.target_rps },
            duration: Duration::from_secs(self.args.duration),
            key_space_size: 10_000,
            key_pattern: KeyPattern::Random { pool_size: 10_000 },
        };
        
        for transport in [Transport::Http, Transport::Grpc, Transport::MsgPack, Transport::Native] {
            run_transport_benchmark(transport, "periodic", workload.clone()).await?;
        }
        
        Ok(())
    }

    async fn run_store_benchmarks(&self) -> Result<()> {
        println!("\n💾 Running Store Comparison Benchmarks\n");
        
        let workload = WorkloadConfig {
            pattern: WorkloadPattern::Steady { rps: self.args.target_rps },
            duration: Duration::from_secs(self.args.duration),
            key_space_size: 10_000,
            key_pattern: KeyPattern::Random { pool_size: 10_000 },
        };
        
        run_store_comparison(workload).await?;
        
        Ok(())
    }

    async fn run_workload_benchmarks(&self) -> Result<()> {
        println!("\n📊 Running Workload Pattern Benchmarks\n");
        
        let patterns = vec![
            ("Steady Load", WorkloadPattern::Steady { rps: self.args.target_rps }),
            ("Burst Pattern", WorkloadPattern::Burst {
                high_rps: self.args.target_rps * 2,
                low_rps: self.args.target_rps / 4,
                burst_duration: Duration::from_secs(5),
                quiet_duration: Duration::from_secs(10),
            }),
            ("Ramp Up", WorkloadPattern::Ramp {
                start_rps: 100,
                end_rps: self.args.target_rps * 2,
                duration: Duration::from_secs(self.args.duration),
            }),
            ("Wave Pattern", WorkloadPattern::Wave {
                base_rps: self.args.target_rps,
                amplitude: self.args.target_rps / 2,
                period: Duration::from_secs(20),
            }),
        ];
        
        for (name, pattern) in patterns {
            println!("\n--- Testing {} ---", name);
            
            let workload = WorkloadConfig {
                pattern,
                duration: Duration::from_secs(self.args.duration),
                key_space_size: 10_000,
                key_pattern: KeyPattern::Random { pool_size: 10_000 },
            };
            
            run_transport_benchmark(Transport::Http, "adaptive", workload).await?;
        }
        
        Ok(())
    }

    async fn run_stress_test(&self) -> Result<()> {
        println!("\n💪 Running Stress Test\n");
        
        let max_rps = self.args.target_rps * 10;
        
        let workload = WorkloadConfig {
            pattern: WorkloadPattern::Ramp {
                start_rps: 1000,
                end_rps: max_rps,
                duration: Duration::from_secs(self.args.duration),
            },
            duration: Duration::from_secs(self.args.duration),
            key_space_size: 100_000,
            key_pattern: KeyPattern::Zipfian { alpha: 1.2 },
        };
        
        println!("Ramping from 1,000 to {} RPS over {} seconds", max_rps, self.args.duration);
        println!("Using Zipfian key distribution (hotspot testing)");
        
        run_transport_benchmark(Transport::Native, "adaptive", workload).await?;
        
        Ok(())
    }

    async fn run_multi_transport_benchmark(&self) -> Result<()> {
        println!("\n🔀 Running Multi-Transport Concurrent Test\n");
        
        let workload = WorkloadConfig {
            pattern: WorkloadPattern::Steady { rps: self.args.target_rps / 4 },
            duration: Duration::from_secs(self.args.duration),
            key_space_size: 10_000,
            key_pattern: KeyPattern::Random { pool_size: 10_000 },
        };
        
        run_multi_transport_test(workload).await?;
        
        Ok(())
    }

    fn compare_results(&self, _previous_run: &str) -> Result<()> {
        println!("\n📈 Comparing with previous results...");
        // TODO: Implement result comparison
        println!("Result comparison not yet implemented");
        Ok(())
    }
}

pub fn print_benchmark_header() {
    println!(r#"
╔══════════════════════════════════════════════════════════════╗
║           ThrottleCrab Performance Benchmark Suite           ║
╚══════════════════════════════════════════════════════════════╝
    "#);
}
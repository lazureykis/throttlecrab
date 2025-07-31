mod integration;

use anyhow::Result;
use clap::Parser;
use integration::benchmark_suite::{BenchmarkArgs, BenchmarkRunner, print_benchmark_header};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("throttlecrab=warn".parse()?)
        )
        .init();
    
    print_benchmark_header();
    
    let args = BenchmarkArgs::parse();
    let runner = BenchmarkRunner::new(args);
    
    runner.run().await?;
    
    println!("\n✅ Benchmark suite completed successfully!");
    
    Ok(())
}
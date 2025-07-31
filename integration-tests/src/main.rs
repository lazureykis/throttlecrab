use anyhow::Result;
use clap::{Parser, Subcommand};

mod perf_test;

#[derive(Parser)]
#[command(name = "throttlecrab-integration-tests")]
#[command(about = "Integration tests for ThrottleCrab", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run performance test
    PerfTest {
        /// Number of threads
        #[arg(short, long, default_value = "20")]
        threads: usize,

        /// Requests per thread
        #[arg(short, long, default_value = "5000")]
        requests: usize,

        /// Server port
        #[arg(short, long, default_value = "58080")]
        port: u16,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("throttlecrab=info".parse()?),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::PerfTest {
            threads,
            requests,
            port,
        } => {
            perf_test::run_performance_test(threads, requests, port).await?;
        }
    }

    Ok(())
}

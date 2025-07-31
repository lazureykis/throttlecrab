use anyhow::Result;
use clap::{Parser, Subcommand};

mod client_perf_test;
mod client_v2_perf_test;
mod direct_native_test;
mod perf_test_multi_transport;

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

        /// Transport type (http, grpc, msgpack, native)
        #[arg(short = 'T', long, default_value = "http")]
        transport: String,
    },
    /// Run native client performance test
    ClientPerfTest {
        /// Number of threads
        #[arg(short, long, default_value = "20")]
        threads: usize,

        /// Requests per thread
        #[arg(short, long, default_value = "5000")]
        requests: usize,

        /// Server port
        #[arg(short, long, default_value = "58072")]
        port: u16,

        /// Connection pool size
        #[arg(short = 'P', long, default_value = "10")]
        pool_size: usize,
    },
    /// Run direct native test (no connection pool)
    DirectNativeTest {
        /// Number of threads
        #[arg(short, long, default_value = "20")]
        threads: usize,

        /// Requests per thread
        #[arg(short, long, default_value = "5000")]
        requests: usize,

        /// Server port
        #[arg(short, long, default_value = "9090")]
        port: u16,
    },
    /// Run client V2 performance test
    ClientV2PerfTest {
        /// Number of threads
        #[arg(short, long, default_value = "20")]
        threads: usize,

        /// Requests per thread
        #[arg(short, long, default_value = "5000")]
        requests: usize,

        /// Server port
        #[arg(short, long, default_value = "9090")]
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
            transport,
        } => {
            perf_test_multi_transport::run_performance_test(threads, requests, port, &transport).await?;
        }
        Commands::ClientPerfTest {
            threads,
            requests,
            port,
            pool_size,
        } => {
            client_perf_test::run_client_performance_test(threads, requests, port, pool_size)
                .await?;
        }
        Commands::DirectNativeTest {
            threads,
            requests,
            port,
        } => {
            direct_native_test::run_direct_native_test(threads, requests, port).await?;
        }
        Commands::ClientV2PerfTest {
            threads,
            requests,
            port,
        } => {
            client_v2_perf_test::run_client_v2_performance_test(threads, requests, port).await?;
        }
    }

    Ok(())
}

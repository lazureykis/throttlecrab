use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::{Child, Command};
use tokio::time::sleep;

use super::connection_pool::{
    MsgPackConnectionPool as ImprovedMsgPackPool, NativeConnectionPool as ImprovedNativePool,
};
use super::workload::{WorkloadConfig, WorkloadGenerator};

pub struct ServerInstance {
    process: Child,
    transport: Transport,
    port: u16,
}

#[derive(Debug, Clone, Copy)]
pub enum Transport {
    Http,
    Grpc,
    MsgPack,
    Native,
}

impl ServerInstance {
    pub async fn start(transport: Transport, port: u16, store_type: &str) -> Result<Self> {
        let mut cmd = Command::new("cargo");
        cmd.arg("run")
            .arg("--release")
            .arg("--bin")
            .arg("throttlecrab")
            .arg("--")
            .arg(format!("--{}", transport.flag_name()))
            .arg(format!("--{}-port", transport.flag_name()))
            .arg(port.to_string())
            .arg("--store")
            .arg(store_type)
            .arg("--log-level")
            .arg("warn");

        let process = cmd.spawn()?;

        // Wait for server to start
        sleep(Duration::from_secs(2)).await;

        Ok(Self {
            process,
            transport,
            port,
        })
    }

    pub async fn stop(mut self) -> Result<()> {
        self.process.kill().await?;
        Ok(())
    }
}

impl Transport {
    pub fn flag_name(&self) -> &'static str {
        match self {
            Transport::Http => "http",
            Transport::Grpc => "grpc",
            Transport::MsgPack => "msgpack",
            Transport::Native => "native",
        }
    }
}

// HTTP client with connection pooling
pub struct HttpClient {
    client: reqwest::Client,
    url: String,
}

impl HttpClient {
    pub fn new(port: u16) -> Self {
        let client = reqwest::Client::builder()
            .pool_max_idle_per_host(100)
            .pool_idle_timeout(Duration::from_secs(30))
            .build()
            .unwrap();

        Self {
            client,
            url: format!("http://127.0.0.1:{port}/throttle"),
        }
    }

    pub async fn test_request(&self, key: String) -> Result<bool> {
        let response = self
            .client
            .post(&self.url)
            .json(&serde_json::json!({
                "key": key,
                "max_burst": 100,
                "count_per_period": 10,
                "period": 60,
                "quantity": 1,
            }))
            .send()
            .await?;

        let json: serde_json::Value = response.json().await?;
        Ok(!json["allowed"].as_bool().unwrap_or(true))
    }
}

pub async fn test_http_transport(port: u16, key: String) -> Result<bool> {
    // For backward compatibility - creates a new client each time
    let client = HttpClient::new(port);
    client.test_request(key).await
}

// gRPC client - cloneable for per-thread usage
#[derive(Clone)]
pub struct GrpcClient {
    client: throttlecrab_server::grpc::rate_limiter_client::RateLimiterClient<
        tonic::transport::Channel,
    >,
}

impl GrpcClient {
    pub async fn new(port: u16) -> Result<Self> {
        use throttlecrab_server::grpc::rate_limiter_client::RateLimiterClient;

        let client = RateLimiterClient::connect(format!("http://127.0.0.1:{port}")).await?;
        Ok(Self { client })
    }

    pub async fn test_request(&mut self, key: String) -> Result<bool> {
        use throttlecrab_server::grpc::ThrottleRequest;

        let request = tonic::Request::new(ThrottleRequest {
            key: key.clone(),
            max_burst: 100,
            count_per_period: 10,
            period: 60,
            quantity: 1,
            timestamp: 0, // Server will use current time
        });

        let response = self.client.throttle(request).await?;
        Ok(!response.into_inner().allowed)
    }
}

pub async fn test_grpc_transport(port: u16, key: String) -> Result<bool> {
    // For backward compatibility - creates a new client each time
    let mut client = GrpcClient::new(port).await?;
    client.test_request(key).await
}

// Use the improved connection pool
pub type MsgPackConnectionPool = ImprovedMsgPackPool;

pub async fn test_msgpack_transport(port: u16, key: String) -> Result<bool> {
    // For backward compatibility - uses connection pool
    let pool = ImprovedMsgPackPool::new(port, 10);
    pool.test_request(key).await
}

// Use the improved connection pool
pub type NativeClient = ImprovedNativePool;

pub async fn test_native_transport(port: u16, key: String) -> Result<bool> {
    // For backward compatibility - uses connection pool
    let client = ImprovedNativePool::new(port, 10);
    client.test_request(key).await
}

pub async fn run_transport_benchmark(
    transport: Transport,
    store_type: &str,
    workload_config: WorkloadConfig,
) -> Result<()> {
    println!(
        "\n=== Testing {} transport with {} store ===",
        transport.flag_name(),
        store_type
    );

    let port = match transport {
        Transport::Http => 18080,
        Transport::Grpc => 18070,
        Transport::MsgPack => 18071,
        Transport::Native => 18072,
    };

    // Start server
    let server = ServerInstance::start(transport, port, store_type).await?;

    // Create workload generator
    let generator = WorkloadGenerator::new(workload_config.clone());
    let stats = generator.stats();

    // Run workload with pooled clients
    let start = std::time::Instant::now();

    match transport {
        Transport::Http => {
            let client = Arc::new(HttpClient::new(port));
            generator
                .run(move |key| {
                    let client = client.clone();
                    async move { client.test_request(key).await }
                })
                .await?;
        }
        Transport::Grpc => {
            let client = GrpcClient::new(port).await?;
            generator
                .run(move |key| {
                    let mut client = client.clone();
                    async move { client.test_request(key).await }
                })
                .await?;
        }
        Transport::MsgPack => {
            let pool = Arc::new(MsgPackConnectionPool::new(port, 50));
            generator
                .run(move |key| {
                    let pool = pool.clone();
                    async move { pool.test_request(key).await }
                })
                .await?;
        }
        Transport::Native => {
            let client = Arc::new(NativeClient::new(port, 50));
            generator
                .run(move |key| {
                    let client = client.clone();
                    async move { client.test_request(key).await }
                })
                .await?;
        }
    }

    let duration = start.elapsed();

    // Print results
    let summary = stats.get_summary();
    summary.print_summary(duration);

    // Stop server
    server.stop().await?;

    Ok(())
}

#[cfg(test)]
mod tests {

    #[tokio::test]
    async fn test_all_transports() -> Result<()> {
        let workload = WorkloadConfig {
            pattern: WorkloadPattern::Steady { rps: 1000 },
            duration: Duration::from_secs(5),
            key_space_size: 100,
            key_pattern: KeyPattern::Random { pool_size: 100 },
        };

        // Test each transport
        for transport in [
            Transport::Http,
            Transport::Grpc,
            Transport::MsgPack,
            Transport::Native,
        ] {
            run_transport_benchmark(transport, "periodic", workload.clone()).await?;
        }

        Ok(())
    }
}

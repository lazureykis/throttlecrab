use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::{Child, Command};
use tokio::time::sleep;
use tokio::sync::Mutex;

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
            url: format!("http://127.0.0.1:{}/throttle", port),
        }
    }
    
    pub async fn test_request(&self, key: String) -> Result<bool> {
        let response = self.client
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

// gRPC client with connection reuse
pub struct GrpcClient {
    client: throttlecrab_server::grpc::rate_limiter_client::RateLimiterClient<tonic::transport::Channel>,
}

impl GrpcClient {
    pub async fn new(port: u16) -> Result<Self> {
        use throttlecrab_server::grpc::rate_limiter_client::RateLimiterClient;
        
        let client = RateLimiterClient::connect(format!("http://127.0.0.1:{}", port)).await?;
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

// Connection pool for MessagePack
pub struct MsgPackConnectionPool {
    connections: Arc<Mutex<Vec<tokio::net::TcpStream>>>,
    addr: String,
    max_connections: usize,
}

impl MsgPackConnectionPool {
    pub fn new(port: u16, max_connections: usize) -> Self {
        Self {
            connections: Arc::new(Mutex::new(Vec::with_capacity(max_connections))),
            addr: format!("127.0.0.1:{}", port),
            max_connections,
        }
    }
    
    async fn get_connection(&self) -> Result<tokio::net::TcpStream> {
        let mut pool = self.connections.lock().await;
        
        if let Some(conn) = pool.pop() {
            Ok(conn)
        } else {
            tokio::net::TcpStream::connect(&self.addr).await.map_err(Into::into)
        }
    }
    
    async fn return_connection(&self, conn: tokio::net::TcpStream) {
        let mut pool = self.connections.lock().await;
        if pool.len() < self.max_connections {
            pool.push(conn);
        }
        // Otherwise, drop the connection
    }
    
    pub async fn test_request(&self, key: String) -> Result<bool> {
        use rmp_serde::{Deserializer, Serializer};
        use serde::{Deserialize, Serialize};
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        
        #[derive(Serialize)]
        struct Request {
            cmd: u8,
            key: String,
            burst: i64,
            rate: i64,
            period: i64,
            quantity: i64,
            timestamp: i64,
        }

        #[derive(Deserialize)]
        struct Response {
            ok: bool,
            allowed: u8,
            #[allow(dead_code)]
            limit: i64,
            #[allow(dead_code)]
            remaining: i64,
            #[allow(dead_code)]
            retry_after: i64,
            #[allow(dead_code)]
            reset_after: i64,
        }
        
        let mut stream = self.get_connection().await?;

        let request = Request {
            cmd: 1, // throttle command
            key,
            burst: 100,
            rate: 10,
            period: 60,
            quantity: 1,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos() as i64,
        };

        // Serialize request
        let mut buf = Vec::new();
        request.serialize(&mut Serializer::new(&mut buf))?;

        // Write length prefix and data
        let len = buf.len() as u32;
        let result = async {
            stream.write_all(&len.to_be_bytes()).await?;
            stream.write_all(&buf).await?;
            stream.flush().await?;

            // Read response length
            let mut len_buf = [0u8; 4];
            stream.read_exact(&mut len_buf).await?;
            let response_len = u32::from_be_bytes(len_buf) as usize;

            // Read response data
            let mut response_buf = vec![0u8; response_len];
            stream.read_exact(&mut response_buf).await?;

            // Deserialize response
            let response: Response = Deserialize::deserialize(&mut Deserializer::new(&response_buf[..]))?;

            // Check if request was rate limited (allowed == 0 means rate limited)
            Ok::<bool, anyhow::Error>(response.ok && response.allowed == 0)
        }.await;
        
        // Return connection to pool
        self.return_connection(stream).await;
        
        result
    }
}

pub async fn test_msgpack_transport(port: u16, key: String) -> Result<bool> {
    // For backward compatibility - uses connection pool
    let pool = MsgPackConnectionPool::new(port, 10);
    pool.test_request(key).await
}

// Native transport client with connection pooling
pub struct NativeClient {
    connections: Arc<Mutex<Vec<tokio::net::TcpStream>>>,
    addr: String,
    max_connections: usize,
}

impl NativeClient {
    pub fn new(port: u16, max_connections: usize) -> Self {
        Self {
            connections: Arc::new(Mutex::new(Vec::with_capacity(max_connections))),
            addr: format!("127.0.0.1:{}", port),
            max_connections,
        }
    }
    
    async fn get_connection(&self) -> Result<tokio::net::TcpStream> {
        let mut pool = self.connections.lock().await;
        
        if let Some(conn) = pool.pop() {
            Ok(conn)
        } else {
            tokio::net::TcpStream::connect(&self.addr).await.map_err(Into::into)
        }
    }
    
    async fn return_connection(&self, conn: tokio::net::TcpStream) {
        let mut pool = self.connections.lock().await;
        if pool.len() < self.max_connections {
            pool.push(conn);
        }
    }
    
    pub async fn test_request(&self, key: String) -> Result<bool> {
        use bytes::{BufMut, BytesMut};
        use std::time::{SystemTime, UNIX_EPOCH};
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        
        let mut stream = self.get_connection().await?;

        // Native protocol request format
        let mut request = BytesMut::new();
        request.put_u8(1); // cmd: 1 for rate limit check
        request.put_u8(key.len() as u8); // key_len
        request.put_i64_le(100); // burst (capacity)
        request.put_i64_le(10); // rate (quantum)  
        request.put_i64_le(60_000_000_000); // period in nanoseconds (60 seconds)
        request.put_i64_le(1); // quantity

        // timestamp in nanoseconds since UNIX epoch
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as i64;
        request.put_i64_le(now);

        // key
        request.put_slice(key.as_bytes());

        let result = async {
            // Send request
            stream.write_all(&request).await?;
            stream.flush().await?;

            // Read response (33 bytes fixed)
            let mut response = vec![0u8; 33];
            stream.read_exact(&mut response).await?;

            // Parse response
            let ok = response[0];
            let allowed = response[1];

            if ok == 0 {
                return Err(anyhow::anyhow!("Server returned error"));
            }

            // allowed == 0 means rate limited (not allowed)
            Ok::<bool, anyhow::Error>(allowed == 0)
        }.await;
        
        // Return connection to pool
        if result.is_ok() {
            self.return_connection(stream).await;
        }
        
        result
    }
}

pub async fn test_native_transport(port: u16, key: String) -> Result<bool> {
    // For backward compatibility - uses connection pool
    let client = NativeClient::new(port, 10);
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
    let server = ServerInstance::start(transport.clone(), port, store_type).await?;

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
            let client = Arc::new(Mutex::new(GrpcClient::new(port).await?));
            generator
                .run(move |key| {
                    let client = client.clone();
                    async move { 
                        let mut c = client.lock().await;
                        c.test_request(key).await 
                    }
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
    use super::*;

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

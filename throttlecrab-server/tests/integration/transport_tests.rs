use std::time::Duration;
use anyhow::Result;
use tokio::process::{Command, Child};
use tokio::time::sleep;

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
        
        Ok(Self { process, transport, port })
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

pub async fn test_http_transport(port: u16, key: String) -> Result<bool> {
    let client = reqwest::Client::new();
    
    let response = client
        .post(format!("http://127.0.0.1:{}/check_rate", port))
        .json(&serde_json::json!({
            "key": key,
            "capacity": 100,
            "quantum": 10,
            "window_seconds": 60,
        }))
        .send()
        .await?;
    
    let json: serde_json::Value = response.json().await?;
    Ok(json["limited"].as_bool().unwrap_or(false))
}

pub async fn test_grpc_transport(port: u16, key: String) -> Result<bool> {
    use throttlecrab_server::grpc::rate_limiter_client::RateLimiterClient;
    use throttlecrab_server::grpc::ThrottleRequest;
    
    let mut client = RateLimiterClient::connect(format!("http://127.0.0.1:{}", port)).await?;
    
    let request = tonic::Request::new(ThrottleRequest {
        key: key.clone(),
        max_burst: 100,
        count_per_period: 10,
        period: 60,
        quantity: 1,
        timestamp: 0, // Server will use current time
    });
    
    let response = client.throttle(request).await?;
    Ok(!response.into_inner().allowed)
}

pub async fn test_msgpack_transport(port: u16, key: String) -> Result<bool> {
    use tokio::net::TcpStream;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use rmp_serde::{Serializer, Deserializer};
    use serde::{Serialize, Deserialize};
    
    #[derive(Serialize)]
    struct Request {
        key: String,
        capacity: i64,
        quantum: i64,
        window_seconds: u64,
    }
    
    #[derive(Deserialize)]
    struct Response {
        limited: bool,
        #[allow(dead_code)]
        remaining: i64,
        #[allow(dead_code)]
        retry_after: Option<u64>,
        #[allow(dead_code)]
        reset_after: Option<u64>,
    }
    
    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port)).await?;
    
    let request = Request {
        key,
        capacity: 100,
        quantum: 10,
        window_seconds: 60,
    };
    
    // Serialize request
    let mut buf = Vec::new();
    request.serialize(&mut Serializer::new(&mut buf))?;
    
    // Write length prefix and data
    let len = buf.len() as u32;
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
    
    Ok(response.limited)
}

pub async fn test_native_transport(port: u16, key: String) -> Result<bool> {
    use tokio::net::TcpStream;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use bytes::{BytesMut, BufMut};
    use std::time::{SystemTime, UNIX_EPOCH};
    
    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port)).await?;
    
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
    Ok(allowed == 0)
}

pub async fn run_transport_benchmark(
    transport: Transport,
    store_type: &str,
    workload_config: WorkloadConfig,
) -> Result<()> {
    println!("\n=== Testing {} transport with {} store ===", 
        transport.flag_name(), store_type);
    
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
    
    // Run workload
    let start = std::time::Instant::now();
    
    match transport {
        Transport::Http => {
            generator.run(move |key| {
                let port = port;
                async move { test_http_transport(port, key).await }
            }).await?;
        },
        Transport::Grpc => {
            generator.run(move |key| {
                let port = port;
                async move { test_grpc_transport(port, key).await }
            }).await?;
        },
        Transport::MsgPack => {
            generator.run(move |key| {
                let port = port;
                async move { test_msgpack_transport(port, key).await }
            }).await?;
        },
        Transport::Native => {
            generator.run(move |key| {
                let port = port;
                async move { test_native_transport(port, key).await }
            }).await?;
        },
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
        for transport in [Transport::Http, Transport::Grpc, Transport::MsgPack, Transport::Native] {
            run_transport_benchmark(transport, "periodic", workload.clone()).await?;
        }

        Ok(())
    }
}
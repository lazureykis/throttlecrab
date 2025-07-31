use anyhow::Result;
use bytes::{BufMut, BytesMut};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[tokio::main]
async fn main() -> Result<()> {
    println!("Native protocol simple test\n");

    // Connect to server
    let mut stream = TcpStream::connect("127.0.0.1:58072").await?;
    stream.set_nodelay(true)?;
    println!("Connected to server");

    // Send a few test requests
    for i in 0..5 {
        let key = format!("test_key_{}", i);
        println!("\nSending request {} with key: {}", i, key);

        // Build request
        let mut request = BytesMut::new();
        request.put_u8(1); // cmd
        request.put_u8(key.len() as u8); // key_len
        request.put_i64_le(100); // burst
        request.put_i64_le(10); // rate
        request.put_i64_le(60_000_000_000); // period in nanoseconds
        request.put_i64_le(1); // quantity

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as i64;
        request.put_i64_le(now);
        request.put_slice(key.as_bytes());

        println!("Request size: {} bytes", request.len());
        println!("Request hex: {:02x?}", &request[0..20]); // First 20 bytes

        // Send request
        stream.write_all(&request).await?;
        stream.flush().await?;
        println!("Request sent");

        // Read response
        let mut response = [0u8; 34];
        match stream.read_exact(&mut response).await {
            Ok(_) => {
                println!("Response received");
                println!("Response hex: {:02x?}", &response[0..10]); // First 10 bytes
                
                let ok = response[0];
                let allowed = response[1];
                let limit = i64::from_le_bytes(response[2..10].try_into().unwrap());
                let remaining = i64::from_le_bytes(response[10..18].try_into().unwrap());
                let retry_after = i64::from_le_bytes(response[18..26].try_into().unwrap());
                // Note: We only have 33 bytes total, so reset_after ends at byte 33
                // Byte 33 would be index 32, but we can't access it with ..33
                // This is likely the bug - trying to read past the buffer!
                
                println!("  ok: {}", ok);
                println!("  allowed: {}", allowed);
                println!("  limit: {}", limit);
                println!("  remaining: {}", remaining);
                println!("  retry_after: {}", retry_after);
                
                // Let's also check if we really read 33 bytes
                println!("  Full response ({} bytes): {:02x?}", response.len(), &response);
            }
            Err(e) => {
                println!("Error reading response: {}", e);
                break;
            }
        }

        // Small delay between requests
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    println!("\nTest completed");
    Ok(())
}
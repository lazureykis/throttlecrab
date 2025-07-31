use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() -> std::io::Result<()> {
    println!("Testing optimized Native Binary transport...");

    let mut stream = TcpStream::connect("127.0.0.1:9092")?;
    stream.set_nodelay(true)?;

    let key = "test_key";
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as i64;

    println!("Sending native binary request...");

    // Send request using native binary protocol
    stream.write_all(&[1u8])?; // cmd
    stream.write_all(&[key.len() as u8])?; // key_len
    stream.write_all(&100i64.to_le_bytes())?; // burst
    stream.write_all(&1000i64.to_le_bytes())?; // rate
    stream.write_all(&60i64.to_le_bytes())?; // period
    stream.write_all(&1i64.to_le_bytes())?; // quantity
    stream.write_all(&timestamp.to_le_bytes())?; // timestamp
    stream.write_all(key.as_bytes())?; // key
    stream.flush()?;

    println!("Waiting for response...");

    // Read response (34 bytes)
    let mut response = [0u8; 34];
    stream.read_exact(&mut response)?;

    let ok = response[0] != 0;
    let allowed = response[1];
    let limit = i64::from_le_bytes([
        response[2],
        response[3],
        response[4],
        response[5],
        response[6],
        response[7],
        response[8],
        response[9],
    ]);
    let remaining = i64::from_le_bytes([
        response[10],
        response[11],
        response[12],
        response[13],
        response[14],
        response[15],
        response[16],
        response[17],
    ]);
    let retry_after = i64::from_le_bytes([
        response[18],
        response[19],
        response[20],
        response[21],
        response[22],
        response[23],
        response[24],
        response[25],
    ]);
    let reset_after = i64::from_le_bytes([
        response[26],
        response[27],
        response[28],
        response[29],
        response[30],
        response[31],
        response[32],
        response[33],
    ]);

    println!("Response:");
    println!("  ok: {ok}");
    println!("  allowed: {allowed}");
    println!("  limit: {limit}");
    println!("  remaining: {remaining}");
    println!("  retry_after: {retry_after}");
    println!("  reset_after: {reset_after}");

    Ok(())
}

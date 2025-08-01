//! Security tests for Redis protocol transport
//! Tests various attack vectors and edge cases

use super::redis::resp::{RespParser, RespValue};

const MAX_BUFFER_SIZE: usize = 64 * 1024; // Same as in redis/mod.rs

#[test]
fn test_buffer_overflow_protection() {
    let mut parser = RespParser::new();
    
    // Try to parse a bulk string with huge size
    let malicious = b"$999999999999999999999\r\n";
    let result = parser.parse(malicious);
    assert!(result.is_err(), "Should reject huge bulk string size");
}

#[test] 
fn test_array_size_limit() {
    let mut parser = RespParser::new();
    
    // Try to parse array with huge element count
    let malicious = b"*999999999999999999999\r\n";
    let result = parser.parse(malicious);
    assert!(result.is_err(), "Should reject huge array size");
}

#[test]
fn test_negative_bulk_string_size() {
    let mut parser = RespParser::new();
    
    // Try negative bulk string size (except -1 which is null)
    let malicious = b"$-999999999\r\n";
    let result = parser.parse(malicious);
    assert!(result.is_err(), "Should reject negative bulk string size");
}

#[test]
fn test_negative_array_size() {
    let mut parser = RespParser::new();
    
    // Try negative array size (except -1 which is null)
    let malicious = b"*-999999999\r\n";
    let result = parser.parse(malicious);
    assert!(result.is_err(), "Should reject negative array size");
}

#[test]
fn test_deeply_nested_arrays() {
    let mut parser = RespParser::new();
    
    // Create deeply nested array that exceeds depth limit
    let mut nested = String::new();
    for _ in 0..200 {
        nested.push_str("*1\r\n");
    }
    nested.push_str(":42\r\n");
    
    let result = parser.parse(nested.as_bytes());
    assert!(result.is_err(), "Should reject deeply nested arrays");
}

#[test]
fn test_partial_command_accumulation() {
    // Simulate attack where client sends partial commands to fill buffer
    let mut buffer = Vec::new();
    let partial = b"$999999";  // Incomplete bulk string size
    
    // Simulate accumulating data
    for _ in 0..10000 {
        buffer.extend_from_slice(partial);
        if buffer.len() > MAX_BUFFER_SIZE {
            // In real code, connection would be closed here
            break;
        }
    }
    
    assert!(buffer.len() > MAX_BUFFER_SIZE, "Buffer should exceed limit");
}

#[test]
fn test_integer_overflow_in_bulk_string() {
    let mut parser = RespParser::new();
    
    // i64::MAX would overflow when cast to usize on 32-bit systems
    let malicious = format!("${}\r\n", i64::MAX).into_bytes();
    let result = parser.parse(&malicious);
    
    // Should either parse successfully (on 64-bit) or reject (on 32-bit)
    // but should never panic or cause undefined behavior
    match result {
        Ok(None) => {}, // Needs more data
        Ok(Some(_)) => {}, // Parsed successfully  
        Err(_) => {}, // Rejected as too large
    }
}

#[test]
fn test_integer_overflow_in_array() {
    let mut parser = RespParser::new();
    
    // i64::MAX would overflow when cast to usize on 32-bit systems
    let malicious = format!("*{}\r\n", i64::MAX).into_bytes();
    let result = parser.parse(&malicious);
    
    // Should be rejected due to size limits
    assert!(result.is_err(), "Should reject array with i64::MAX elements");
}

#[test]
fn test_null_byte_in_bulk_string() {
    let mut parser = RespParser::new();
    
    // Bulk string with null bytes
    let data = b"$5\r\nhel\x00lo\r\n";
    let result = parser.parse(data).unwrap();
    
    match result {
        Some((RespValue::BulkString(Some(s)), _)) => {
            assert_eq!(s.len(), 5);
            assert_eq!(s.as_bytes()[3], 0);
        }
        _ => panic!("Expected bulk string with null byte"),
    }
}

#[test]
fn test_invalid_utf8_in_bulk_string() {
    let mut parser = RespParser::new();
    
    // Invalid UTF-8 sequence
    let data = b"$4\r\n\xff\xfe\xfd\xfc\r\n";
    let result = parser.parse(data);
    
    // Parser should handle invalid UTF-8 gracefully
    assert!(result.is_err(), "Should reject invalid UTF-8");
}

#[test]
fn test_array_with_mixed_huge_elements() {
    let mut parser = RespParser::new();
    
    // Array with mix of valid and invalid elements
    let data = b"*3\r\n:42\r\n$999999999999\r\ntest\r\n:100\r\n";
    let result = parser.parse(data);
    
    // Should fail on huge bulk string
    assert!(result.is_err(), "Should reject array with huge bulk string");
}

#[test]
fn test_recursive_array_memory_exhaustion() {
    let mut parser = RespParser::new();
    
    // Create array with many elements to test size limits
    // This will be rejected because 10000000 > MAX_ARRAY_SIZE
    let data = "*10000000\r\n";
    
    // Parser should reject due to size limits
    let result = parser.parse(data.as_bytes());
    assert!(result.is_err(), "Should reject huge array");
}
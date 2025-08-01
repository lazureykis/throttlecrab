//! RESP (Redis Serialization Protocol) implementation
//!
//! This module provides parsing and serialization for RESP protocol data types.

use anyhow::{Result, bail};
use std::str;

const MAX_BULK_STRING_SIZE: i64 = 512 * 1024 * 1024; // 512MB max
const MAX_ARRAY_SIZE: i64 = 1024 * 1024; // 1M elements max
const MAX_ARRAY_DEPTH: usize = 128; // Max nesting depth

/// RESP value types
#[derive(Debug, Clone, PartialEq)]
pub enum RespValue {
    /// Simple string: +OK\r\n
    SimpleString(String),
    /// Error: -ERR message\r\n
    Error(String),
    /// Integer: :42\r\n
    Integer(i64),
    /// Bulk string: $6\r\nfoobar\r\n or $-1\r\n (null)
    BulkString(Option<String>),
    /// Array: *2\r\n$3\r\nfoo\r\n$3\r\nbar\r\n
    Array(Vec<RespValue>),
}

/// RESP protocol parser
pub struct RespParser {
    depth: usize,
}

impl RespParser {
    pub fn new() -> Self {
        Self { depth: 0 }
    }

    /// Parse a RESP value from bytes
    /// Returns Some((value, bytes_consumed)) if a complete value is found
    /// Returns None if more data is needed
    pub fn parse(&mut self, data: &[u8]) -> Result<Option<(RespValue, usize)>> {
        if data.is_empty() {
            return Ok(None);
        }

        match data[0] {
            b'+' => self.parse_simple_string(data),
            b'-' => self.parse_error(data),
            b':' => self.parse_integer(data),
            b'$' => self.parse_bulk_string(data),
            b'*' => self.parse_array(data),
            _ => bail!("Invalid RESP type marker: {}", data[0] as char),
        }
    }

    fn parse_simple_string(&self, data: &[u8]) -> Result<Option<(RespValue, usize)>> {
        if let Some((line, consumed)) = self.read_line(data) {
            let s = str::from_utf8(&line[1..])?.to_string();
            Ok(Some((RespValue::SimpleString(s), consumed)))
        } else {
            Ok(None)
        }
    }

    fn parse_error(&self, data: &[u8]) -> Result<Option<(RespValue, usize)>> {
        if let Some((line, consumed)) = self.read_line(data) {
            let s = str::from_utf8(&line[1..])?.to_string();
            Ok(Some((RespValue::Error(s), consumed)))
        } else {
            Ok(None)
        }
    }

    fn parse_integer(&self, data: &[u8]) -> Result<Option<(RespValue, usize)>> {
        if let Some((line, consumed)) = self.read_line(data) {
            let s = str::from_utf8(&line[1..])?;
            let n: i64 = s.parse()?;
            Ok(Some((RespValue::Integer(n), consumed)))
        } else {
            Ok(None)
        }
    }

    fn parse_bulk_string(&self, data: &[u8]) -> Result<Option<(RespValue, usize)>> {
        let (length_line, mut consumed) = match self.read_line(data) {
            Some(v) => v,
            None => return Ok(None),
        };

        let length_str = str::from_utf8(&length_line[1..])?;
        let length: i64 = length_str.parse()?;

        if length == -1 {
            // Null bulk string
            return Ok(Some((RespValue::BulkString(None), consumed)));
        }

        // Prevent integer overflow and enforce size limits
        if !(0..=MAX_BULK_STRING_SIZE).contains(&length) {
            bail!("Invalid bulk string length: {}", length);
        }

        let length = length as usize;

        // Check if we have enough data for the string + CRLF
        if data.len() < consumed + length + 2 {
            return Ok(None);
        }

        let string_data = &data[consumed..consumed + length];
        let s = str::from_utf8(string_data)?.to_string();

        consumed += length + 2; // String + CRLF

        Ok(Some((RespValue::BulkString(Some(s)), consumed)))
    }

    fn parse_array(&mut self, data: &[u8]) -> Result<Option<(RespValue, usize)>> {
        // Check recursion depth
        if self.depth >= MAX_ARRAY_DEPTH {
            bail!("Maximum array nesting depth exceeded");
        }

        let (count_line, mut consumed) = match self.read_line(data) {
            Some(v) => v,
            None => return Ok(None),
        };

        let count_str = str::from_utf8(&count_line[1..])?;
        let count: i64 = count_str.parse()?;

        if count == -1 {
            // Null array
            return Ok(Some((RespValue::Array(vec![]), consumed)));
        }

        // Prevent integer overflow and enforce size limits
        if !(0..=MAX_ARRAY_SIZE).contains(&count) {
            bail!("Invalid array size: {}", count);
        }

        let count = count as usize;
        let mut elements = Vec::with_capacity(count);

        // Increment depth for recursive parsing
        self.depth += 1;

        for _ in 0..count {
            match self.parse(&data[consumed..])? {
                Some((value, element_consumed)) => {
                    elements.push(value);
                    consumed += element_consumed;
                }
                None => {
                    self.depth -= 1;
                    return Ok(None); // Need more data
                }
            }
        }

        // Decrement depth after parsing
        self.depth -= 1;

        Ok(Some((RespValue::Array(elements), consumed)))
    }

    /// Read a line terminated by CRLF
    /// Returns Some((line_without_crlf, total_bytes_consumed)) or None if incomplete
    fn read_line(&self, data: &[u8]) -> Option<(Vec<u8>, usize)> {
        for i in 0..data.len().saturating_sub(1) {
            if data[i] == b'\r' && data[i + 1] == b'\n' {
                let line = data[..i].to_vec();
                return Some((line, i + 2));
            }
        }
        None
    }
}

impl Default for RespParser {
    fn default() -> Self {
        Self::new()
    }
}

/// RESP protocol serializer
pub struct RespSerializer;

impl RespSerializer {
    /// Serialize a RESP value to bytes
    pub fn serialize(value: &RespValue) -> Vec<u8> {
        match value {
            RespValue::SimpleString(s) => {
                let mut buf = vec![b'+'];
                buf.extend_from_slice(s.as_bytes());
                buf.extend_from_slice(b"\r\n");
                buf
            }
            RespValue::Error(s) => {
                let mut buf = vec![b'-'];
                buf.extend_from_slice(s.as_bytes());
                buf.extend_from_slice(b"\r\n");
                buf
            }
            RespValue::Integer(n) => {
                let mut buf = vec![b':'];
                buf.extend_from_slice(n.to_string().as_bytes());
                buf.extend_from_slice(b"\r\n");
                buf
            }
            RespValue::BulkString(s) => match s {
                Some(s) => {
                    let mut buf = vec![b'$'];
                    buf.extend_from_slice(s.len().to_string().as_bytes());
                    buf.extend_from_slice(b"\r\n");
                    buf.extend_from_slice(s.as_bytes());
                    buf.extend_from_slice(b"\r\n");
                    buf
                }
                None => b"$-1\r\n".to_vec(),
            },
            RespValue::Array(elements) => {
                let mut buf = vec![b'*'];
                buf.extend_from_slice(elements.len().to_string().as_bytes());
                buf.extend_from_slice(b"\r\n");
                for element in elements {
                    buf.extend_from_slice(&Self::serialize(element));
                }
                buf
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_string() {
        let mut parser = RespParser::new();
        let data = b"+OK\r\n";
        let result = parser.parse(data).unwrap();
        assert_eq!(result, Some((RespValue::SimpleString("OK".to_string()), 5)));
    }

    #[test]
    fn test_parse_error() {
        let mut parser = RespParser::new();
        let data = b"-ERR unknown command\r\n";
        let result = parser.parse(data).unwrap();
        assert_eq!(
            result,
            Some((RespValue::Error("ERR unknown command".to_string()), 22))
        );
    }

    #[test]
    fn test_parse_integer() {
        let mut parser = RespParser::new();
        let data = b":42\r\n";
        let result = parser.parse(data).unwrap();
        assert_eq!(result, Some((RespValue::Integer(42), 5)));
    }

    #[test]
    fn test_parse_bulk_string() {
        let mut parser = RespParser::new();
        let data = b"$6\r\nfoobar\r\n";
        let result = parser.parse(data).unwrap();
        assert_eq!(
            result,
            Some((RespValue::BulkString(Some("foobar".to_string())), 12))
        );
    }

    #[test]
    fn test_parse_null_bulk_string() {
        let mut parser = RespParser::new();
        let data = b"$-1\r\n";
        let result = parser.parse(data).unwrap();
        assert_eq!(result, Some((RespValue::BulkString(None), 5)));
    }

    #[test]
    fn test_parse_array() {
        let mut parser = RespParser::new();
        let data = b"*2\r\n$3\r\nfoo\r\n$3\r\nbar\r\n";
        let result = parser.parse(data).unwrap();
        assert_eq!(
            result,
            Some((
                RespValue::Array(vec![
                    RespValue::BulkString(Some("foo".to_string())),
                    RespValue::BulkString(Some("bar".to_string())),
                ]),
                22
            ))
        );
    }

    #[test]
    fn test_serialize_simple_string() {
        let value = RespValue::SimpleString("OK".to_string());
        let serialized = RespSerializer::serialize(&value);
        assert_eq!(serialized, b"+OK\r\n");
    }

    #[test]
    fn test_serialize_array() {
        let value = RespValue::Array(vec![
            RespValue::BulkString(Some("foo".to_string())),
            RespValue::Integer(42),
        ]);
        let serialized = RespSerializer::serialize(&value);
        assert_eq!(serialized, b"*2\r\n$3\r\nfoo\r\n:42\r\n");
    }
}

use anyhow::{Result, anyhow};
use clap::Parser;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub transports: TransportConfig,
    pub store: StoreConfig,
    pub buffer_size: usize,
    pub log_level: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TransportConfig {
    pub http: Option<HttpConfig>,
    pub grpc: Option<GrpcConfig>,
    pub msgpack: Option<MsgPackConfig>,
    pub native: Option<NativeConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HttpConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GrpcConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MsgPackConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NativeConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StoreConfig {
    pub store_type: StoreType,
    pub capacity: usize,
    // Store-specific parameters
    pub cleanup_interval: u64,    // For periodic store (seconds)
    pub cleanup_probability: u64, // For probabilistic store (1 in N)
    pub min_interval: u64,        // For adaptive store (seconds)
    pub max_interval: u64,        // For adaptive store (seconds)
    pub max_operations: usize,    // For adaptive store
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum StoreType {
    Periodic,
    Probabilistic,
    Adaptive,
}

impl std::str::FromStr for StoreType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "periodic" => Ok(StoreType::Periodic),
            "probabilistic" => Ok(StoreType::Probabilistic),
            "adaptive" => Ok(StoreType::Adaptive),
            _ => Err(anyhow!(
                "Invalid store type: {}. Valid options are: periodic, probabilistic, adaptive",
                s
            )),
        }
    }
}

#[derive(Parser, Debug)]
#[command(
    name = "throttlecrab-server",
    about = "High-performance rate limiting server",
    long_about = "A high-performance rate limiting server with multiple protocol support.\n\nAt least one transport must be specified."
)]
pub struct Args {
    // HTTP Transport
    #[arg(long, help = "Enable HTTP transport", env = "THROTTLECRAB_HTTP")]
    pub http: bool,
    #[arg(
        long,
        value_name = "HOST",
        help = "HTTP host",
        default_value = "127.0.0.1",
        env = "THROTTLECRAB_HTTP_HOST"
    )]
    pub http_host: String,
    #[arg(
        long,
        value_name = "PORT",
        help = "HTTP port",
        default_value_t = 8080,
        env = "THROTTLECRAB_HTTP_PORT"
    )]
    pub http_port: u16,

    // gRPC Transport
    #[arg(long, help = "Enable gRPC transport", env = "THROTTLECRAB_GRPC")]
    pub grpc: bool,
    #[arg(
        long,
        value_name = "HOST",
        help = "gRPC host",
        default_value = "127.0.0.1",
        env = "THROTTLECRAB_GRPC_HOST"
    )]
    pub grpc_host: String,
    #[arg(
        long,
        value_name = "PORT",
        help = "gRPC port",
        default_value_t = 8070,
        env = "THROTTLECRAB_GRPC_PORT"
    )]
    pub grpc_port: u16,

    // MessagePack Transport
    #[arg(
        long,
        help = "Enable MessagePack transport",
        env = "THROTTLECRAB_MSGPACK"
    )]
    pub msgpack: bool,
    #[arg(
        long,
        value_name = "HOST",
        help = "MessagePack host",
        default_value = "127.0.0.1",
        env = "THROTTLECRAB_MSGPACK_HOST"
    )]
    pub msgpack_host: String,
    #[arg(
        long,
        value_name = "PORT",
        help = "MessagePack port",
        default_value_t = 8071,
        env = "THROTTLECRAB_MSGPACK_PORT"
    )]
    pub msgpack_port: u16,

    // Native Transport
    #[arg(long, help = "Enable Native transport", env = "THROTTLECRAB_NATIVE")]
    pub native: bool,
    #[arg(
        long,
        value_name = "HOST",
        help = "Native host",
        default_value = "127.0.0.1",
        env = "THROTTLECRAB_NATIVE_HOST"
    )]
    pub native_host: String,
    #[arg(
        long,
        value_name = "PORT",
        help = "Native port",
        default_value_t = 8072,
        env = "THROTTLECRAB_NATIVE_PORT"
    )]
    pub native_port: u16,

    // Store Configuration
    #[arg(
        long,
        value_name = "TYPE",
        help = "Store type: periodic, probabilistic, adaptive",
        default_value = "periodic",
        env = "THROTTLECRAB_STORE"
    )]
    pub store: StoreType,
    #[arg(
        long,
        value_name = "SIZE",
        help = "Initial store capacity",
        default_value_t = 100_000,
        env = "THROTTLECRAB_STORE_CAPACITY"
    )]
    pub store_capacity: usize,

    // Store-specific options
    #[arg(
        long,
        value_name = "SECS",
        help = "Cleanup interval for periodic store (seconds)",
        default_value_t = 300,
        env = "THROTTLECRAB_STORE_CLEANUP_INTERVAL"
    )]
    pub store_cleanup_interval: u64,
    #[arg(
        long,
        value_name = "N",
        help = "Cleanup probability for probabilistic store (1 in N)",
        default_value_t = 10_000,
        env = "THROTTLECRAB_STORE_CLEANUP_PROBABILITY"
    )]
    pub store_cleanup_probability: u64,
    #[arg(
        long,
        value_name = "SECS",
        help = "Minimum cleanup interval for adaptive store (seconds)",
        default_value_t = 5,
        env = "THROTTLECRAB_STORE_MIN_INTERVAL"
    )]
    pub store_min_interval: u64,
    #[arg(
        long,
        value_name = "SECS",
        help = "Maximum cleanup interval for adaptive store (seconds)",
        default_value_t = 300,
        env = "THROTTLECRAB_STORE_MAX_INTERVAL"
    )]
    pub store_max_interval: u64,
    #[arg(
        long,
        value_name = "N",
        help = "Maximum operations before cleanup for adaptive store",
        default_value_t = 1_000_000,
        env = "THROTTLECRAB_STORE_MAX_OPERATIONS"
    )]
    pub store_max_operations: usize,

    // General options
    #[arg(
        long,
        value_name = "SIZE",
        help = "Channel buffer size",
        default_value_t = 100_000,
        env = "THROTTLECRAB_BUFFER_SIZE"
    )]
    pub buffer_size: usize,
    #[arg(
        long,
        value_name = "LEVEL",
        help = "Log level: error, warn, info, debug, trace",
        default_value = "info",
        env = "THROTTLECRAB_LOG_LEVEL"
    )]
    pub log_level: String,
}

impl Config {
    pub fn from_env_and_args() -> Result<Self> {
        // Clap automatically handles environment variables with the precedence:
        // 1. CLI arguments (highest priority)
        // 2. Environment variables
        // 3. Default values (lowest priority)
        let args = Args::parse();

        // Build config from parsed args (which already include env vars)
        let mut config = Config {
            transports: TransportConfig {
                http: None,
                grpc: None,
                msgpack: None,
                native: None,
            },
            store: StoreConfig {
                store_type: args.store,
                capacity: args.store_capacity,
                cleanup_interval: args.store_cleanup_interval,
                cleanup_probability: args.store_cleanup_probability,
                min_interval: args.store_min_interval,
                max_interval: args.store_max_interval,
                max_operations: args.store_max_operations,
            },
            buffer_size: args.buffer_size,
            log_level: args.log_level,
        };

        // Configure transports based on parsed args
        if args.http {
            config.transports.http = Some(HttpConfig {
                host: args.http_host,
                port: args.http_port,
            });
        }

        if args.grpc {
            config.transports.grpc = Some(GrpcConfig {
                host: args.grpc_host,
                port: args.grpc_port,
            });
        }

        if args.msgpack {
            config.transports.msgpack = Some(MsgPackConfig {
                host: args.msgpack_host,
                port: args.msgpack_port,
            });
        }

        if args.native {
            config.transports.native = Some(NativeConfig {
                host: args.native_host,
                port: args.native_port,
            });
        }

        // Validate that at least one transport is enabled
        if !config.has_any_transport() {
            return Err(anyhow!(
                "At least one transport must be specified.\n\n\
                Available transports:\n  \
                --http       Enable HTTP transport\n  \
                --grpc       Enable gRPC transport\n  \
                --msgpack    Enable MessagePack transport\n  \
                --native     Enable Native transport\n\n\
                Example:\n  \
                throttlecrab-server --http --http-port 7070\n  \
                throttlecrab-server --msgpack --grpc\n\n\
                For more information, try '--help'"
            ));
        }

        Ok(config)
    }

    pub fn has_any_transport(&self) -> bool {
        self.transports.http.is_some()
            || self.transports.grpc.is_some()
            || self.transports.msgpack.is_some()
            || self.transports.native.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_store_type_from_str() {
        assert_eq!(
            StoreType::from_str("periodic").unwrap(),
            StoreType::Periodic
        );
        assert_eq!(
            StoreType::from_str("PERIODIC").unwrap(),
            StoreType::Periodic
        );
        assert_eq!(
            StoreType::from_str("probabilistic").unwrap(),
            StoreType::Probabilistic
        );
        assert_eq!(
            StoreType::from_str("adaptive").unwrap(),
            StoreType::Adaptive
        );
        assert!(StoreType::from_str("invalid").is_err());
    }
}

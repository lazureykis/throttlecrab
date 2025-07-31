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
    long_about = "A high-performance rate limiting server with multiple protocol support.\n\nAt least one transport must be specified.\n\nEnvironment variables with THROTTLECRAB_ prefix are supported. CLI arguments take precedence over environment variables."
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

    // Utility options
    #[arg(
        long,
        help = "List all environment variables and exit",
        action = clap::ArgAction::SetTrue
    )]
    pub list_env_vars: bool,
}

impl Config {
    pub fn from_env_and_args() -> Result<Self> {
        // Clap automatically handles environment variables with the precedence:
        // 1. CLI arguments (highest priority)
        // 2. Environment variables
        // 3. Default values (lowest priority)
        let args = Args::parse();

        // Handle --list-env-vars
        if args.list_env_vars {
            Self::print_env_vars();
            std::process::exit(0);
        }

        // Build config from parsed args (which already include env vars)
        let mut config = Config {
            transports: TransportConfig {
                http: None,
                grpc: None,
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

        if args.native {
            config.transports.native = Some(NativeConfig {
                host: args.native_host,
                port: args.native_port,
            });
        }

        // Validate configuration
        config.validate()?;

        Ok(config)
    }

    pub fn has_any_transport(&self) -> bool {
        self.transports.http.is_some()
            || self.transports.grpc.is_some()
            || self.transports.native.is_some()
    }

    fn validate(&self) -> Result<()> {
        if !self.has_any_transport() {
            return Err(anyhow!(
                "At least one transport must be specified.\n\n\
                Available transports:\n  \
                --http       Enable HTTP transport\n  \
                --grpc       Enable gRPC transport\n  \
                --native     Enable Native transport\n\n\
                Example:\n  \
                throttlecrab-server --http --http-port 7070\n  \
                throttlecrab-server --grpc --native\n\n\
                For more information, try '--help'"
            ));
        }

        // Additional validation could be added here in the future
        // e.g., validate port ranges, check for conflicting options, etc.

        Ok(())
    }

    fn print_env_vars() {
        println!("ThrottleCrab Environment Variables");
        println!("==================================");
        println!();
        println!("All environment variables use the THROTTLECRAB_ prefix.");
        println!("CLI arguments take precedence over environment variables.");
        println!();

        println!("Transport Configuration:");
        println!("  THROTTLECRAB_HTTP=true|false          Enable HTTP transport");
        println!("  THROTTLECRAB_HTTP_HOST=<host>         HTTP host [default: 127.0.0.1]");
        println!("  THROTTLECRAB_HTTP_PORT=<port>         HTTP port [default: 8080]");
        println!();
        println!("  THROTTLECRAB_GRPC=true|false          Enable gRPC transport");
        println!("  THROTTLECRAB_GRPC_HOST=<host>         gRPC host [default: 127.0.0.1]");
        println!("  THROTTLECRAB_GRPC_PORT=<port>         gRPC port [default: 8070]");
        println!();
        println!("  THROTTLECRAB_NATIVE=true|false        Enable Native transport");
        println!("  THROTTLECRAB_NATIVE_HOST=<host>       Native host [default: 127.0.0.1]");
        println!("  THROTTLECRAB_NATIVE_PORT=<port>       Native port [default: 8072]");
        println!();

        println!("Store Configuration:");
        println!(
            "  THROTTLECRAB_STORE=<type>             Store type: periodic, probabilistic, adaptive [default: periodic]"
        );
        println!(
            "  THROTTLECRAB_STORE_CAPACITY=<size>    Initial store capacity [default: 100000]"
        );
        println!();
        println!("  For periodic store:");
        println!(
            "    THROTTLECRAB_STORE_CLEANUP_INTERVAL=<secs>   Cleanup interval in seconds [default: 300]"
        );
        println!();
        println!("  For probabilistic store:");
        println!(
            "    THROTTLECRAB_STORE_CLEANUP_PROBABILITY=<n>   Cleanup probability (1 in N) [default: 10000]"
        );
        println!();
        println!("  For adaptive store:");
        println!(
            "    THROTTLECRAB_STORE_MIN_INTERVAL=<secs>       Minimum cleanup interval [default: 5]"
        );
        println!(
            "    THROTTLECRAB_STORE_MAX_INTERVAL=<secs>       Maximum cleanup interval [default: 300]"
        );
        println!(
            "    THROTTLECRAB_STORE_MAX_OPERATIONS=<n>        Max operations before cleanup [default: 1000000]"
        );
        println!();

        println!("General Configuration:");
        println!("  THROTTLECRAB_BUFFER_SIZE=<size>       Channel buffer size [default: 100000]");
        println!(
            "  THROTTLECRAB_LOG_LEVEL=<level>        Log level: error, warn, info, debug, trace [default: info]"
        );
        println!();

        println!("Examples:");
        println!("  # Enable HTTP transport on port 8080");
        println!("  export THROTTLECRAB_HTTP=true");
        println!("  export THROTTLECRAB_HTTP_PORT=8080");
        println!();
        println!("  # Use adaptive store with custom settings");
        println!("  export THROTTLECRAB_STORE=adaptive");
        println!("  export THROTTLECRAB_STORE_MIN_INTERVAL=10");
        println!("  export THROTTLECRAB_STORE_MAX_INTERVAL=600");
        println!();
        println!("  # Run server (CLI args override env vars)");
        println!("  throttlecrab-server --http-port 9090  # Will use port 9090, not 8080");
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

    #[test]
    fn test_config_validation_no_transport() {
        let config = Config {
            transports: TransportConfig {
                http: None,
                grpc: None,
                native: None,
            },
            store: StoreConfig {
                store_type: StoreType::Periodic,
                capacity: 100_000,
                cleanup_interval: 300,
                cleanup_probability: 10_000,
                min_interval: 5,
                max_interval: 300,
                max_operations: 1_000_000,
            },
            buffer_size: 100_000,
            log_level: "info".to_string(),
        };

        assert!(config.validate().is_err());
        assert!(!config.has_any_transport());
    }

    #[test]
    fn test_config_validation_with_transport() {
        let config = Config {
            transports: TransportConfig {
                http: Some(HttpConfig {
                    host: "127.0.0.1".to_string(),
                    port: 8080,
                }),
                grpc: None,
                native: None,
            },
            store: StoreConfig {
                store_type: StoreType::Periodic,
                capacity: 100_000,
                cleanup_interval: 300,
                cleanup_probability: 10_000,
                min_interval: 5,
                max_interval: 300,
                max_operations: 1_000_000,
            },
            buffer_size: 100_000,
            log_level: "info".to_string(),
        };

        assert!(config.validate().is_ok());
        assert!(config.has_any_transport());
    }

    #[test]
    fn test_config_multiple_transports() {
        let config = Config {
            transports: TransportConfig {
                http: Some(HttpConfig {
                    host: "0.0.0.0".to_string(),
                    port: 8080,
                }),
                grpc: Some(GrpcConfig {
                    host: "0.0.0.0".to_string(),
                    port: 50051,
                }),
                native: None,
            },
            store: StoreConfig {
                store_type: StoreType::Adaptive,
                capacity: 200_000,
                cleanup_interval: 300,
                cleanup_probability: 10_000,
                min_interval: 10,
                max_interval: 600,
                max_operations: 2_000_000,
            },
            buffer_size: 50_000,
            log_level: "debug".to_string(),
        };

        assert!(config.validate().is_ok());
        assert!(config.has_any_transport());
    }
}

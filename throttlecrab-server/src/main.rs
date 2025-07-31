mod actor;
mod config;
mod store;
mod transport;
mod types;

#[cfg(test)]
mod actor_tests;

use anyhow::Result;
use tokio::task::JoinSet;

use crate::config::Config;
use crate::transport::{
    Transport, grpc::GrpcTransport, http::HttpTransport, msgpack::MsgPackTransport,
    native::NativeTransport,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Parse configuration from environment variables and CLI arguments
    let config = Config::from_env_and_args()?;

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(format!("throttlecrab={}", config.log_level).parse()?),
        )
        .init();

    // Create the rate limiter actor with the configured store
    let limiter = store::create_rate_limiter(&config.store, config.buffer_size);

    // Create a set to manage multiple transport tasks
    let mut transport_tasks = JoinSet::new();

    // Start HTTP transport if enabled
    if let Some(http_config) = &config.transports.http {
        let limiter_handle = limiter.clone();
        let host = http_config.host.clone();
        let port = http_config.port;

        transport_tasks.spawn(async move {
            tracing::info!("Starting HTTP transport on {}:{}", host, port);
            let transport = HttpTransport::new(&host, port);
            transport.start(limiter_handle).await
        });
    }

    // Start gRPC transport if enabled
    if let Some(grpc_config) = &config.transports.grpc {
        let limiter_handle = limiter.clone();
        let host = grpc_config.host.clone();
        let port = grpc_config.port;

        transport_tasks.spawn(async move {
            tracing::info!("Starting gRPC transport on {}:{}", host, port);
            let transport = GrpcTransport::new(&host, port);
            transport.start(limiter_handle).await
        });
    }

    // Start MessagePack transport if enabled
    if let Some(msgpack_config) = &config.transports.msgpack {
        let limiter_handle = limiter.clone();
        let host = msgpack_config.host.clone();
        let port = msgpack_config.port;

        transport_tasks.spawn(async move {
            tracing::info!("Starting MessagePack transport on {}:{}", host, port);
            let transport = MsgPackTransport::new(&host, port);
            transport.start(limiter_handle).await
        });
    }

    // Start Native transport if enabled
    if let Some(native_config) = &config.transports.native {
        let limiter_handle = limiter.clone();
        let host = native_config.host.clone();
        let port = native_config.port;

        transport_tasks.spawn(async move {
            tracing::info!("Starting Native transport on {}:{}", host, port);
            let transport = NativeTransport::new(&host, port);
            transport.start(limiter_handle).await
        });
    }

    tracing::info!(
        "ThrottleCrab server started with store type: {:?}",
        config.store.store_type
    );
    tracing::info!(
        "Store capacity: {}, Buffer size: {}",
        config.store.capacity,
        config.buffer_size
    );

    // Wait for all transport tasks to complete (they run indefinitely)
    while let Some(result) = transport_tasks.join_next().await {
        match result {
            Ok(Ok(())) => {
                tracing::info!("Transport task completed successfully");
            }
            Ok(Err(e)) => {
                tracing::error!("Transport task failed: {}", e);
                return Err(e);
            }
            Err(e) => {
                tracing::error!("Transport task panicked: {}", e);
                return Err(anyhow::anyhow!("Transport task panicked"));
            }
        }
    }

    Ok(())
}

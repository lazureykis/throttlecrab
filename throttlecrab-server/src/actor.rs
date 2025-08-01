//! Actor-based rate limiter for shared state management
//!
//! This module implements an actor pattern to ensure thread-safe access to the
//! rate limiter state. All transports communicate with a single actor instance,
//! guaranteeing consistent rate limiting across protocols.
//!
//! # Architecture
//!
//! The actor pattern provides:
//! - **Thread Safety**: Single-threaded access to mutable state
//! - **Async Communication**: Non-blocking message passing via channels
//! - **Protocol Independence**: All transports use the same interface
//!
//! # Example
//!
//! ```ignore
//! // Spawn an actor with an adaptive store
//! let limiter = RateLimiterActor::spawn_adaptive(10000, AdaptiveStore::new());
//!
//! // Use the handle from any transport
//! let response = limiter.throttle(request).await?;
//! ```

use crate::metrics::Metrics;
use crate::types::{ThrottleRequest, ThrottleResponse};
use anyhow::Result;
use std::sync::Arc;
use throttlecrab::{AdaptiveStore, CellError, PeriodicStore, ProbabilisticStore, RateLimiter};
use tokio::sync::{mpsc, oneshot};

/// Message types for the rate limiter actor
///
/// Currently supports throttle requests, but can be extended with
/// additional message types like statistics queries or cache clearing.
pub enum RateLimiterMessage {
    /// Check rate limit for a key
    Throttle {
        /// The rate limit request
        request: ThrottleRequest,
        /// Channel to send the response back
        response_tx: oneshot::Sender<Result<ThrottleResponse>>,
    },
    // Future: Stats, Clear, Shutdown, etc.
}

/// Handle to communicate with the rate limiter actor
///
/// This handle can be cloned and shared across multiple tasks/threads.
/// All operations are async and non-blocking.
#[derive(Clone)]
pub struct RateLimiterHandle {
    tx: mpsc::Sender<RateLimiterMessage>,
    pub metrics: Arc<Metrics>,
}

impl RateLimiterHandle {
    /// Check rate limit for a key
    ///
    /// Sends a throttle request to the actor and waits for the response.
    /// This method is cancel-safe and can be used in select! expressions.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The actor has shut down
    /// - The response channel was dropped
    pub async fn throttle(&self, request: ThrottleRequest) -> Result<ThrottleResponse> {
        let (response_tx, response_rx) = oneshot::channel();

        self.tx
            .send(RateLimiterMessage::Throttle {
                request,
                response_tx,
            })
            .await
            .map_err(|_| anyhow::anyhow!("Rate limiter actor has shut down"))?;

        response_rx
            .await
            .map_err(|_| anyhow::anyhow!("Rate limiter actor dropped response channel"))?
    }
}

/// The rate limiter actor factory
///
/// Provides static methods to spawn rate limiter actors with different store types.
/// Each actor runs in its own Tokio task and processes messages sequentially.
pub struct RateLimiterActor;

impl RateLimiterActor {
    /// Spawn a new rate limiter actor with a periodic store
    ///
    /// # Parameters
    ///
    /// - `buffer_size`: Channel buffer size for backpressure control
    /// - `store`: The periodic store instance to use
    ///
    /// # Returns
    ///
    /// A [`RateLimiterHandle`] for communicating with the actor
    pub fn spawn_periodic(buffer_size: usize, store: PeriodicStore, metrics: Arc<Metrics>) -> RateLimiterHandle {
        let (tx, rx) = mpsc::channel(buffer_size);
        let metrics_clone = Arc::clone(&metrics);

        tokio::spawn(async move {
            let store_type = StoreType::Periodic(RateLimiter::new(store));
            run_actor(rx, store_type, metrics_clone).await;
        });

        RateLimiterHandle { tx, metrics }
    }

    /// Spawn a new rate limiter actor with a probabilistic store
    ///
    /// # Parameters
    ///
    /// - `buffer_size`: Channel buffer size for backpressure control
    /// - `store`: The probabilistic store instance to use
    ///
    /// # Returns
    ///
    /// A [`RateLimiterHandle`] for communicating with the actor
    pub fn spawn_probabilistic(buffer_size: usize, store: ProbabilisticStore, metrics: Arc<Metrics>) -> RateLimiterHandle {
        let (tx, rx) = mpsc::channel(buffer_size);
        let metrics_clone = Arc::clone(&metrics);

        tokio::spawn(async move {
            let store_type = StoreType::Probabilistic(RateLimiter::new(store));
            run_actor(rx, store_type, metrics_clone).await;
        });

        RateLimiterHandle { tx, metrics }
    }

    /// Spawn a new rate limiter actor with an adaptive store
    ///
    /// # Parameters
    ///
    /// - `buffer_size`: Channel buffer size for backpressure control
    /// - `store`: The adaptive store instance to use
    ///
    /// # Returns
    ///
    /// A [`RateLimiterHandle`] for communicating with the actor
    pub fn spawn_adaptive(buffer_size: usize, store: AdaptiveStore, metrics: Arc<Metrics>) -> RateLimiterHandle {
        let (tx, rx) = mpsc::channel(buffer_size);
        let metrics_clone = Arc::clone(&metrics);

        tokio::spawn(async move {
            let store_type = StoreType::Adaptive(RateLimiter::new(store));
            run_actor(rx, store_type, metrics_clone).await;
        });

        RateLimiterHandle { tx, metrics }
    }
}

/// Internal enum to handle different store types
enum StoreType {
    Periodic(RateLimiter<PeriodicStore>),
    Probabilistic(RateLimiter<ProbabilisticStore>),
    Adaptive(RateLimiter<AdaptiveStore>),
}

impl StoreType {
    fn rate_limit(
        &mut self,
        key: &str,
        max_burst: i64,
        count_per_period: i64,
        period: i64,
        quantity: i64,
        timestamp: std::time::SystemTime,
    ) -> Result<(bool, throttlecrab::RateLimitResult), CellError> {
        match self {
            StoreType::Periodic(limiter) => limiter.rate_limit(
                key,
                max_burst,
                count_per_period,
                period,
                quantity,
                timestamp,
            ),
            StoreType::Probabilistic(limiter) => limiter.rate_limit(
                key,
                max_burst,
                count_per_period,
                period,
                quantity,
                timestamp,
            ),
            StoreType::Adaptive(limiter) => limiter.rate_limit(
                key,
                max_burst,
                count_per_period,
                period,
                quantity,
                timestamp,
            ),
        }
    }
}

async fn run_actor(mut rx: mpsc::Receiver<RateLimiterMessage>, mut store_type: StoreType, _metrics: Arc<Metrics>) {
    while let Some(msg) = rx.recv().await {
        match msg {
            RateLimiterMessage::Throttle {
                request,
                response_tx,
            } => {
                let response = handle_throttle(&mut store_type, request);
                // Ignore send errors - receiver may have timed out
                let _ = response_tx.send(response);
            }
        }
    }

    tracing::info!("Rate limiter actor shutting down");
}

fn handle_throttle(
    store_type: &mut StoreType,
    request: ThrottleRequest,
) -> Result<ThrottleResponse> {
    // Check the rate limit
    let (allowed, result) = store_type
        .rate_limit(
            &request.key,
            request.max_burst,
            request.count_per_period,
            request.period,
            request.quantity,
            request.timestamp,
        )
        .map_err(|e| anyhow::anyhow!("Rate limit check failed: {}", e))?;

    Ok(ThrottleResponse::from((allowed, result)))
}

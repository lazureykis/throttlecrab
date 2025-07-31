use crate::types::{ThrottleRequest, ThrottleResponse};
use anyhow::Result;
use throttlecrab::{AdaptiveStore, CellError, PeriodicStore, ProbabilisticStore, RateLimiter};
use tokio::sync::{mpsc, oneshot};

/// Message types for the rate limiter actor
pub enum RateLimiterMessage {
    Throttle {
        request: ThrottleRequest,
        response_tx: oneshot::Sender<Result<ThrottleResponse>>,
    },
    // Future: Stats, Clear, Shutdown, etc.
}

/// Handle to communicate with the rate limiter actor
#[derive(Clone)]
pub struct RateLimiterHandle {
    tx: mpsc::Sender<RateLimiterMessage>,
}

impl RateLimiterHandle {
    /// Check rate limit for a key
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

/// The rate limiter actor
pub struct RateLimiterActor;

impl RateLimiterActor {
    /// Spawn a new rate limiter actor with a periodic store
    pub fn spawn_periodic(buffer_size: usize, store: PeriodicStore) -> RateLimiterHandle {
        let (tx, rx) = mpsc::channel(buffer_size);

        tokio::spawn(async move {
            let store_type = StoreType::Periodic(RateLimiter::new(store));
            run_actor(rx, store_type).await;
        });

        RateLimiterHandle { tx }
    }

    /// Spawn a new rate limiter actor with a probabilistic store
    pub fn spawn_probabilistic(buffer_size: usize, store: ProbabilisticStore) -> RateLimiterHandle {
        let (tx, rx) = mpsc::channel(buffer_size);

        tokio::spawn(async move {
            let store_type = StoreType::Probabilistic(RateLimiter::new(store));
            run_actor(rx, store_type).await;
        });

        RateLimiterHandle { tx }
    }

    /// Spawn a new rate limiter actor with an adaptive store
    pub fn spawn_adaptive(buffer_size: usize, store: AdaptiveStore) -> RateLimiterHandle {
        let (tx, rx) = mpsc::channel(buffer_size);

        tokio::spawn(async move {
            let store_type = StoreType::Adaptive(RateLimiter::new(store));
            run_actor(rx, store_type).await;
        });

        RateLimiterHandle { tx }
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

async fn run_actor(mut rx: mpsc::Receiver<RateLimiterMessage>, mut store_type: StoreType) {
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

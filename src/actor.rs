use crate::types::{ThrottleRequest, ThrottleResponse};
use anyhow::Result;
use throttlecrab::{MemoryStore, RateLimiter};
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

/// The rate limiter actor that runs in a single thread
pub struct RateLimiterActor {
    store: MemoryStore,
    rx: mpsc::Receiver<RateLimiterMessage>,
}

impl RateLimiterActor {
    /// Spawn a new rate limiter actor and return a handle to communicate with it
    pub fn spawn(buffer_size: usize) -> RateLimiterHandle {
        let (tx, rx) = mpsc::channel(buffer_size);

        tokio::spawn(async move {
            let mut actor = RateLimiterActor {
                store: MemoryStore::new(),
                rx,
            };

            actor.run().await;
        });

        RateLimiterHandle { tx }
    }

    /// Main actor loop
    async fn run(&mut self) {
        while let Some(msg) = self.rx.recv().await {
            match msg {
                RateLimiterMessage::Throttle {
                    request,
                    response_tx,
                } => {
                    let response = self.handle_throttle(request);
                    // Ignore send errors - receiver may have timed out
                    let _ = response_tx.send(response);
                }
            }
        }

        tracing::info!("Rate limiter actor shutting down");
    }

    /// Handle a throttle request
    fn handle_throttle(&mut self, request: ThrottleRequest) -> Result<ThrottleResponse> {
        // Create a rate limiter for this request
        let mut limiter = RateLimiter::new_from_parameters(
            &mut self.store,
            request.max_burst,
            request.count_per_period,
            request.period,
        )
        .map_err(|e| anyhow::anyhow!("Invalid rate limit parameters: {}", e))?;

        // Check the rate limit
        let (allowed, result) = limiter
            .rate_limit(&request.key, request.quantity, request.timestamp)
            .map_err(|e| anyhow::anyhow!("Rate limit check failed: {}", e))?;

        Ok(ThrottleResponse::from((allowed, result)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_rate_limiting() {
        let handle = RateLimiterActor::spawn(100);

        // First request should succeed
        let req = ThrottleRequest {
            key: "test".to_string(),
            max_burst: 5,
            count_per_period: 10,
            period: 60,
            quantity: 1,
            timestamp: std::time::SystemTime::now(),
        };

        let resp = handle.throttle(req.clone()).await.unwrap();
        assert!(resp.allowed);
        assert_eq!(resp.limit, 5);
        assert_eq!(resp.remaining, 4);
    }
}

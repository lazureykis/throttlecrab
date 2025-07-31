# ThrottleCrab Architecture

## Actor-Based Single-Threaded Design

### Core Components

```
┌─────────────────────────────────────────────────────────┐
│                    Client Applications                   │
└─────────────────┬───────────────┬───────────────┬───────┘
                  │               │               │
┌─────────────────▼───────────────▼───────────────▼───────┐
│                    Transport Layer                       │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐      │
│  │   HTTP      │ │    gRPC     │ │Native Binary │ ...  │
│  └──────┬──────┘ └──────┬──────┘ └──────┬──────┘      │
└─────────┴───────────────┴───────────────┴──────────────┘
                          │
                          │ Requests via
                          │ mpsc channel
                          ▼
┌─────────────────────────────────────────────────────────┐
│                   Channel (mpsc)                         │
│         Request ────────────────► Response               │
└─────────────────────────┬───────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│              Single-Threaded Actor Loop                  │
│  ┌─────────────────────────────────────────────────┐   │
│  │          Rate Limiter Core (GCRA)                │   │
│  └─────────────────────────────────────────────────┘   │
│  ┌─────────────────────────────────────────────────┐   │
│  │    In-Memory Storage (HashMap, no locks)         │   │
│  └─────────────────────────────────────────────────┘   │
│                                                          │
│  - Processes requests sequentially                       │
│  - No mutexes or locks needed                           │
│  - Predictable latency                                  │
└──────────────────────────────────────────────────────────┘
```

### 1. Actor Message Types (`src/core/mod.rs`)

```rust
pub struct ThrottleRequest {
    pub key: String,
    pub max_burst: u32,
    pub count_per_period: u32,
    pub period_seconds: u32,
    pub quantity: u32,
}

pub struct ThrottleResponse {
    pub allowed: bool,
    pub limit: u32,
    pub remaining: u32,
    pub retry_after_seconds: u32,
    pub reset_after_seconds: u32,
}

// Message sent through channel
pub enum RateLimiterMessage {
    Throttle {
        request: ThrottleRequest,
        response_tx: oneshot::Sender<Result<ThrottleResponse>>,
    },
    // Future: Stats, Clear, etc.
}

// Handle to communicate with the actor
#[derive(Clone)]
pub struct RateLimiterHandle {
    tx: mpsc::Sender<RateLimiterMessage>,
}

impl RateLimiterHandle {
    pub async fn throttle(&self, request: ThrottleRequest) -> Result<ThrottleResponse> {
        let (response_tx, response_rx) = oneshot::channel();
        self.tx.send(RateLimiterMessage::Throttle { request, response_tx }).await?;
        response_rx.await?
    }
}
```

### 2. Actor Implementation (`src/core/actor.rs`)

```rust
pub struct RateLimiterActor {
    storage: HashMap<String, CellState>,
    rx: mpsc::Receiver<RateLimiterMessage>,
}

struct CellState {
    tat: f64,  // Theoretical Arrival Time
    tau: f64,  // Emission interval (period/rate)
}

impl RateLimiterActor {
    pub fn spawn(buffer_size: usize) -> RateLimiterHandle {
        let (tx, rx) = mpsc::channel(buffer_size);
        
        tokio::spawn(async move {
            let mut actor = RateLimiterActor {
                storage: HashMap::new(),
                rx,
            };
            actor.run().await;
        });
        
        RateLimiterHandle { tx }
    }
    
    async fn run(&mut self) {
        while let Some(msg) = self.rx.recv().await {
            match msg {
                RateLimiterMessage::Throttle { request, response_tx } => {
                    let response = self.handle_throttle(request);
                    let _ = response_tx.send(response);
                }
            }
        }
    }
    
    fn handle_throttle(&mut self, request: ThrottleRequest) -> Result<ThrottleResponse> {
        // GCRA algorithm implementation here
        // No locks needed - we own all the data!
    }
}
```

### 3. Transport Trait (`src/transport/mod.rs`)

```rust
#[async_trait]
pub trait Transport {
    async fn start(self, limiter: RateLimiterHandle) -> Result<()>;
    fn name(&self) -> &str;
    fn port(&self) -> u16;
}
```

### 4. HTTP Transport (`src/transport/http.rs`)

```rust
pub struct HttpTransport {
    host: String,
    port: u16,
}

impl HttpTransport {
    async fn handle_request(
        req: Request<Body>,
        limiter: RateLimiterHandle,
    ) -> Result<Response<Body>> {
        match (req.method(), req.uri().path()) {
            (&Method::POST, "/throttle") => {
                // Parse JSON request body
                let body_bytes = hyper::body::to_bytes(req.into_body()).await?;
                let request: HttpRequest = serde_json::from_slice(&body_bytes)?;
                
                // Send to actor via channel
                let response = limiter.throttle(request.into()).await?;
                
                // Return JSON response
                let response_json = serde_json::to_string(&HttpResponse::from(response))?;
                Ok(Response::builder()
                    .status(200)
                    .header("content-type", "application/json")
                    .body(Body::from(response_json))?)
            }
            _ => Ok(Response::builder().status(404).body(Body::empty())?)
        }
    }
}
```

#### JSON Format
```rust
// Request (JSON)
struct HttpRequest {
    key: String,
    max_burst: u32,
    count_per_period: u32,
    period: u32,
    quantity: Option<u32>,  // default: 1
}

// Response (JSON)
struct HttpResponse {
    allowed: bool,
    limit: u32,
    remaining: u32,
    retry_after: u32,
    reset_after: u32,
}
```

### 4. Project Structure

```
src/
├── main.rs              # Binary entry point
├── lib.rs               # Library exports
├── core/
│   ├── mod.rs           # Core traits and types
│   ├── gcra.rs          # GCRA algorithm implementation
│   └── storage/
│       ├── mod.rs       # Storage trait
│       └── memory.rs    # In-memory storage
├── transport/
│   ├── mod.rs           # Transport trait
│   ├── http.rs          # HTTP/JSON
│   ├── grpc.rs          # gRPC
│   └── native.rs        # Native binary protocol
└── config.rs            # Configuration structs
```

### 5. Dependencies

```toml
[dependencies]
# Core
tokio = { version = "1", features = ["full"] }
async-trait = "0.1"
# No dependencies - pure Rust implementation
anyhow = "1"

# HTTP/JSON
hyper = { version = "0.14", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Utilities
tracing = "0.1"
tracing-subscriber = "0.3"
clap = { version = "4", features = ["derive"] }
config = "0.13"

# Storage
dashmap = "5"  # Concurrent HashMap

[dev-dependencies]
criterion = "0.5"
tokio-test = "0.4"
```

### 6. Configuration

```toml
# config.toml
[server]
log_level = "info"

[transports.http]
enabled = true
host = "0.0.0.0"
port = 8080
max_connections = 10000
buffer_size = 8192

[storage]
type = "memory"
cleanup_interval_seconds = 60

[limits]
# Global defaults
default_burst = 10
default_rate = 60
default_period = 60
```

### 7. Example Usage

```rust
// Client example
let mut client = ThrottleCrabClient::connect("127.0.0.1:9090").await?;

let response = client.throttle(
    "user:123",  // key
    15,          // burst
    30,          // rate
    60,          // period
    1,           // quantity
).await?;

if response.allowed {
    println!("Request allowed! {} remaining", response.remaining);
} else {
    println!("Rate limited. Retry after {} seconds", response.retry_after);
}
```

## Performance Targets

- **Latency**: < 100μs p99 (local network)
- **Throughput**: > 100k requests/second per core
- **Memory**: < 100 bytes per active key
- **Connections**: Support 10k+ concurrent TCP connections
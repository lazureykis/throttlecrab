# ThrottleCrab Architecture

## Actor-Based Single-Threaded Design

```
┌─────────────────────────────────────────────────────────┐
│                    Client Applications                  │
└─────────────────┬───────────────┬───────────────┬───────┘
                  │               │               │
┌─────────────────▼───────────────▼───────────────▼───────┐
│                    Transport Layer                      │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐        │
│  │   HTTP      │ │    gRPC     │ │    Redis    │        │
│  └──────┬──────┘ └──────┬──────┘ └──────┬──────┘        │
└─────────┴───────────────┴───────────────┴───────────────┘
                          │
                          │ Requests via
                          │ mpsc channel
                          ▼
┌─────────────────────────────────────────────────────────┐
│                   Channel (mpsc)                        │
│         Request ────────────────► Response              │
└─────────────────────────┬───────────────────────────────┘
                          │
                          ▼
┌────────────────────────────────────────────────────────┐
│              Single-Threaded Actor Loop                │
│  ┌─────────────────────────────────────────────────┐   │
│  │          Rate Limiter Core (GCRA)               │   │
│  └─────────────────────────────────────────────────┘   │
│  ┌─────────────────────────────────────────────────┐   │
│  │    In-Memory Storage (HashMap, no locks)        │   │
│  └─────────────────────────────────────────────────┘   │
│                                                        │
│  - Processes requests sequentially                     │
│  - No mutexes or locks needed                          │
│  - Predictable latency                                 │
└────────────────────────────────────────────────────────┘
```

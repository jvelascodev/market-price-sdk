# Market Price SDK

A high-performance Rust SDK for tracking real-time cryptocurrency market prices for major assets (SOL, BTC, ETH, etc.).

## Features

- **Pyth Hermes (V2) Integration**: Real-time price streaming via Pyth's Hermes SSE API (HTTP/2).
- **Reactive Architecture**: Zero-latency price delivery via `subscribe()` broadcast channel.
- **Automated Failover**: Defaults to **Hermes** with optional fallback to **CoinGecko**.
- **In-Memory Cache**: Sub-microsecond price retrieval from a thread-safe `RwLock` store.
- **Background Polling/Streaming**: Background tasks handle both REST polling and SSE streaming.
- **Resilient**: Built-in exponential backoff, retry logic, and staleness detection.
- **Singleton Design**: Simple `MarketPriceTracker::global()` interface for easy integration.

## ⚠️ Breaking Change: Async Initialization

As of version 0.1.0, the `MarketPriceTracker` initialization has become **asynchronous**.

- **Before**: `let tracker = MarketPriceTracker::global();`
- **Now**: `let tracker = MarketPriceTracker::global().await;`

Existing code must be updated to `.await` the initialization calls.

## Quick Start

### Basic Usage

```rust
use market_price_sdk::{MarketPriceTracker, Asset};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get the global tracker (initializes on first call)
    let tracker = MarketPriceTracker::global().await;

    // Get a price (checks cache first, handles staleness)
    match tracker.get_price(Asset::SOL).await {
        Ok(price) => println!("SOL Price: ${:.2} (via {})", price.price_usd, price.source),
        Err(e) => eprintln!("Error: {}", e),
    }

    Ok(())
}
```

### Reactive Streaming

For low-latency applications, subscribe to real-time updates directly:

```rust
use market_price_sdk::MarketPriceTracker;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tracker = MarketPriceTracker::global().await;
    let mut rx = tracker.subscribe();

    println!("Listening for real-time price updates...");
    while let Ok(price) = rx.recv().await {
        println!("Update: {} -> ${:.2} ({})", price.asset.symbol(), price.price_usd, price.source);
    }

    Ok(())
}
```

## Configuration

The SDK uses zero runtime config files. Behavior is controlled via compile-time constants in `src/constants.rs` and environment variables:

| Variable | Description | Default |
|----------|-------------|---------|
| `MARKET_PRICE_PROVIDER` | Selection: `hermes`, `failover`, `hyperliquid`, or `coingecko` | `hermes` |
| `RUST_LOG` | Logging level (e.g., `info`, `debug`) | `info` |

## Benchmarks

The SDK is optimized for high-frequency trading where decision latency is critical.

| Operation | Latency | Source |
|-----------|---------|--------|
| **Cache Retrieval** | ~850ns - 1.5µs | In-process memory |
| **Stream Update** | Real-time | Pyth Hermes |
| **REST API Refresh** | 350ms - 450ms | External Network |

Run the benchmarks yourself:
```bash
cargo run --example sol_price_benchmark
```
Or try the streaming example:
```bash
cargo run --example hermes_stream
```

## Architecture

The tracker manages a background loop/stream that populates an internal `MarketPriceStore`.

```mermaid
graph TD
    A[MarketPriceTracker::global] --> B[Background Tasks]
    B --> C{Provider Selection}
    C -->|SSE Stream| D[Pyth Hermes (DEFAULT)]
    C -->|REST Polling| E[Hyperliquid]
    C -->|REST Polling| F[CoinGecko (BACKUP)]
    D & E & F --> G[MarketPriceStore]
    H[Your Code] -->|get_price| G
```

## License

MIT

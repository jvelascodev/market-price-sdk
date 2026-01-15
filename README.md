# Market Price SDK

A high-performance Rust SDK for tracking real-time cryptocurrency market prices for major assets (SOL, BTC, ETH, etc.).

## Features

- **Pyth gRPC Integration**: Low-latency, real-time price streaming via Yellowstone Geyser (no REST rate limits).
- **Automated Failover**: Defaults to **Pyth gRPC** with automatic fallback to **CoinGecko**.
- **In-Memory Cache**: Sub-microsecond price retrieval from a thread-safe `RwLock` store.
- **Background Polling/Streaming**: Background tasks handle both REST polling and gRPC streaming.
- **Resilient**: Built-in exponential backoff, retry logic, and staleness detection.
- **Singleton Design**: Simple `MarketPriceTracker::global()` interface for easy integration.

## ⚠️ Breaking Change: Async Initialization

As of version 0.1.0, the `MarketPriceTracker` initialization has become **asynchronous**. This was required to support low-latency gRPC streaming.

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
    // By default, it uses Pyth gRPC as the primary source
    let tracker = MarketPriceTracker::global().await;

    // Get a price (checks cache first, handles staleness)
    match tracker.get_price(Asset::SOL).await {
        Ok(price) => println!("SOL Price: ${:.2} (via {})", price.price_usd, price.source),
        Err(e) => eprintln!("Error: {}", e),
    }

    Ok(())
}
```

## Configuration

The SDK uses zero runtime config files. Behavior is controlled via compile-time constants in `src/constants.rs` and environment variables:

| Variable | Description | Default |
|----------|-------------|---------|
| `MARKET_PRICE_PROVIDER` | Selection: `pyth-grpc`, `failover`, `hyperliquid`, or `coingecko` | `failover` (Pyth + CoinGecko) |
| `PYTH_GRPC_ENDPOINT` | Yellowstone gRPC endpoint | `https://solana-yellowstone-grpc.publicnode.com:443` |
| `PYTH_GRPC_TOKEN` | Optional X-Token for gRPC authentication | `None` |
| `RUST_LOG` | Logging level (e.g., `info`, `debug`) | `info` |

## Benchmarks

The SDK is optimized for high-frequency trading where decision latency is critical.

| Operation | Latency | Source |
|-----------|---------|--------|
| **Cache Retrieval** | ~850ns - 1.5µs | In-process memory |
| **gRPC Update** | < 10ms (Real-time) | Pyth/Solana Network |
| **REST API Refresh** | 350ms - 450ms | External Network |

Run the benchmarks yourself:
```bash
cargo run --example sol_price_benchmark
```
Or try the Pyth-specific example:
```bash
cargo run --example pyth_v2_price_example
```

## Architecture

The tracker manages a background loop/stream that populates an internal `MarketPriceStore`.

```mermaid
graph TD
    A[MarketPriceTracker::global] --> B[Background Tasks]
    B --> C{Provider Selection}
    C -->|gRPC Stream| D[Pyth gRPC (DEFAULT)]
    C -->|REST Polling| E[Hyperliquid]
    C -->|REST Polling| F[CoinGecko (BACKUP)]
    D & E & F --> G[MarketPriceStore]
    H[Your Code] -->|get_price| G
```

## License

MIT

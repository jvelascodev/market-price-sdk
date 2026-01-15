# Issue: Redundant Polling in Streaming Providers (Hermes)

**Date:** 2026-01-15  
**Status:** 📅 PLANNED  
**Severity:** Medium  
**Component:** `market-price-sdk` / `tracker.rs`

---

## Summary

The current architecture of `MarketPriceTracker` is designed for polling-based providers (CoinGecko, Hyperliquid). When using a streaming provider like **Hermes**, the system still maintains multi-layered polling cycles (60s in Tracker, 60s in `SolPriceCache`), which introduces unnecessary latency and redundant memory copying.

---

## Detailed Problem Description

1.  **Tracker Polling**: `MarketPriceTracker` runs a background task that calls `fetch_prices()` every 60 seconds. For Hermes, this method simply reads from the provider's internal cache, resulting in a redundant copy from `HermesProvider` cache to `MarketPriceStore`.
2.  **Latency Lag**: A price update received via SSE in Hermes may wait up to 60 seconds before being synced to the tracker's store, and another 60 seconds before being synced to downstream caches (like `SolPriceCache` in `sol-streamer-normalizer`).
3.  **CPU Waste**: Repeatedly polling a streaming source for data that it already holds in memory is inefficient.

---

## Proposed Solution: Push-Based Architecture

We will refactor the SDK to support a "Push" model for providers that support streaming.

### 1. Provider Trait Updates
The `MarketPriceProvider` trait will be extended:
- `fn is_streaming(&self) -> bool`: Allows the tracker to detect if it should disable polling.
- `async fn start_streaming(&self, store: Arc<MarketPriceStore>, tx: broadcast::Sender<PriceData>)`: Allows the provider to push updates directly into the shared store and broadcast them.

### 2. Tracker Reactive Support
- `MarketPriceTracker` will provide a `subscribe()` method returning a `broadcast::Receiver<PriceData>`.
- The background polling loop will be disabled for streaming providers.
- The provider will be responsible for lifecycle management of the stream.

### 3. Downstream Reactivity
Components like `SolPriceCache` will migrate from `loop { sleep(60s); get_price() }` to `while let Ok(price) = rx.recv().await { ... }`, ensuring near-zero latency for price updates.

---

## Expected Impact
- **Latency**: Reduced from average 30s-60s to <10ms for Pyth price delivery to consumers.
- **Resource Usage**: Eliminated background threads/timers performing redundant polls.
- **Code Quality**: Better separation of concerns between polling and streaming providers.

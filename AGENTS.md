# Market Price SDK - Agent Guidelines

## SDK Overview and Purpose

The Market Price SDK provides real-time cryptocurrency market prices for major assets (SOL, BTC, ETH, stablecoins) from off-chain data providers. It features a robust **Failover Mechanism** that prioritizes **Hyperliquid** for low-latency mid-prices, with a fallback to **CoinGecko** for high availability. Designed for trading applications, it tracks major assets using off-chain APIs while deferring on-chain memecoin prices to specialized token price trackers.

## Key Architectural Patterns for Agents

- **Singleton Pattern**: Global `MarketPriceTracker::global()` instance for application-wide access
- **Background Polling**: Automatic price fetching every 25 seconds with configurable intervals
- **Stale Price Detection**: Automatic rejection of prices older than 5 minutes with configurable thresholds
- **Multi-Asset Support**: Concurrent tracking of multiple assets with thread-safe `Arc<RwLock>` storage
- **Async/Await**: Full async operations with Tokio runtime for non-blocking price retrieval
- **Provider Abstraction**: Pluggable `MarketPriceProvider` trait supporting CoinGecko, Hyperliquid, and mock providers
- **Automated Failover**: `FailoverProvider` chain (Hyperliquid -> CoinGecko) for maximum reliability
- **Health Monitoring**: Built-in health check API for operational monitoring
- **Retry Logic**: Exponential backoff with configurable retry attempts for provider failures

## Configuration Constraints and Defaults

All configuration is compile-time constants in `constants.rs`:

- **Refresh Interval**: 25 seconds (configurable via `REFRESH_INTERVAL_SECS`)
- **Stale Threshold**: 300 seconds (5 minutes, configurable via `STALE_THRESHOLD_SECS`)
- **Request Timeout**: 10 seconds (configurable via `REQUEST_TIMEOUT_SECS`)
- **Max Retry Attempts**: 3 (configurable via `MAX_RETRY_ATTEMPTS`)
- **Enabled Assets**: SOL and BTC by default (configurable via `ENABLED_ASSETS`)
- **Primary Provider**: Hyperliquid (low-latency mid-prices)
- **Backup Provider**: CoinGecko v3 API
- **Selection**: Configurable via `MARKET_PRICE_PROVIDER` environment variable

## Integration Patterns and Best Practices

- **Global Access**: Use `MarketPriceTracker::global().await` for singleton access across agents
- **Error Handling**: Pattern match on `PriceError` variants (NotAvailable, Stale, ProviderFailure)
- **Price Retrieval**: Use `get_price(Asset)` for single assets, `get_all_prices()` for bulk retrieval
- **Health Checks**: Implement periodic `health_check()` monitoring for operational reliability
- **Force Refresh**: Use `refresh_now()` to bypass polling intervals during critical operations
- **Staleness Validation**: Check `is_stale()` before using cached prices in time-sensitive scenarios
- **Provider Failover**: Design agents to handle provider failures gracefully with retry logic

## Performance Considerations

- **Cache Performance**: Sub-millisecond access to cached prices with lock-free reads
- **Network Efficiency**: Single batch API call every 25 seconds for all enabled assets
- **Memory Usage**: ~1KB per asset in in-memory cache with efficient storage
- **Concurrency**: Minimal write contention with `Arc<RwLock>` allowing multiple concurrent readers
- **Latency**: 10-second timeout protection for API calls with exponential backoff
- **Batch Operations**: Prefer `fetch_prices()` over individual `fetch_price()` calls for efficiency

## Error Handling Patterns

- **PriceError Handling**:
  - `NotAvailable`: Wait for background task or call `refresh_now()`
  - `Stale`: Data too old, consider provider issues or refresh manually
  - `ProviderFailure`: Handle network/API issues with retry logic
- **ProviderError Handling**:
  - `NetworkError`: Implement retry with backoff
  - `RateLimitExceeded`: Respect API limits, increase refresh intervals
  - `InvalidResponse`: Log and retry, may indicate API changes
- **Retry Strategy**: Exponential backoff (1s, 2s, 4s...) up to 30 seconds max
- **Graceful Degradation**: Continue operation with stale data when possible, alert on persistent failures

## Testing Approaches Specific to SDK

- **Unit Testing**: Use `MockProvider` for isolated testing with `MarketPriceTracker::with_provider()`
- **Error Scenario Testing**: Set mock errors with `mock_provider.set_error()` to test failure handling
- **Integration Testing**: Test with real APIs using `cargo run --example sol_price_benchmark`
- **Failover Testing**: Verify fallback logic by overriding providers via environment variables
- **Performance Testing**: Validate cache access times (sub-microsecond) vs API latency (hundreds of ms)

## Agent Development Guidelines

- **Initialization**: Initialize global tracker early in agent startup, before trading operations
- **Monitoring**: Implement health checks and staleness monitoring in agent control loops
- **Fallback Logic**: Design agents to operate with stale prices when fresh data is unavailable
- **Resource Management**: Reuse global tracker instance across agent components
- **Logging**: Log price fetch failures and staleness events for operational visibility
- **Configuration**: Use compile-time constants for predictable agent behavior across deployments
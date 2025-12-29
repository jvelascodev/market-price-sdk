//! Global Market Price Tracker service
//!
//! Provides a singleton instance for tracking cryptocurrency market prices.

use crate::{
    constants::{
        ENABLED_ASSETS, INITIAL_BACKOFF_MS, MAX_BACKOFF_MS, MAX_RETRY_ATTEMPTS,
        REFRESH_INTERVAL_SECS,
    },
    error::{PriceError, ProviderError},
    metrics::{MetricsCollector, ProviderMetrics},
    provider::MarketPriceProvider,
    providers::{CoinGeckoProvider, FailoverProvider, HyperliquidProvider},
    store::MarketPriceStore,
    types::{Asset, ComponentHealth, HealthStatus, PriceData},
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::OnceCell;
use tokio::time::sleep;

static GLOBAL_TRACKER: OnceCell<Arc<MarketPriceTracker>> = OnceCell::const_new();

/// Global Market Price Tracker
///
/// Manages fetching and storing cryptocurrency prices from external providers.
/// Uses a singleton pattern for easy access throughout the application.
///
/// # Example
/// ```no_run
/// use market_price_sdk::{MarketPriceTracker, Asset};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let tracker = MarketPriceTracker::global().await;
/// let sol_price = tracker.get_price(Asset::SOL).await?;
/// println!("SOL: ${:.2}", sol_price.price_usd);
/// # Ok(())
/// # }
/// ```
pub struct MarketPriceTracker {
    store: Arc<MarketPriceStore>,
    provider: Arc<dyn MarketPriceProvider>,
    metrics: Arc<MetricsCollector>,
}

impl Default for MarketPriceTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl MarketPriceTracker {
    /// Returns the global singleton instance
    ///
    /// On first call, this initializes the tracker and starts the background
    /// polling task. Subsequent calls return the same instance.
    pub async fn global() -> Arc<Self> {
        GLOBAL_TRACKER
            .get_or_init(|| async {
                let tracker = Self::new();
                tracker.start_background_task();
                Arc::new(tracker)
            })
            .await
            .clone()
    }

    /// Creates a new market price tracker
    ///
    /// This is primarily for testing. Use `global()` in production code.
    /// By default, it uses the provider specified in the `MARKET_PRICE_PROVIDER`
    /// environment variable ("coingecko" or "hyperliquid"). Defaults to coingecko.
    pub fn new() -> Self {
        let provider_name = std::env::var("MARKET_PRICE_PROVIDER").unwrap_or_else(|_| "failover".to_string());
        
        let provider: Arc<dyn MarketPriceProvider> = match provider_name.to_lowercase().as_str() {
            "hyperliquid" => Arc::new(HyperliquidProvider::default()),
            "coingecko" => Arc::new(CoinGeckoProvider::default()),
            _ => {
                // Default failover: Hyperliquid (primary) -> CoinGecko (backup)
                Arc::new(FailoverProvider::new(vec![
                    Arc::new(HyperliquidProvider::default()),
                    Arc::new(CoinGeckoProvider::default()),
                ]))
            }
        };

        Self::with_provider(provider)
    }

    /// Creates a new market price tracker with a custom provider
    ///
    /// This is primarily for testing with mock providers.
    pub fn with_provider(provider: Arc<dyn MarketPriceProvider>) -> Self {
        let store = Arc::new(MarketPriceStore::new());
        let metrics = Arc::new(MetricsCollector::new(provider.provider_name()));

        Self { store, provider, metrics }
    }

    /// Starts the background polling task
    fn start_background_task(&self) {
        let store = self.store.clone();
        let provider = self.provider.clone();
        let metrics = self.metrics.clone();

        tokio::spawn(async move {
            tracing::info!(
                refresh_interval_secs = REFRESH_INTERVAL_SECS,
                "Starting market price tracker background task"
            );

            loop {
                if let Err(e) = Self::fetch_and_update(&provider, &store, &metrics).await {
                    tracing::warn!(error = %e, "Failed to fetch prices");
                }

                sleep(Duration::from_secs(REFRESH_INTERVAL_SECS)).await;
            }
        });
    }

    /// Fetches prices from provider and updates the store with metrics tracking
    async fn fetch_and_update(
        provider: &Arc<dyn MarketPriceProvider>,
        store: &Arc<MarketPriceStore>,
        metrics: &Arc<MetricsCollector>,
    ) -> Result<(), ProviderError> {
        let mut backoff_ms = INITIAL_BACKOFF_MS;
        let start = Instant::now();

        for attempt in 1..=MAX_RETRY_ATTEMPTS {
            match provider.fetch_prices(ENABLED_ASSETS).await {
                Ok(prices) => {
                    tracing::debug!(
                        count = prices.len(),
                        provider = provider.provider_name(),
                        latency_ms = start.elapsed().as_millis() as u64,
                        "Successfully fetched prices"
                    );
                    store.update_prices(prices).await;
                    metrics.record_request(start.elapsed(), true).await;
                    return Ok(());
                }
                Err(e) => {
                    tracing::warn!(
                        attempt = attempt,
                        max_attempts = MAX_RETRY_ATTEMPTS,
                        error = %e,
                        "Failed to fetch prices, retrying"
                    );

                    if attempt < MAX_RETRY_ATTEMPTS {
                        sleep(Duration::from_millis(backoff_ms)).await;
                        backoff_ms = (backoff_ms * 2).min(MAX_BACKOFF_MS);
                    } else {
                        metrics.record_request(start.elapsed(), false).await;
                        return Err(e);
                    }
                }
            }
        }

        Err(ProviderError::InvalidResponse(
            "Max retries exceeded".to_string(),
        ))
    }

    /// Gets the current price for an asset
    ///
    /// # Arguments
    /// * `asset` - The asset to get the price for
    ///
    /// # Returns
    /// The current price data or an error if not available or stale
    ///
    /// # Example
    /// ```no_run
    /// # use market_price_sdk::{MarketPriceTracker, Asset};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let tracker = MarketPriceTracker::global().await;
    /// let price = tracker.get_price(Asset::SOL).await?;
    /// println!("SOL: ${:.2}", price.price_usd);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_price(&self, asset: Asset) -> Result<PriceData, PriceError> {
        self.store.get_price(asset).await
    }

    /// Gets prices for all tracked assets
    ///
    /// # Returns
    /// HashMap of all assets with their current prices (non-stale only)
    ///
    /// # Example
    /// ```no_run
    /// # use market_price_sdk::{MarketPriceTracker, Asset};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let tracker = MarketPriceTracker::global().await;
    /// let prices = tracker.get_all_prices().await;
    /// for (asset, price) in prices {
    ///     println!("{}: ${:.2}", asset.symbol(), price.price_usd);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_all_prices(&self) -> HashMap<Asset, PriceData> {
        self.store.get_all_prices().await
    }

    /// Checks if price data is available for an asset
    ///
    /// # Arguments
    /// * `asset` - The asset to check
    ///
    /// # Returns
    /// True if price data exists (regardless of staleness)
    pub async fn has_price(&self, asset: Asset) -> bool {
        self.store.has_price(asset).await
    }

    /// Checks if price data is stale for an asset
    ///
    /// # Arguments
    /// * `asset` - The asset to check
    ///
    /// # Returns
    /// True if price data is stale or doesn't exist
    pub async fn is_stale(&self, asset: Asset) -> bool {
        self.store.is_stale(asset).await
    }

    /// Returns the name of the current provider
    pub fn provider_name(&self) -> &str {
        self.provider.provider_name()
    }

    /// Forces an immediate price refresh
    ///
    /// This bypasses the normal polling interval and fetches fresh prices immediately.
    ///
    /// # Returns
    /// Ok if prices were successfully fetched and updated
    pub async fn refresh_now(&self) -> Result<(), ProviderError> {
        Self::fetch_and_update(&self.provider, &self.store, &self.metrics).await
    }

    /// Gets provider metrics including latency percentiles and success rates
    ///
    /// # Returns
    /// ProviderMetrics with p50/p99 latencies and success rate
    ///
    /// # Example
    /// ```no_run
    /// # use market_price_sdk::MarketPriceTracker;
    /// # async fn example() {
    /// let tracker = MarketPriceTracker::global().await;
    /// let metrics = tracker.get_provider_metrics().await;
    /// println!("Provider {}: p50={}ms, p99={}ms, success_rate={:.1}%",
    ///     metrics.provider_name,
    ///     metrics.latency_p50_ms,
    ///     metrics.latency_p99_ms,
    ///     metrics.success_rate * 100.0
    /// );
    /// # }
    /// ```
    pub async fn get_provider_metrics(&self) -> ProviderMetrics {
        self.metrics.get_metrics().await
    }

    /// Perform a health check on the market price tracker
    ///
    /// # Returns
    /// ComponentHealth indicating the status of the tracker and its components
    pub async fn health_check(&self) -> ComponentHealth {
        let mut details = std::collections::HashMap::new();

        // Check if we have any prices available
        let available_prices = self.get_all_prices().await;
        details.insert(
            "available_prices".to_string(),
            serde_json::json!(available_prices.len()),
        );

        // Provider name
        details.insert(
            "provider_name".to_string(),
            serde_json::json!(self.provider_name()),
        );

        // Check for stale prices
        let mut stale_assets = Vec::new();
        for asset in ENABLED_ASSETS.iter() {
            if self.is_stale(*asset).await {
                stale_assets.push(asset.symbol().to_string());
            }
        }
        details.insert("stale_prices".to_string(), serde_json::json!(stale_assets));

        // Determine overall health
        let status = if available_prices.is_empty() {
            HealthStatus::Unhealthy
        } else if !stale_assets.is_empty() {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };

        let message = match status {
            HealthStatus::Healthy => {
                "Market price tracker is operational with fresh data".to_string()
            }
            HealthStatus::Degraded => format!(
                "Market price tracker has {} stale prices",
                stale_assets.len()
            ),
            HealthStatus::Unhealthy => {
                "Market price tracker has no available price data".to_string()
            }
        };

        ComponentHealth {
            name: "market_price_tracker".to_string(),
            status,
            message: Some(message),
            details,
            last_checked: chrono::Utc::now(),
        }
    }
}


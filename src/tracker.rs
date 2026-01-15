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
    providers::{CoinGeckoProvider, HyperliquidProvider},
    store::MarketPriceStore,
    types::{Asset, ComponentHealth, HealthStatus, PriceData},
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, OnceCell};
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
    update_tx: broadcast::Sender<PriceData>,
    shutdown_tx: broadcast::Sender<()>,
}

impl MarketPriceTracker {
    /// Returns the global singleton instance
    ///
    /// On first call, this initializes the tracker and starts the background
    /// polling task. Subsequent calls return the same instance.
    pub async fn global() -> Arc<Self> {
        GLOBAL_TRACKER
            .get_or_init(|| async {
                let tracker = Self::new().await;
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
    /// environment variable ("coingecko" or "hyperliquid"). Defaults to hermes.
    pub async fn new() -> Self {
        let provider_name =
            std::env::var("MARKET_PRICE_PROVIDER").unwrap_or_else(|_| "hermes".to_string());

        let provider: Arc<dyn MarketPriceProvider> = match provider_name.to_lowercase().as_str() {
            "hermes" | "default" => match crate::providers::HermesProvider::new().await {
                Ok(p) => p,
                Err(e) => {
                    tracing::error!(
                        error = %e,
                        provider = "hermes",
                        "Failed to initialize Hermes provider. Falling back to CoinGecko."
                    );
                    Arc::new(CoinGeckoProvider::default())
                }
            },
            "failover" => {
                // Failover: Hermes (primary) -> CoinGecko (backup)
                let primary = match crate::providers::HermesProvider::new().await {
                    Ok(p) => Some(p as Arc<dyn MarketPriceProvider>),
                    Err(_) => None,
                };

                let backup = Arc::new(CoinGeckoProvider::default());

                if let Some(p) = primary {
                    Arc::new(crate::providers::FailoverProvider::new(vec![p, backup]))
                } else {
                    backup
                }
            }
            "hyperliquid" => Arc::new(HyperliquidProvider::default()),
            "coingecko" => Arc::new(CoinGeckoProvider::default()),
            _ => {
                tracing::warn!(
                    provider = %provider_name,
                    "Unknown provider specified. Defaulting to Hermes."
                );
                match crate::providers::HermesProvider::new().await {
                    Ok(p) => p,
                    Err(_) => Arc::new(CoinGeckoProvider::default()),
                }
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
        let (update_tx, _) = broadcast::channel(1000);
        let (shutdown_tx, _) = broadcast::channel(1);

        Self {
            store,
            provider,
            metrics,
            update_tx,
            shutdown_tx,
        }
    }

    /// Subscribes to real-time price updates
    ///
    /// This is the reactive way to consume prices, especially with
    /// streaming providers like Hermes.
    pub fn subscribe(&self) -> broadcast::Receiver<PriceData> {
        self.update_tx.subscribe()
    }

    /// Starts the background polling task
    fn start_background_task(&self) {
        let store = self.store.clone();
        let provider = self.provider.clone();
        let metrics = self.metrics.clone();
        let update_tx = self.update_tx.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        if provider.is_streaming() {
            tracing::info!(
                provider = provider.provider_name(),
                "Starting market price tracker in reactive streaming mode"
            );
            provider.start_streaming(store, update_tx);
            return;
        }

        tokio::spawn(async move {
            tracing::info!(
                refresh_interval_secs = REFRESH_INTERVAL_SECS,
                "Starting market price tracker background task"
            );

            // Initial fetch
            if let Err(e) = Self::fetch_and_update(&provider, &store, &metrics, &update_tx).await {
                tracing::warn!(error = %e, "Initial price fetch failed");
            }

            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        tracing::info!("Market price tracker background task shutting down");
                        break;
                    }
                    _ = sleep(Duration::from_secs(REFRESH_INTERVAL_SECS)) => {
                        if let Err(e) = Self::fetch_and_update(&provider, &store, &metrics, &update_tx).await {
                            tracing::warn!(error = %e, "Failed to fetch prices");
                        }
                    }
                }
            }
        });
    }

    /// Fetches prices from provider and updates the store with metrics tracking
    async fn fetch_and_update(
        provider: &Arc<dyn MarketPriceProvider>,
        store: &Arc<MarketPriceStore>,
        metrics: &Arc<MetricsCollector>,
        update_tx: &broadcast::Sender<PriceData>,
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
                    store.update_prices(prices.clone()).await;

                    // Broadcast updates for reactive consumers
                    for price in prices.values() {
                        let _ = update_tx.send(price.clone());
                    }

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
        match self.store.get_price(asset).await {
            Ok(price) => Ok(price),
            Err(_) => {
                // If not in store, try fetching directly from provider
                // This is especially useful for streaming providers like Pyth gRPC
                self.provider.fetch_price(asset).await.map_err(|e| {
                    PriceError::not_available(&format!(
                        "{} (Provider error: {})",
                        asset.symbol(),
                        e
                    ))
                })
            }
        }
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
        Self::fetch_and_update(&self.provider, &self.store, &self.metrics, &self.update_tx).await
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

    /// Shutdown the market price tracker
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(());
    }
}

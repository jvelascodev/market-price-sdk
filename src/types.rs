//! Types for the market price tracker

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Supported cryptocurrency assets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Asset {
    /// Solana
    SOL,
    /// Bitcoin
    BTC,
    /// Ethereum
    ETH,
    /// USD Coin
    USDC,
    /// Tether
    USDT,
    /// Wrapped Bitcoin
    WBTC,
    /// Wrapped Ethereum
    WETH,
}

impl Asset {
    /// Get the asset symbol
    pub fn symbol(&self) -> &'static str {
        match self {
            Asset::SOL => "SOL",
            Asset::BTC => "BTC",
            Asset::ETH => "ETH",
            Asset::USDC => "USDC",
            Asset::USDT => "USDT",
            Asset::WBTC => "WBTC",
            Asset::WETH => "WETH",
        }
    }

    /// Get the CoinGecko ID for this asset
    pub fn coingecko_id(&self) -> &'static str {
        match self {
            Asset::SOL => "solana",
            Asset::BTC => "bitcoin",
            Asset::ETH => "ethereum",
            Asset::USDC => "usd-coin",
            Asset::USDT => "tether",
            Asset::WBTC => "wrapped-bitcoin",
            Asset::WETH => "weth",
        }
    }

    /// Get the Hyperliquid symbol for this asset
    pub fn hyperliquid_symbol(&self) -> &'static str {
        match self {
            Asset::SOL => "SOL",
            Asset::BTC => "BTC",
            Asset::ETH => "ETH",
            Asset::USDC => "USDC",
            Asset::USDT => "USDT",
            Asset::WBTC => "WBTC",
            Asset::WETH => "WETH",
        }
    }

    /// Get all supported assets
    pub fn all() -> &'static [Asset] {
        &[
            Asset::SOL,
            Asset::BTC,
            Asset::ETH,
            Asset::USDC,
            Asset::USDT,
            Asset::WBTC,
            Asset::WETH,
        ]
    }

    /// Get the stale threshold for this asset in seconds
    ///
    /// Different assets have different freshness requirements:
    /// - High-frequency assets (SOL, ETH): 120 seconds
    /// - Moderate frequency (BTC, WBTC, WETH): 180 seconds
    /// - Stablecoins (USDC, USDT): 300 seconds (price rarely changes)
    pub fn stale_threshold_secs(&self) -> u64 {
        match self {
            // High-frequency trading assets need fresher data
            Asset::SOL | Asset::ETH => 120,
            // Moderate frequency
            Asset::BTC | Asset::WBTC | Asset::WETH => 180,
            // Stablecoins - price is relatively stable
            Asset::USDC | Asset::USDT => 300,
        }
    }
}

/// Price data for an asset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceData {
    /// The asset
    pub asset: Asset,

    /// Price in USD
    pub price_usd: f64,

    /// 24h price change percentage
    pub price_change_24h: Option<f64>,

    /// Last updated timestamp
    pub last_updated: DateTime<Utc>,

    /// Data source
    pub source: String,
}

impl PriceData {
    /// Create new price data
    pub fn new(asset: Asset, price_usd: f64, source: String) -> Self {
        Self {
            asset,
            price_usd,
            price_change_24h: None,
            last_updated: Utc::now(),
            source,
        }
    }

    /// Create new price data with change percentage
    pub fn with_change(
        asset: Asset,
        price_usd: f64,
        price_change_24h: Option<f64>,
        source: String,
    ) -> Self {
        Self {
            asset,
            price_usd,
            price_change_24h,
            last_updated: Utc::now(),
            source,
        }
    }

    /// Check if the price data is stale (older than threshold seconds)
    pub fn is_stale(&self, threshold_seconds: u64) -> bool {
        let now = Utc::now();
        let age = now.signed_duration_since(self.last_updated);
        age.num_seconds() > threshold_seconds as i64
    }

    /// Get the age of the price data in seconds
    pub fn age(&self) -> std::time::Duration {
        let now = Utc::now();
        let duration = now.signed_duration_since(self.last_updated);
        std::time::Duration::from_secs(duration.num_seconds().max(0) as u64)
    }
}

/// Market price events for the unified event system
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MarketPriceEvent {
    /// Price was updated for an asset
    PriceUpdated {
        id: Uuid,
        asset: Asset,
        old_price_usd: Option<f64>,
        new_price_usd: f64,
        price_change_24h: Option<f64>,
        timestamp: DateTime<Utc>,
    },

    /// Price fetch failed
    PriceFetchFailed {
        id: Uuid,
        asset: Asset,
        error_message: String,
        timestamp: DateTime<Utc>,
    },

    /// Provider status changed
    ProviderStatusChanged {
        id: Uuid,
        provider: String,
        status: ProviderStatus,
        timestamp: DateTime<Utc>,
    },
}

impl MarketPriceEvent {
    /// Get the event ID
    pub fn id(&self) -> Uuid {
        match self {
            MarketPriceEvent::PriceUpdated { id, .. } => *id,
            MarketPriceEvent::PriceFetchFailed { id, .. } => *id,
            MarketPriceEvent::ProviderStatusChanged { id, .. } => *id,
        }
    }

    /// Get the event type as string
    pub fn event_type(&self) -> &'static str {
        match self {
            MarketPriceEvent::PriceUpdated { .. } => "PRICE_UPDATED",
            MarketPriceEvent::PriceFetchFailed { .. } => "PRICE_FETCH_FAILED",
            MarketPriceEvent::ProviderStatusChanged { .. } => "PROVIDER_STATUS_CHANGED",
        }
    }
}

impl std::fmt::Display for MarketPriceEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MarketPriceEvent::PriceUpdated {
                asset,
                new_price_usd,
                ..
            } => {
                write!(
                    f,
                    "Price updated: {} = ${:.2}",
                    asset.symbol(),
                    new_price_usd
                )
            }
            MarketPriceEvent::PriceFetchFailed {
                asset,
                error_message,
                ..
            } => {
                write!(
                    f,
                    "Price fetch failed for {}: {}",
                    asset.symbol(),
                    error_message
                )
            }
            MarketPriceEvent::ProviderStatusChanged {
                provider, status, ..
            } => {
                write!(f, "Provider {} status: {:?}", provider, status)
            }
        }
    }
}

/// Provider status
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderStatus {
    /// Provider is healthy
    Healthy,
    /// Provider is experiencing issues
    Degraded,
    /// Provider is unavailable
    Unavailable,
}

/// Overall system health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthStatus {
    /// System is healthy and all components are operational
    Healthy,
    /// System is degraded but still functional
    Degraded,
    /// System is unhealthy and requires attention
    Unhealthy,
}

/// Component health information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    /// Component name
    pub name: String,
    /// Component status
    pub status: HealthStatus,
    /// Optional status message
    pub message: Option<String>,
    /// Component-specific details
    pub details: std::collections::HashMap<String, serde_json::Value>,
    /// Last checked timestamp
    pub last_checked: DateTime<Utc>,
}

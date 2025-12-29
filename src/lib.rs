//! # Global Market Price Tracker SDK
//!
//! Provides real-time market prices for major cryptocurrency assets (SOL, BTC, etc.)
//! from off-chain data providers like CoinGecko.
//!
//! ## Important: This is NOT for on-chain token price tracking
//!
//! This module tracks **major assets** (SOL, BTC, USDC) using **off-chain APIs**.
//! For tracking **on-chain memecoin prices** during trading, see the examples
//! in `/examples/token_price_tracker.rs`.
//!
//! ## Usage
//!
//! The tracker uses a singleton pattern for easy access throughout the application:
//!
//! ```no_run
//! use market_price_sdk::{MarketPriceTracker, Asset};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Get the global tracker instance
//! let tracker = MarketPriceTracker::global().await;
//!
//! // Get a single price
//! let sol_price = tracker.get_price(Asset::SOL).await?;
//! println!("SOL: ${:.2}", sol_price.price_usd);
//!
//! // Get all prices
//! let all_prices = tracker.get_all_prices().await;
//! for (asset, price) in all_prices {
//!     println!("{}: ${:.2}", asset.symbol(), price.price_usd);
//! }
//! # Ok(())
//! # }
//! ```

pub mod constants;
pub mod error;
pub mod metrics;
pub mod provider;
pub mod providers;
pub mod store;
pub mod tracker;
pub mod types;

// Re-export commonly used types
pub use error::{PriceError, ProviderError};
pub use metrics::ProviderMetrics;
pub use tracker::MarketPriceTracker;
pub use types::{
    Asset, ComponentHealth, HealthStatus, MarketPriceEvent, PriceData, ProviderStatus,
};

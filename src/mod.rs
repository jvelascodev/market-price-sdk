//! # Global Market Price Tracker
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
//!
//! ## Configuration
//!
//! The tracker operates with zero runtime configuration. All settings are defined
//! as compile-time constants in the `constants` module:
//!
//! - Refresh interval: 25 seconds
//! - Stale threshold: 300 seconds
//! - Enabled assets: SOL, BTC
//! - Provider: CoinGecko
//!
//! ## Architecture
//!
//! ```text
//! MarketPriceTracker::global()
//!     ↓
//! Background Task (polls every 25s)
//!     ↓
//! MarketPriceProvider (CoinGecko)
//!     ↓
//! MarketPriceStore (in-memory)
//!     ↓
//! Your Code (get_price, get_all_prices)
//! ```
//!
//! ## Error Handling
//!
//! ```no_run
//! use market_price_sdk::{MarketPriceTracker, Asset, PriceError};
//!
//! # async fn example() {
//! let tracker = MarketPriceTracker::global().await;
//!
//! match tracker.get_price(Asset::BTC).await {
//!     Ok(price) => println!("BTC: ${:.2}", price.price_usd),
//!     Err(PriceError::NotAvailable { asset }) => {
//!         println!("Price not available yet for {}", asset)
//!     }
//!     Err(PriceError::Stale { asset, age }) => {
//!         println!("Price for {} is stale (age: {:?})", asset, age)
//!     }
//!     Err(e) => eprintln!("Error: {}", e),
//! }
//! # }
//! ```
//!
//! ## Adding New Providers
//!
//! To add a new price provider (e.g., Binance):
//!
//! 1. Implement the `MarketPriceProvider` trait
//! 2. Add your provider to `src/sdk/market_price/providers/`
//! 3. Update `tracker.rs` to use your provider
//!
//! ## Adding New Assets
//!
//! To track additional assets:
//!
//! 1. Add the asset to the `Asset` enum in `types.rs`
//! 2. Implement the `coingecko_id()` method for the new asset
//! 3. Add the asset to `ENABLED_ASSETS` in `constants.rs`

pub mod constants;
pub mod error;
pub mod provider;
pub mod providers;
pub mod store;
pub mod tracker;
pub mod types;

// Re-export commonly used types
pub use error::{PriceError, ProviderError};
pub use tracker::MarketPriceTracker;
pub use types::{Asset, PriceData};

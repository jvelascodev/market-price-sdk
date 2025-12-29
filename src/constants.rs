//! Constants for the Global Market Price Tracker
//!
//! All configuration for the market price tracker is centralized here.
//! No runtime configuration (config.yml) is used - the system operates
//! transparently with these compile-time constants.

use crate::types::Asset;

/// How often to fetch prices from the provider (in seconds)
pub const REFRESH_INTERVAL_SECS: u64 = 60;

/// How long before price data is considered stale (in seconds)
pub const STALE_THRESHOLD_SECS: u64 = 300;

/// HTTP request timeout when fetching prices (in seconds)
pub const REQUEST_TIMEOUT_SECS: u64 = 10;

/// Maximum number of retry attempts when a provider fails
pub const MAX_RETRY_ATTEMPTS: u32 = 3;

/// Initial backoff delay for retries (in milliseconds)
pub const INITIAL_BACKOFF_MS: u64 = 1000;

/// Maximum backoff delay for retries (in milliseconds)
pub const MAX_BACKOFF_MS: u64 = 30000;

/// Assets to track by default
pub const ENABLED_ASSETS: &[Asset] = &[Asset::SOL, Asset::BTC];

/// CoinGecko API base URL
pub const COINGECKO_API_URL: &str = "https://api.coingecko.com/api/v3";

/// CoinGecko API endpoint for simple price queries
pub const COINGECKO_SIMPLE_PRICE_ENDPOINT: &str = "/simple/price";

/// Hyperliquid API base URL
pub const HYPERLIQUID_API_URL: &str = "https://api.hyperliquid.xyz/info";

/// User agent for HTTP requests
pub const USER_AGENT: &str = "solana-sniper-bot/0.1.0";

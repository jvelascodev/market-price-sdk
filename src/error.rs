//! Error types for the Global Market Price Tracker

use std::time::Duration;
use thiserror::Error;

/// Errors that can occur when fetching prices from a provider
#[derive(Debug, Error)]
pub enum ProviderError {
    /// Network request failed
    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),

    /// Invalid response from provider
    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    /// Rate limit exceeded
    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    /// Asset not supported by this provider
    #[error("Asset not supported: {0}")]
    UnsupportedAsset(String),

    /// Provider API error
    #[error("Provider API error: {0}")]
    ApiError(String),

    /// Timeout waiting for response
    #[error("Request timeout")]
    Timeout,
}

/// Errors that can occur when retrieving price data
#[derive(Debug, Error, Clone)]
pub enum PriceError {
    /// Price data not yet available (never fetched)
    #[error("Price data not available for {asset}")]
    NotAvailable { asset: String },

    /// Price data is too old (stale)
    #[error("Price data for {asset} is stale (age: {age:?})")]
    Stale { asset: String, age: Duration },

    /// All providers failed to fetch price
    #[error("All providers failed: {0}")]
    ProviderFailure(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

impl PriceError {
    /// Creates a NotAvailable error
    pub fn not_available(asset: &str) -> Self {
        Self::NotAvailable {
            asset: asset.to_string(),
        }
    }

    /// Creates a Stale error
    pub fn stale(asset: &str, age: Duration) -> Self {
        Self::Stale {
            asset: asset.to_string(),
            age,
        }
    }

    /// Creates a ProviderFailure error
    pub fn provider_failure(msg: impl Into<String>) -> Self {
        Self::ProviderFailure(msg.into())
    }

    /// Creates an Internal error
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }
}

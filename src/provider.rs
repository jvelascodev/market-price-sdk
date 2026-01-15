//! Provider abstraction for fetching market prices from external APIs

use crate::{
    error::ProviderError,
    store::MarketPriceStore,
    types::{Asset, PriceData},
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;

/// Trait for market price providers
///
/// Implementations can fetch cryptocurrency prices from various sources
/// (CoinGecko, Binance, Jupiter, etc.)
#[async_trait]
pub trait MarketPriceProvider: Send + Sync {
    /// Fetches the current price for a single asset
    ///
    /// # Arguments
    /// * `asset` - The asset to fetch the price for
    ///
    /// # Returns
    /// Price data for the asset or an error if the fetch fails
    async fn fetch_price(&self, asset: Asset) -> Result<PriceData, ProviderError>;

    /// Fetches prices for multiple assets in a single request
    ///
    /// This is more efficient than calling `fetch_price` multiple times.
    ///
    /// # Arguments
    /// * `assets` - Slice of assets to fetch prices for
    ///
    /// # Returns
    /// HashMap of asset to price data, or an error if the fetch fails
    async fn fetch_prices(
        &self,
        assets: &[Asset],
    ) -> Result<HashMap<Asset, PriceData>, ProviderError>;

    /// Returns the name of this provider
    fn provider_name(&self) -> &'static str;

    /// Returns true if this is a streaming provider (e.g. gRPC, SSE)
    fn is_streaming(&self) -> bool {
        false
    }

    /// Starts streaming updates into the provided store and broadcast channel
    fn start_streaming(
        &self,
        _store: Arc<MarketPriceStore>,
        _update_tx: broadcast::Sender<PriceData>,
    ) {
        // Default no-op for non-streaming providers
    }
}

#[cfg(test)]
pub mod mock {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// Mock provider for testing
    pub struct MockProvider {
        responses: Arc<Mutex<HashMap<Asset, Result<PriceData, ProviderError>>>>,
        call_count: Arc<Mutex<usize>>,
    }

    impl Default for MockProvider {
        fn default() -> Self {
            Self::new()
        }
    }

    impl MockProvider {
        pub fn new() -> Self {
            Self {
                responses: Arc::new(Mutex::new(HashMap::new())),
                call_count: Arc::new(Mutex::new(0)),
            }
        }

        pub fn set_price(&self, asset: Asset, price_usd: f64) {
            let price_data = PriceData::new(asset, price_usd, "mock".to_string());
            self.responses.lock().unwrap().insert(asset, Ok(price_data));
        }

        pub fn set_error(&self, asset: Asset, error: ProviderError) {
            self.responses.lock().unwrap().insert(asset, Err(error));
        }

        pub fn call_count(&self) -> usize {
            *self.call_count.lock().unwrap()
        }
    }

    #[async_trait]
    impl MarketPriceProvider for MockProvider {
        async fn fetch_price(&self, asset: Asset) -> Result<PriceData, ProviderError> {
            *self.call_count.lock().unwrap() += 1;
            let responses = self.responses.lock().unwrap();
            match responses.get(&asset) {
                Some(Ok(price)) => Ok(price.clone()),
                Some(Err(err)) => {
                    // Manual "clone" of ProviderError since it doesn't implement Clone
                    match err {
                        ProviderError::NetworkError(e) => Err(ProviderError::ApiError(format!(
                            "Network error (cloned): {}",
                            e
                        ))),
                        ProviderError::InvalidResponse(s) => {
                            Err(ProviderError::InvalidResponse(s.clone()))
                        }
                        ProviderError::RateLimitExceeded => Err(ProviderError::RateLimitExceeded),
                        ProviderError::UnsupportedAsset(s) => {
                            Err(ProviderError::UnsupportedAsset(s.clone()))
                        }
                        ProviderError::ApiError(s) => Err(ProviderError::ApiError(s.clone())),
                        ProviderError::Timeout => Err(ProviderError::Timeout),
                    }
                }
                None => Err(ProviderError::UnsupportedAsset(asset.symbol().to_string())),
            }
        }

        async fn fetch_prices(
            &self,
            assets: &[Asset],
        ) -> Result<HashMap<Asset, PriceData>, ProviderError> {
            *self.call_count.lock().unwrap() += 1;
            let mut result = HashMap::new();
            for asset in assets {
                if let Ok(price) = self.fetch_price(*asset).await {
                    result.insert(*asset, price);
                }
            }
            if result.is_empty() {
                Err(ProviderError::InvalidResponse(
                    "No prices available".to_string(),
                ))
            } else {
                Ok(result)
            }
        }

        fn provider_name(&self) -> &'static str {
            "mock"
        }
    }
}

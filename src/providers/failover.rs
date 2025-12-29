//! Failover price provider implementation

use crate::{
    error::ProviderError,
    provider::MarketPriceProvider,
    types::{Asset, PriceData},
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

/// Price provider that attempts to fetch from multiple providers in order
/// until one succeeds.
pub struct FailoverProvider {
    providers: Vec<Arc<dyn MarketPriceProvider>>,
}

impl FailoverProvider {
    /// Creates a new failover provider with a list of providers
    /// 
    /// The providers are tried in the order they are provided.
    pub fn new(providers: Vec<Arc<dyn MarketPriceProvider>>) -> Self {
        Self { providers }
    }
}

#[async_trait]
impl MarketPriceProvider for FailoverProvider {
    async fn fetch_price(&self, asset: Asset) -> Result<PriceData, ProviderError> {
        let mut last_error = None;

        for provider in &self.providers {
            match provider.fetch_price(asset).await {
                Ok(price) => return Ok(price),
                Err(e) => {
                    log::warn!(
                        "Provider {} failed to fetch price for {}: {}",
                        provider.provider_name(),
                        asset.symbol(),
                        e
                    );
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            ProviderError::InvalidResponse("No providers configured for failover".to_string())
        }))
    }

    async fn fetch_prices(
        &self,
        assets: &[Asset],
    ) -> Result<HashMap<Asset, PriceData>, ProviderError> {
        let mut last_error = None;

        for provider in &self.providers {
            match provider.fetch_prices(assets).await {
                Ok(prices) => return Ok(prices),
                Err(e) => {
                    log::warn!(
                        "Provider {} failed to fetch prices: {}",
                        provider.provider_name(),
                        e
                    );
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            ProviderError::InvalidResponse("No providers configured for failover".to_string())
        }))
    }

    fn provider_name(&self) -> &'static str {
        // We return the name of the first provider as the primary identifier,
        // or "failover" if we want to be explicit.
        "failover"
    }
}

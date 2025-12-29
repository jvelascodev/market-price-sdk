//! Hyperliquid price provider implementation

use crate::{
    constants::{HYPERLIQUID_API_URL, REQUEST_TIMEOUT_SECS, USER_AGENT},
    error::ProviderError,
    provider::MarketPriceProvider,
    types::{Asset, PriceData},
};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Hyperliquid API request for info
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum HyperliquidRequest {
    AllMids,
}

/// Hyperliquid API response for allMids
/// Returns a map of symbol to mid price as string
#[derive(Debug, Deserialize)]
struct AllMidsResponse(HashMap<String, String>);

/// Hyperliquid price provider
pub struct HyperliquidProvider {
    client: Client,
}

impl HyperliquidProvider {
    /// Creates a new Hyperliquid provider
    pub fn new() -> Result<Self, ProviderError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .user_agent(USER_AGENT)
            .build()
            .map_err(ProviderError::NetworkError)?;

        Ok(Self { client })
    }

    /// Parses the Hyperliquid response into price data
    fn parse_response(
        &self,
        response: AllMidsResponse,
        assets: &[Asset],
    ) -> HashMap<Asset, PriceData> {
        let mut result = HashMap::new();

        for asset in assets {
            let symbol = asset.hyperliquid_symbol();
            if let Some(price_str) = response.0.get(symbol) {
                if let Ok(price_usd) = price_str.parse::<f64>() {
                    result.insert(
                        *asset,
                        PriceData::new(*asset, price_usd, self.provider_name().to_string()),
                    );
                }
            }
        }

        result
    }
}

impl Default for HyperliquidProvider {
    fn default() -> Self {
        Self::new().expect("Failed to create Hyperliquid provider")
    }
}

#[async_trait]
impl MarketPriceProvider for HyperliquidProvider {
    async fn fetch_price(&self, asset: Asset) -> Result<PriceData, ProviderError> {
        let prices = self.fetch_prices(&[asset]).await?;
        prices
            .get(&asset)
            .cloned()
            .ok_or_else(|| ProviderError::UnsupportedAsset(asset.symbol().to_string()))
    }

    async fn fetch_prices(
        &self,
        assets: &[Asset],
    ) -> Result<HashMap<Asset, PriceData>, ProviderError> {
        if assets.is_empty() {
            return Ok(HashMap::new());
        }

        log::debug!("Fetching prices from Hyperliquid: {}", HYPERLIQUID_API_URL);

        let request_body = HyperliquidRequest::AllMids;

        let response = self
            .client
            .post(HYPERLIQUID_API_URL)
            .json(&request_body)
            .send()
            .await
            .map_err(ProviderError::NetworkError)?;

        // Check for rate limiting
        if response.status().as_u16() == 429 {
            return Err(ProviderError::RateLimitExceeded);
        }

        // Check for other errors
        if !response.status().is_success() {
            return Err(ProviderError::ApiError(format!(
                "HTTP {}: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        let response_text = response.text().await.map_err(ProviderError::NetworkError)?;

        let mids: AllMidsResponse = serde_json::from_str(&response_text).map_err(|e| {
            ProviderError::InvalidResponse(format!(
                "Failed to parse Hyperliquid response: {}. Response: {}",
                e, response_text
            ))
        })?;

        let prices = self.parse_response(mids, assets);

        if prices.is_empty() {
            return Err(ProviderError::InvalidResponse(
                "No prices returned from Hyperliquid".to_string(),
            ));
        }

        log::debug!(
            "Successfully fetched {} prices from Hyperliquid",
            prices.len()
        );

        Ok(prices)
    }

    fn provider_name(&self) -> &'static str {
        "hyperliquid"
    }
}

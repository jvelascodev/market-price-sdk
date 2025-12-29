//! CoinGecko price provider implementation

use crate::{
    constants::{
        COINGECKO_API_URL, COINGECKO_SIMPLE_PRICE_ENDPOINT, REQUEST_TIMEOUT_SECS, USER_AGENT,
    },
    error::ProviderError,
    provider::MarketPriceProvider,
    types::{Asset, PriceData},
};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;

/// CoinGecko API response for simple price queries
#[derive(Debug, Deserialize)]
struct CoinGeckoResponse {
    #[serde(flatten)]
    prices: HashMap<String, CoinGeckoPriceData>,
}

#[derive(Debug, Deserialize)]
struct CoinGeckoPriceData {
    usd: f64,
}

/// CoinGecko price provider
pub struct CoinGeckoProvider {
    client: Client,
}

impl CoinGeckoProvider {
    /// Creates a new CoinGecko provider
    pub fn new() -> Result<Self, ProviderError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .user_agent(USER_AGENT)
            .build()
            .map_err(ProviderError::NetworkError)?;

        Ok(Self { client })
    }

    /// Builds the CoinGecko API URL for fetching prices
    fn build_url(&self, assets: &[Asset]) -> String {
        let ids = assets
            .iter()
            .map(|a| a.coingecko_id())
            .collect::<Vec<_>>()
            .join(",");

        format!(
            "{}{}?ids={}&vs_currencies=usd",
            COINGECKO_API_URL, COINGECKO_SIMPLE_PRICE_ENDPOINT, ids
        )
    }

    /// Parses the CoinGecko response into price data
    fn parse_response(
        &self,
        response: CoinGeckoResponse,
        assets: &[Asset],
    ) -> HashMap<Asset, PriceData> {
        let mut result = HashMap::new();

        for asset in assets {
            let id = asset.coingecko_id();
            if let Some(price_data) = response.prices.get(id) {
                result.insert(
                    *asset,
                    PriceData::new(*asset, price_data.usd, self.provider_name().to_string()),
                );
            }
        }

        result
    }
}

impl Default for CoinGeckoProvider {
    fn default() -> Self {
        Self::new().expect("Failed to create CoinGecko provider")
    }
}

#[async_trait]
impl MarketPriceProvider for CoinGeckoProvider {
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

        let url = self.build_url(assets);
        log::debug!("Fetching prices from CoinGecko: {}", url);

        let response = self
            .client
            .get(&url)
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

        let coingecko_response: CoinGeckoResponse =
            serde_json::from_str(&response_text).map_err(|e| {
                ProviderError::InvalidResponse(format!(
                    "Failed to parse CoinGecko response: {}. Response: {}",
                    e, response_text
                ))
            })?;

        let prices = self.parse_response(coingecko_response, assets);

        if prices.is_empty() {
            return Err(ProviderError::InvalidResponse(
                "No prices returned from CoinGecko".to_string(),
            ));
        }

        log::debug!(
            "Successfully fetched {} prices from CoinGecko",
            prices.len()
        );

        Ok(prices)
    }

    fn provider_name(&self) -> &'static str {
        "coingecko"
    }
}


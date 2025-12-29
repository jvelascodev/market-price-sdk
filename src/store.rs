//! In-memory price store with broadcast capabilities

use crate::{
    constants::STALE_THRESHOLD_SECS,
    error::PriceError,
    types::{Asset, PriceData},
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Type alias for an individual price slot (optionally contains price data)
type PriceSlot = Arc<RwLock<Option<PriceData>>>;

/// Type alias for the price map (asset -> price slot)
type PriceMap = HashMap<Asset, PriceSlot>;

/// In-memory store for market prices
///
/// Uses tokio watch channels for efficient broadcast-style updates
/// where multiple consumers can subscribe to price changes.
pub struct MarketPriceStore {
    /// Storage for price data per asset
    prices: Arc<RwLock<PriceMap>>,
}

impl MarketPriceStore {
    /// Creates a new market price store
    pub fn new() -> Self {
        Self {
            prices: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Initializes storage for a specific asset
    async fn ensure_asset(&self, asset: Asset) {
        let mut prices = self.prices.write().await;
        prices
            .entry(asset)
            .or_insert_with(|| Arc::new(RwLock::new(None)));
    }

    /// Updates the price for a specific asset
    ///
    /// # Arguments
    /// * `asset` - The asset to update
    /// * `price_data` - The new price data
    pub async fn update_price(&self, asset: Asset, price_data: PriceData) {
        self.ensure_asset(asset).await;

        let prices = self.prices.read().await;
        if let Some(price_slot) = prices.get(&asset) {
            let mut slot = price_slot.write().await;
            *slot = Some(price_data.clone());
            log::debug!(
                "Updated price for {}: ${:.2}",
                asset.symbol(),
                price_data.price_usd
            );
        }
    }

    /// Updates prices for multiple assets
    ///
    /// # Arguments
    /// * `prices` - HashMap of asset to price data
    pub async fn update_prices(&self, prices: HashMap<Asset, PriceData>) {
        for (asset, price_data) in prices {
            self.update_price(asset, price_data).await;
        }
    }

    /// Gets the current price for an asset
    ///
    /// # Arguments
    /// * `asset` - The asset to get the price for
    ///
    /// # Returns
    /// The current price data or an error if not available or stale
    pub async fn get_price(&self, asset: Asset) -> Result<PriceData, PriceError> {
        let prices = self.prices.read().await;
        let price_slot = prices
            .get(&asset)
            .ok_or_else(|| PriceError::not_available(asset.symbol()))?;

        let slot = price_slot.read().await;
        let price_data = slot
            .as_ref()
            .ok_or_else(|| PriceError::not_available(asset.symbol()))?
            .clone();

        // Check if price is stale
        if price_data.is_stale(STALE_THRESHOLD_SECS) {
            let age = price_data.age();
            return Err(PriceError::stale(asset.symbol(), age));
        }

        Ok(price_data)
    }

    /// Gets all available prices
    ///
    /// # Returns
    /// HashMap of all assets with their current prices
    pub async fn get_all_prices(&self) -> HashMap<Asset, PriceData> {
        let mut result = HashMap::new();
        let prices = self.prices.read().await;

        for (asset, price_slot) in prices.iter() {
            let slot = price_slot.read().await;
            if let Some(price_data) = slot.as_ref() {
                // Only include non-stale prices
                if !price_data.is_stale(STALE_THRESHOLD_SECS) {
                    result.insert(*asset, price_data.clone());
                }
            }
        }

        result
    }

    /// Checks if price data exists for an asset
    ///
    /// # Arguments
    /// * `asset` - The asset to check
    ///
    /// # Returns
    /// True if price data exists (regardless of staleness)
    pub async fn has_price(&self, asset: Asset) -> bool {
        let prices = self.prices.read().await;
        if let Some(price_slot) = prices.get(&asset) {
            let slot = price_slot.read().await;
            slot.is_some()
        } else {
            false
        }
    }

    /// Checks if price data is stale for an asset
    ///
    /// # Arguments
    /// * `asset` - The asset to check
    ///
    /// # Returns
    /// True if price data is stale or doesn't exist
    pub async fn is_stale(&self, asset: Asset) -> bool {
        let prices = self.prices.read().await;
        if let Some(price_slot) = prices.get(&asset) {
            let slot = price_slot.read().await;
            if let Some(price_data) = slot.as_ref() {
                price_data.is_stale(STALE_THRESHOLD_SECS)
            } else {
                true
            }
        } else {
            true
        }
    }
}

impl Default for MarketPriceStore {
    fn default() -> Self {
        Self::new()
    }
}


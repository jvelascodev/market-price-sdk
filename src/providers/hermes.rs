use crate::store::MarketPriceStore;
use crate::types::{Asset, PriceData};
use crate::ProviderError;
use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::stream::StreamExt;
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::broadcast;
use tracing::{error, info};

#[derive(Debug, Deserialize)]
struct HermesPriceUpdate {
    id: String,
    price: HermesPrice,
}

#[derive(Debug, Deserialize)]
struct HermesPrice {
    price: String,
    #[allow(dead_code)]
    conf: String,
    expo: i32,
    #[allow(dead_code)]
    publish_time: i64,
}

#[derive(Debug, Deserialize)]
struct HermesMessage {
    parsed: Vec<HermesPriceUpdate>,
}

#[allow(dead_code)]
struct HermesStats {
    total_updates: u64,
    last_update: std::time::Instant,
}

pub struct HermesProvider {
    client: reqwest::Client,
    prices: Arc<RwLock<HashMap<Asset, PriceData>>>,
    #[allow(dead_code)]
    stats: Arc<RwLock<HermesStats>>,
}

impl HermesProvider {
    pub async fn new() -> Result<Arc<Self>, ProviderError> {
        let client = reqwest::Client::new();
        let prices = Arc::new(RwLock::new(HashMap::new()));
        let stats = Arc::new(RwLock::new(HermesStats {
            total_updates: 0,
            last_update: std::time::Instant::now(),
        }));

        let provider = Arc::new(Self {
            client,
            prices,
            stats,
        });

        Ok(provider)
    }

    async fn stream_prices(
        client: Client,
        prices: Arc<RwLock<HashMap<Asset, PriceData>>>,
        global_store: Option<Arc<MarketPriceStore>>,
        update_tx: Option<broadcast::Sender<PriceData>>,
        stats: Arc<RwLock<HermesStats>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Build URL
        let mut url = "https://hermes.pyth.network/v2/updates/price/stream?".to_string();
        let assets = Asset::all();
        let mut asset_map = HashMap::new();

        for asset in assets {
            if let Some(id) = asset.pyth_feed_id() {
                url.push_str(&format!("ids[]={}&", id));
                // Store normalized ID to map back to Asset (Hermes returns 0x prefix usually)
                asset_map.insert(id.to_string(), *asset);
                asset_map.insert(id.replace("0x", ""), *asset); // Handle both formats just in case
            }
        }

        if url.ends_with('&') {
            url.pop();
        }

        println!("DEBUG: Connecting to Hermes URL: {}", url);

        let response = client.get(&url).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await?;
            error!("Hermes returned error: {} - {}", status, text);
            return Err(Box::new(std::io::Error::other(format!(
                "Hermes error: {} - {}",
                status, text
            ))));
        }

        let mut stream = response.bytes_stream().eventsource();

        while let Some(event) = stream.next().await {
            match event {
                Ok(event) => {
                    // tracing::trace!("Event Type: {}, Data Len: {}", event.event, event.data.len());
                    if event.event == "price_update" || event.event == "message" {
                        tracing::trace!("Received Hermes event: {}", event.data);
                        if let Ok(msg) = serde_json::from_str::<HermesMessage>(&event.data) {
                            for update in msg.parsed {
                                let id = update.id;
                                if let Some(asset) = asset_map
                                    .get(&id)
                                    .or_else(|| asset_map.get(&id.replace("0x", "")))
                                {
                                    if let Ok(price) = update.price.price.parse::<f64>() {
                                        let final_price = price * 10f64.powi(update.price.expo);
                                        let price_data = PriceData::new(
                                            *asset,
                                            final_price,
                                            "hermes-sse".to_string(),
                                        );

                                        // Update local cache
                                        {
                                            let mut prices_lock = prices.write().unwrap();
                                            prices_lock.insert(*asset, price_data.clone());
                                        }

                                        // Update global store if available
                                        if let Some(ref store) = global_store {
                                            store.update_price(*asset, price_data.clone()).await;
                                        }

                                        // Broadcast if channel available
                                        if let Some(ref tx) = update_tx {
                                            let _ = tx.send(price_data);
                                        }

                                        tracing::debug!(
                                            "Updated {} to ${:.2} (Hermes)",
                                            asset.symbol(),
                                            final_price
                                        );

                                        // Update stats
                                        {
                                            let mut stats_lock = stats.write().unwrap();
                                            stats_lock.total_updates += 1;
                                            stats_lock.last_update = std::time::Instant::now();
                                        }
                                    }
                                }
                            }
                        } else {
                            tracing::warn!("Failed to parse Hermes message: {}", event.data);
                        }
                    }
                }
                Err(e) => {
                    error!("Error in Hermes stream: {}", e);
                    return Err(Box::new(e));
                }
            }
        }

        Ok(())
    }
}

#[async_trait]
impl crate::provider::MarketPriceProvider for HermesProvider {
    async fn fetch_price(&self, asset: Asset) -> Result<PriceData, ProviderError> {
        let prices = self.prices.read().unwrap();
        if let Some(data) = prices.get(&asset) {
            Ok(data.clone())
        } else {
            Err(ProviderError::UnsupportedAsset(format!(
                "Price not available for {}",
                asset.symbol()
            )))
        }
    }

    async fn fetch_prices(
        &self,
        assets: &[Asset],
    ) -> Result<HashMap<Asset, PriceData>, ProviderError> {
        let prices = self.prices.read().unwrap();
        let mut result = HashMap::new();
        for asset in assets {
            if let Some(data) = prices.get(asset) {
                result.insert(*asset, data.clone());
            }
        }

        if result.is_empty() {
            Err(ProviderError::UnsupportedAsset(
                "No prices available in cache yet".to_string(),
            ))
        } else {
            Ok(result)
        }
    }

    fn provider_name(&self) -> &'static str {
        "hermes-sse"
    }

    fn is_streaming(&self) -> bool {
        true
    }

    fn start_streaming(
        &self,
        store: Arc<MarketPriceStore>,
        update_tx: broadcast::Sender<PriceData>,
    ) {
        let prices = self.prices.clone();
        let stats = self.stats.clone();
        let client = self.client.clone();

        tokio::spawn(async move {
            loop {
                info!("Connecting to Hermes real-time stream...");
                if let Err(e) = Self::stream_prices(
                    client.clone(),
                    prices.clone(),
                    Some(store.clone()),
                    Some(update_tx.clone()),
                    stats.clone(),
                )
                .await
                {
                    error!("Hermes stream disconnected: {}. Reconnecting in 5s...", e);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        });
    }
}

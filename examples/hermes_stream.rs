use market_price_sdk::provider::MarketPriceProvider;
use market_price_sdk::providers::HermesProvider;
use market_price_sdk::Asset;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Pyth Hermes (V2 Streaming) Example");
    println!("==================================");

    // Initialize Hermes Provider directly
    println!("Connecting to Hermes SSE stream...");
    let provider = HermesProvider::new().await?;

    println!("Waiting for streaming updates...");

    // Watch loop
    for _ in 0..10 {
        sleep(Duration::from_secs(2)).await;

        println!("\n{:-<50}", "");
        for asset in Asset::all() {
            match provider.fetch_price(*asset).await {
                Ok(price) => {
                    println!(
                        "{:<10} ${:<10.4} (Last Update: {:?})",
                        asset.symbol(),
                        price.price_usd,
                        price.age()
                    );
                }
                Err(_) => {
                    // println!("{:<10} Waiting...", asset.symbol());
                }
            }
        }
    }

    Ok(())
}

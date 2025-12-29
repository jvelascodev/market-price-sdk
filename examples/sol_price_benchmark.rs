use market_price_sdk::{MarketPriceTracker, Asset};
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize the tracker
    // On first call to global(), it initializes and starts the background task.
    // However, if we want to measure API latency, we'll use refresh_now().
    let tracker = MarketPriceTracker::global().await;
    let provider_name = tracker.provider_name();
    
    println!("Benchmarking Market Price SDK (Asset: SOL, Provider: {})...", provider_name);
    println!("-------------------------------------------");

    // 2. Benchmark API Latency
    println!("1. Benchmarking API Latency (fetching from external provider)...");
    let start_api = Instant::now();
    
    // Forces an immediate fetch from the remote provider (CoinGecko by default)
    if let Err(e) = tracker.refresh_now().await {
        eprintln!("   Warning: API fetch failed: {}. Using cached value if available.", e);
    }
    let api_latency = start_api.elapsed();
    
    match tracker.get_price(Asset::SOL).await {
        Ok(price) => {
            println!("   Price:  ${:.2}", price.price_usd);
            println!("   Source: {}", price.source);
            println!("   API Latency (network + parsing): {:?}", api_latency);
        }
        Err(e) => {
            eprintln!("   Error: Could not retrieve SOL price: {}", e);
            return Ok(());
        }
    }
    println!();

    // 3. Benchmark Memory Latency
    println!("2. Benchmarking Memory Latency (fetching from internal RwLock cache)...");
    let iterations = 10_000;
    let mut total_memory_latency = std::time::Duration::default();
    
    // Warm up the cache
    let _ = tracker.get_price(Asset::SOL).await?;

    let start_bench = Instant::now();
    for _ in 0..iterations {
        let start_mem = Instant::now();
        // This involves a read lock on the store and a read lock on the asset slot
        let _ = tracker.get_price(Asset::SOL).await?;
        total_memory_latency += start_mem.elapsed();
    }
    let bench_total = start_bench.elapsed();
    
    let avg_memory_latency = total_memory_latency / iterations;
    let avg_total_bench = bench_total / iterations;

    println!("   Total iterations: {}", iterations);
    println!("   Average Retrieval Latency (per call): {:?}", avg_memory_latency);
    println!("   Average Total time (including loop overhead): {:?}", avg_total_bench);
    println!();

    println!("-------------------------------------------");
    println!("Performance Summary:");
    println!("- Network/API Latency:  {:?}", api_latency);
    println!("- Memory Cache Latency: {:?}", avg_memory_latency);
    
    if avg_memory_latency.as_nanos() > 0 {
        let speedup = api_latency.as_secs_f64() / avg_memory_latency.as_secs_f64();
        println!("- Speedup: The internal cache is approx. {:.0}x faster than the network API.", speedup);
    }

    Ok(())
}

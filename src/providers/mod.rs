//! Market price provider implementations

pub mod coingecko;
pub mod failover;
pub mod hyperliquid;

pub use coingecko::CoinGeckoProvider;
pub use failover::FailoverProvider;
pub use hyperliquid::HyperliquidProvider;
pub mod hermes;
pub use hermes::HermesProvider;

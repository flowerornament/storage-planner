//! Pricing API integrations
//!
//! Fetches prices from various sources. Each module implements the PriceSource trait.

pub mod bestbuy;
pub mod ebay;

use anyhow::Result;
use crate::core::models::{ItemCondition, Price, PriceSource as PriceSourceEnum};

/// Trait for price fetching implementations
pub trait PriceFetcher {
    /// Fetch prices for an item by search query
    fn fetch(&self, query: &str) -> Result<Vec<PriceResult>>;

    /// Check if this fetcher is available (has API keys)
    fn is_available(&self) -> bool;

    /// Get the source identifier
    fn source(&self) -> PriceSourceEnum;
}

/// Result from a price fetch
#[derive(Debug, Clone)]
pub struct PriceResult {
    pub price: f64,
    pub currency: String,
    pub condition: ItemCondition,
    pub url: Option<String>,
    pub title: Option<String>,
}

impl PriceResult {
    /// Convert to a Price model
    pub fn to_price(&self, item_id: &str, source: PriceSourceEnum) -> Price {
        let mut price = Price::new(item_id, source, self.price, self.condition);
        price.url = self.url.clone();
        price
    }
}

//! Pricing API integrations
//!
//! Fetches prices and product information from various sources.
//! Each module implements the PriceFetcher and/or ProductFetcher traits.

pub mod bestbuy;
pub mod ebay;
pub mod fallback;
pub mod product;
pub mod url_parser;

use crate::core::models::{ItemCondition, Price, PriceSource as PriceSourceEnum};
use anyhow::Result;

// Re-export commonly used types
pub use fallback::{generate_agent_response, print_fallback_instructions, FallbackReason};
pub use product::{Identifiers, PriceInfo, ProductFetcher, ProductInfo};
pub use url_parser::{parse_url, ParsedUrl, Retailer};

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

/// Try to fetch product info using available APIs
///
/// Returns the first successful result from available fetchers.
/// Order: Best Buy (simple auth) -> eBay (OAuth)
pub fn fetch_product(query: &str) -> Result<Option<ProductInfo>> {
    // Try Best Buy first (simpler API, good for electronics)
    let bestbuy = bestbuy::BestBuyFetcher::new();
    if bestbuy.is_available() {
        if let Ok(products) = ProductFetcher::fetch_by_query(&bestbuy, query) {
            if let Some(product) = products.into_iter().next() {
                return Ok(Some(product));
            }
        }
    }

    // Try eBay
    let ebay = ebay::EbayFetcher::new();
    if ebay.is_available() {
        if let Ok(products) = ProductFetcher::fetch_by_query(&ebay, query) {
            if let Some(product) = products.into_iter().next() {
                return Ok(Some(product));
            }
        }
    }

    Ok(None)
}

/// Check which pricing APIs are available
pub fn available_sources() -> Vec<&'static str> {
    let mut sources = Vec::new();

    if bestbuy::BestBuyFetcher::new().is_available() {
        sources.push("bestbuy");
    }
    if ebay::EbayFetcher::new().is_available() {
        sources.push("ebay");
    }

    sources
}

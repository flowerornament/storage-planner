//! Best Buy Products API integration
//!
//! Requires SP_BESTBUY_API_KEY environment variable.

use anyhow::Result;

use super::{PriceFetcher, PriceResult};
use crate::core::models::PriceSource;

pub struct BestBuyFetcher {
    api_key: Option<String>,
}

impl BestBuyFetcher {
    pub fn new() -> Self {
        Self {
            api_key: std::env::var("SP_BESTBUY_API_KEY").ok(),
        }
    }
}

impl Default for BestBuyFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl PriceFetcher for BestBuyFetcher {
    fn fetch(&self, _query: &str) -> Result<Vec<PriceResult>> {
        if !self.is_available() {
            anyhow::bail!("Best Buy API key not configured");
        }

        // TODO: Implement Best Buy Products API integration
        // 1. Build search query URL
        // 2. Make request with API key
        // 3. Parse results and return PriceResult list

        Ok(Vec::new())
    }

    fn is_available(&self) -> bool {
        self.api_key.is_some()
    }

    fn source(&self) -> PriceSource {
        PriceSource::BestBuy
    }
}

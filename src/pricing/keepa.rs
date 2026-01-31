//! Keepa API integration (Amazon price history)
//!
//! Requires SP_KEEPA_API_KEY environment variable.

use anyhow::Result;

use super::{PriceFetcher, PriceResult};
use crate::core::models::PriceSource;

pub struct KeepaFetcher {
    api_key: Option<String>,
}

impl KeepaFetcher {
    pub fn new() -> Self {
        Self {
            api_key: std::env::var("SP_KEEPA_API_KEY").ok(),
        }
    }
}

impl Default for KeepaFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl PriceFetcher for KeepaFetcher {
    fn fetch(&self, _query: &str) -> Result<Vec<PriceResult>> {
        if !self.is_available() {
            anyhow::bail!("Keepa API key not configured");
        }

        // TODO: Implement Keepa API integration
        // 1. Search for ASIN by product name
        // 2. Get price history for ASIN
        // 3. Return current/recent prices as PriceResult list

        Ok(Vec::new())
    }

    fn is_available(&self) -> bool {
        self.api_key.is_some()
    }

    fn source(&self) -> PriceSource {
        PriceSource::Keepa
    }
}

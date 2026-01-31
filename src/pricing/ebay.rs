//! eBay Browse API integration
//!
//! Requires SP_EBAY_APP_ID and SP_EBAY_CERT_ID environment variables.

use anyhow::Result;

use super::{PriceFetcher, PriceResult};
use crate::core::models::PriceSource;

pub struct EbayFetcher {
    app_id: Option<String>,
    cert_id: Option<String>,
}

impl EbayFetcher {
    pub fn new() -> Self {
        Self {
            app_id: std::env::var("SP_EBAY_APP_ID").ok(),
            cert_id: std::env::var("SP_EBAY_CERT_ID").ok(),
        }
    }
}

impl Default for EbayFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl PriceFetcher for EbayFetcher {
    fn fetch(&self, _query: &str) -> Result<Vec<PriceResult>> {
        if !self.is_available() {
            anyhow::bail!("eBay API credentials not configured");
        }

        // TODO: Implement eBay Browse API integration
        // 1. Get OAuth token using app_id and cert_id
        // 2. Search for items using the Browse API
        // 3. Parse results and return PriceResult list

        Ok(Vec::new())
    }

    fn is_available(&self) -> bool {
        self.app_id.is_some() && self.cert_id.is_some()
    }

    fn source(&self) -> PriceSource {
        PriceSource::Ebay
    }
}

//! eBay Browse API integration
//!
//! Requires SP_EBAY_APP_ID and SP_EBAY_CERT_ID environment variables.
//!
//! Uses OAuth2 Client Credentials flow for authentication.
//! API Documentation: https://developer.ebay.com/api-docs/buy/browse/overview.html

use anyhow::{bail, Result};
use camino::Utf8PathBuf;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use super::product::{Identifiers, PriceInfo, ProductFetcher, ProductInfo};
use super::{PriceFetcher, PriceResult};
use crate::core::models::{ItemCondition, PriceSource};

const TOKEN_URL: &str = "https://api.ebay.com/identity/v1/oauth2/token";
const BROWSE_API_URL: &str = "https://api.ebay.com/buy/browse/v1/item_summary/search";

/// eBay Browse API fetcher
pub struct EbayFetcher {
    app_id: Option<String>,
    cert_id: Option<String>,
    token_cache_dir: Utf8PathBuf,
}

impl EbayFetcher {
    pub fn new() -> Self {
        Self {
            app_id: std::env::var("SP_EBAY_APP_ID").ok(),
            cert_id: std::env::var("SP_EBAY_CERT_ID").ok(),
            token_cache_dir: Utf8PathBuf::from(".sp"),
        }
    }

    /// Check if API credentials are configured
    pub fn is_available(&self) -> bool {
        self.app_id.is_some() && self.cert_id.is_some()
    }

    /// Set a custom directory for token caching
    pub fn with_token_cache_dir(mut self, dir: Utf8PathBuf) -> Self {
        self.token_cache_dir = dir;
        self
    }

    /// Get a valid OAuth2 token, refreshing if needed
    fn get_token(&self) -> Result<String> {
        // Try to load cached token
        if let Some(cached) = self.load_cached_token()? {
            if cached.is_valid() {
                return Ok(cached.access_token);
            }
        }

        // Fetch new token
        let token = self.fetch_new_token()?;
        self.cache_token(&token)?;
        Ok(token.access_token)
    }

    /// Fetch a new OAuth2 token using client credentials
    fn fetch_new_token(&self) -> Result<CachedToken> {
        let app_id = self
            .app_id
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("SP_EBAY_APP_ID not set"))?;
        let cert_id = self
            .cert_id
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("SP_EBAY_CERT_ID not set"))?;

        // Build Basic auth header
        let credentials = format!("{}:{}", app_id, cert_id);
        let auth_header = format!("Basic {}", base64_encode(&credentials));

        let response = ureq::post(TOKEN_URL)
            .set("Authorization", &auth_header)
            .set("Content-Type", "application/x-www-form-urlencoded")
            .send_form(&[
                ("grant_type", "client_credentials"),
                ("scope", "https://api.ebay.com/oauth/api_scope"),
            ])
            .map_err(|e| anyhow::anyhow!("eBay OAuth request failed: {}", e))?;

        let token_response: TokenResponse = response
            .into_json()
            .map_err(|e| anyhow::anyhow!("Failed to parse eBay token response: {}", e))?;

        Ok(CachedToken {
            access_token: token_response.access_token,
            expires_at: Utc::now() + Duration::seconds(token_response.expires_in as i64 - 60), // 1 min buffer
        })
    }

    /// Load cached token from disk
    fn load_cached_token(&self) -> Result<Option<CachedToken>> {
        let path = self.token_cache_dir.join("ebay_token.json");
        if !path.exists() {
            return Ok(None);
        }

        let content = fs_err::read_to_string(&path)?;
        let token: CachedToken = serde_json::from_str(&content)?;
        Ok(Some(token))
    }

    /// Cache token to disk
    fn cache_token(&self, token: &CachedToken) -> Result<()> {
        let path = self.token_cache_dir.join("ebay_token.json");
        let content = serde_json::to_string_pretty(token)?;
        fs_err::write(&path, content)?;
        Ok(())
    }

    /// Search for items using the Browse API
    fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchItem>> {
        let token = self.get_token()?;

        let url = format!(
            "{}?q={}&limit={}",
            BROWSE_API_URL,
            urlencoding::encode(query),
            limit
        );

        let response = ureq::get(&url)
            .set("Authorization", &format!("Bearer {}", token))
            .set("X-EBAY-C-MARKETPLACE-ID", "EBAY_US")
            .call()
            .map_err(|e| anyhow::anyhow!("eBay API request failed: {}", e))?;

        let search_response: SearchResponse = response
            .into_json()
            .map_err(|e| anyhow::anyhow!("Failed to parse eBay search response: {}", e))?;

        Ok(search_response.item_summaries.unwrap_or_default())
    }

    /// Convert eBay condition to our ItemCondition
    fn parse_condition(&self, condition: &Option<String>) -> ItemCondition {
        match condition.as_deref() {
            Some("NEW") | Some("New") => ItemCondition::New,
            Some("USED") | Some("Used") => ItemCondition::Used,
            Some("REFURBISHED") | Some("Refurbished") | Some("Certified - Refurbished") => {
                ItemCondition::Refurbished
            }
            Some(s) if s.to_lowercase().contains("open box") => ItemCondition::OpenBox,
            _ => ItemCondition::Used, // Default to used for eBay
        }
    }

    /// Convert search item to ProductInfo
    fn to_product_info(&self, item: &SearchItem) -> ProductInfo {
        let mut info = ProductInfo::new(&item.title);

        // Try to extract brand from title (common pattern: "Brand - Model")
        if let Some(dash_pos) = item.title.find(" - ") {
            let potential_brand = &item.title[..dash_pos];
            if potential_brand.split_whitespace().count() <= 2 {
                info.brand = Some(potential_brand.to_string());
            }
        }

        info.identifiers = Identifiers {
            ebay_item_id: Some(item.item_id.clone()),
            ..Default::default()
        };

        if let Some(ref price) = item.price {
            if let Ok(amount) = price.value.parse::<f64>() {
                let condition = self.parse_condition(&item.condition);
                info.price = Some(PriceInfo::new(amount).with_condition(condition));
            }
        }

        info.source_url = Some(item.item_web_url.clone());

        info
    }
}

impl Default for EbayFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl PriceFetcher for EbayFetcher {
    fn fetch(&self, query: &str) -> Result<Vec<PriceResult>> {
        if !EbayFetcher::is_available(self) {
            bail!("eBay API credentials not configured. Set SP_EBAY_APP_ID and SP_EBAY_CERT_ID.");
        }

        let items = self.search(query, 10)?;

        let results = items
            .iter()
            .filter_map(|item| {
                let price = item.price.as_ref()?;
                let amount = price.value.parse::<f64>().ok()?;
                Some(PriceResult {
                    price: amount,
                    currency: price.currency.clone(),
                    condition: self.parse_condition(&item.condition),
                    url: Some(item.item_web_url.clone()),
                    title: Some(item.title.clone()),
                })
            })
            .collect();

        Ok(results)
    }

    fn is_available(&self) -> bool {
        EbayFetcher::is_available(self)
    }

    fn source(&self) -> PriceSource {
        PriceSource::Ebay
    }
}

impl ProductFetcher for EbayFetcher {
    fn fetch_by_query(&self, query: &str) -> Result<Vec<ProductInfo>> {
        if !EbayFetcher::is_available(self) {
            bail!("eBay API credentials not configured. Set SP_EBAY_APP_ID and SP_EBAY_CERT_ID.");
        }

        let items = self.search(query, 10)?;
        let products = items.iter().map(|i| self.to_product_info(i)).collect();
        Ok(products)
    }

    fn fetch_by_upc(&self, upc: &str) -> Result<Option<ProductInfo>> {
        if !EbayFetcher::is_available(self) {
            bail!("eBay API credentials not configured. Set SP_EBAY_APP_ID and SP_EBAY_CERT_ID.");
        }

        // eBay Browse API supports GTIN filter
        let token = self.get_token()?;
        let url = format!("{}?gtin={}&limit=1", BROWSE_API_URL, upc);

        let response = ureq::get(&url)
            .set("Authorization", &format!("Bearer {}", token))
            .set("X-EBAY-C-MARKETPLACE-ID", "EBAY_US")
            .call()
            .map_err(|e| anyhow::anyhow!("eBay API request failed: {}", e))?;

        let search_response: SearchResponse = response.into_json()?;

        Ok(search_response
            .item_summaries
            .and_then(|items| items.first().map(|i| self.to_product_info(i))))
    }

    fn fetch_by_id(&self, item_id: &str) -> Result<Option<ProductInfo>> {
        if !EbayFetcher::is_available(self) {
            bail!("eBay API credentials not configured. Set SP_EBAY_APP_ID and SP_EBAY_CERT_ID.");
        }

        // Note: Getting a specific item requires the getItem endpoint, not search
        // For now, we'll do a search with epid filter if available
        // This is a simplification - full item lookup would use /buy/browse/v1/item/{item_id}
        let token = self.get_token()?;
        let url = format!("https://api.ebay.com/buy/browse/v1/item/v1|{}|0", item_id);

        let response = ureq::get(&url)
            .set("Authorization", &format!("Bearer {}", token))
            .set("X-EBAY-C-MARKETPLACE-ID", "EBAY_US")
            .call();

        match response {
            Ok(resp) => {
                let item: ItemResponse = resp.into_json()?;
                let mut info = ProductInfo::new(&item.title);
                info.identifiers.ebay_item_id = Some(item.item_id);
                if let Some(price) = item.price {
                    if let Ok(amount) = price.value.parse::<f64>() {
                        info.price = Some(PriceInfo::new(amount));
                    }
                }
                info.source_url = Some(item.item_web_url);
                Ok(Some(info))
            }
            Err(_) => Ok(None),
        }
    }

    fn is_available(&self) -> bool {
        EbayFetcher::is_available(self)
    }

    fn source_name(&self) -> &'static str {
        "ebay"
    }
}

// Simple base64 encoding for auth header
fn base64_encode(input: &str) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let bytes = input.as_bytes();
    let mut result = String::new();

    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).map(|&b| b as u32).unwrap_or(0);
        let b2 = chunk.get(2).map(|&b| b as u32).unwrap_or(0);

        let triple = (b0 << 16) | (b1 << 8) | b2;

        result.push(ALPHABET[(triple >> 18) as usize & 0x3F] as char);
        result.push(ALPHABET[(triple >> 12) as usize & 0x3F] as char);

        if chunk.len() > 1 {
            result.push(ALPHABET[(triple >> 6) as usize & 0x3F] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(ALPHABET[triple as usize & 0x3F] as char);
        } else {
            result.push('=');
        }
    }

    result
}

// OAuth token caching

#[derive(Debug, Serialize, Deserialize)]
struct CachedToken {
    access_token: String,
    expires_at: DateTime<Utc>,
}

impl CachedToken {
    fn is_valid(&self) -> bool {
        Utc::now() < self.expires_at
    }
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: u64,
}

// Browse API response types

#[derive(Debug, Deserialize)]
struct SearchResponse {
    #[serde(rename = "itemSummaries")]
    item_summaries: Option<Vec<SearchItem>>,
}

#[derive(Debug, Deserialize)]
struct SearchItem {
    #[serde(rename = "itemId")]
    item_id: String,
    title: String,
    price: Option<Price>,
    condition: Option<String>,
    #[serde(rename = "itemWebUrl")]
    item_web_url: String,
}

#[derive(Debug, Deserialize)]
struct Price {
    value: String,
    currency: String,
}

#[derive(Debug, Deserialize)]
struct ItemResponse {
    #[serde(rename = "itemId")]
    item_id: String,
    title: String,
    price: Option<Price>,
    #[serde(rename = "itemWebUrl")]
    item_web_url: String,
}

// Add urlencoding helper since we don't have the crate
mod urlencoding {
    pub fn encode(s: &str) -> String {
        let mut result = String::new();
        for c in s.chars() {
            match c {
                'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '~' => result.push(c),
                ' ' => result.push('+'),
                _ => {
                    for byte in c.to_string().as_bytes() {
                        result.push_str(&format!("%{:02X}", byte));
                    }
                }
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_encode() {
        assert_eq!(base64_encode("hello"), "aGVsbG8=");
        assert_eq!(base64_encode("test:pass"), "dGVzdDpwYXNz");
    }

    #[test]
    fn test_urlencoding() {
        assert_eq!(urlencoding::encode("samsung 870 evo"), "samsung+870+evo");
        assert_eq!(urlencoding::encode("test&query"), "test%26query");
    }

    #[test]
    fn test_parse_condition() {
        let fetcher = EbayFetcher::new();
        assert_eq!(
            fetcher.parse_condition(&Some("NEW".to_string())),
            ItemCondition::New
        );
        assert_eq!(
            fetcher.parse_condition(&Some("USED".to_string())),
            ItemCondition::Used
        );
        assert_eq!(
            fetcher.parse_condition(&Some("Certified - Refurbished".to_string())),
            ItemCondition::Refurbished
        );
    }

    #[test]
    fn test_not_available_without_credentials() {
        std::env::remove_var("SP_EBAY_APP_ID");
        std::env::remove_var("SP_EBAY_CERT_ID");
        let fetcher = EbayFetcher::new();
        assert!(!fetcher.is_available());
    }

    #[test]
    fn test_cached_token_validity() {
        let valid_token = CachedToken {
            access_token: "test".to_string(),
            expires_at: Utc::now() + Duration::hours(1),
        };
        assert!(valid_token.is_valid());

        let expired_token = CachedToken {
            access_token: "test".to_string(),
            expires_at: Utc::now() - Duration::hours(1),
        };
        assert!(!expired_token.is_valid());
    }
}

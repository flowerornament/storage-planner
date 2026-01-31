//! Best Buy Products API integration
//!
//! Requires SP_BESTBUY_API_KEY environment variable.
//!
//! API Documentation: https://developer.bestbuy.com/documentation

use anyhow::{bail, Result};
use serde::Deserialize;
use serde_json::Value as JsonValue;

use super::product::{Identifiers, PriceInfo, ProductFetcher, ProductInfo};
use super::{PriceFetcher, PriceResult};
use crate::core::models::{ItemCondition, PriceSource};

const API_BASE: &str = "https://api.bestbuy.com/v1/products";

/// Best Buy Products API fetcher
pub struct BestBuyFetcher {
    api_key: Option<String>,
}

impl BestBuyFetcher {
    pub fn new() -> Self {
        Self {
            api_key: std::env::var("SP_BESTBUY_API_KEY").ok(),
        }
    }

    /// Check if API key is configured
    pub fn is_available(&self) -> bool {
        self.api_key.is_some()
    }

    /// Build a search URL with the given query
    fn build_search_url(&self, query: &str) -> String {
        let api_key = self.api_key.as_deref().unwrap_or("");
        let encoded_query = query.replace(' ', "+");
        format!(
            "{}(search={})?apiKey={}&format=json&show=sku,name,manufacturer,upc,salePrice,regularPrice,details,url,image,categoryPath",
            API_BASE, encoded_query, api_key
        )
    }

    /// Build a URL to fetch by SKU
    fn build_sku_url(&self, sku: &str) -> String {
        let api_key = self.api_key.as_deref().unwrap_or("");
        format!(
            "{}(sku={})?apiKey={}&format=json&show=sku,name,manufacturer,upc,salePrice,regularPrice,details,url,image,categoryPath",
            API_BASE, sku, api_key
        )
    }

    /// Build a URL to fetch by UPC
    fn build_upc_url(&self, upc: &str) -> String {
        let api_key = self.api_key.as_deref().unwrap_or("");
        format!(
            "{}(upc={})?apiKey={}&format=json&show=sku,name,manufacturer,upc,salePrice,regularPrice,details,url,image,categoryPath",
            API_BASE, upc, api_key
        )
    }

    /// Make an API request and parse the response
    fn make_request(&self, url: &str) -> Result<ApiResponse> {
        let response = ureq::get(url)
            .call()
            .map_err(|e| anyhow::anyhow!("Best Buy API request failed: {}", e))?;

        let body: ApiResponse = response
            .into_json()
            .map_err(|e| anyhow::anyhow!("Failed to parse Best Buy API response: {}", e))?;

        Ok(body)
    }

    /// Convert API product to our ProductInfo
    fn to_product_info(&self, product: &ApiProduct) -> ProductInfo {
        let mut info = ProductInfo::new(&product.name);
        info.brand = product.manufacturer.clone();
        info.category = self.guess_category(&product.category_path);
        info.specs = self.extract_specs(&product.details);
        info.identifiers = Identifiers {
            bestbuy_sku: Some(product.sku.to_string()),
            upc: product.upc.clone(),
            ..Default::default()
        };

        if let Some(price) = product.sale_price.or(product.regular_price) {
            info.price = Some(PriceInfo::new(price));
        }

        info.source_url = product.url.clone();

        info
    }

    /// Extract specs from Best Buy's details array
    fn extract_specs(&self, details: &Option<Vec<ApiDetail>>) -> JsonValue {
        let mut specs = serde_json::Map::new();

        if let Some(details) = details {
            for detail in details {
                // Map common detail names to our spec keys
                let key = match detail.name.to_lowercase().as_str() {
                    "total capacity" | "capacity" => "capacity",
                    "interface" | "interface type" => "interface",
                    "form factor" => "form_factor",
                    "read speed" | "maximum read speed" => "read_speed",
                    "write speed" | "maximum write speed" => "write_speed",
                    "color" => "color",
                    "brand" => continue, // Skip, we have manufacturer
                    "model number" | "model" => "model",
                    "height" => "height",
                    "width" => "width",
                    "depth" => "depth",
                    "weight" => "weight",
                    _ => &detail.name,
                };

                specs.insert(
                    key.to_lowercase().replace(' ', "_"),
                    JsonValue::String(detail.value.clone()),
                );
            }
        }

        JsonValue::Object(specs)
    }

    /// Guess category from Best Buy category path
    fn guess_category(&self, category_path: &Option<Vec<ApiCategory>>) -> Option<String> {
        if let Some(categories) = category_path {
            for cat in categories.iter().rev() {
                let name_lower = cat.name.to_lowercase();
                if name_lower.contains("solid state drive") || name_lower.contains("ssd") {
                    return Some("ssd".to_string());
                }
                // Check enclosure BEFORE hdd (since "hard drive enclosures" contains both)
                if name_lower.contains("enclosure") {
                    return Some("enclosure".to_string());
                }
                if name_lower.contains("hard drive") || name_lower.contains("hdd") {
                    return Some("hdd".to_string());
                }
                if name_lower.contains("cable") {
                    return Some("cable".to_string());
                }
                if name_lower.contains("adapter") {
                    return Some("adapter".to_string());
                }
            }
        }
        None
    }
}

impl Default for BestBuyFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl PriceFetcher for BestBuyFetcher {
    fn fetch(&self, query: &str) -> Result<Vec<PriceResult>> {
        if !BestBuyFetcher::is_available(self) {
            bail!("Best Buy API key not configured. Set SP_BESTBUY_API_KEY environment variable.");
        }

        let url = self.build_search_url(query);
        let response = self.make_request(&url)?;

        let results = response
            .products
            .iter()
            .filter_map(|p| {
                p.sale_price.or(p.regular_price).map(|price| PriceResult {
                    price,
                    currency: "USD".to_string(),
                    condition: ItemCondition::New,
                    url: p.url.clone(),
                    title: Some(p.name.clone()),
                })
            })
            .collect();

        Ok(results)
    }

    fn is_available(&self) -> bool {
        BestBuyFetcher::is_available(self)
    }

    fn source(&self) -> PriceSource {
        PriceSource::BestBuy
    }
}

impl ProductFetcher for BestBuyFetcher {
    fn fetch_by_query(&self, query: &str) -> Result<Vec<ProductInfo>> {
        if !BestBuyFetcher::is_available(self) {
            bail!("Best Buy API key not configured. Set SP_BESTBUY_API_KEY environment variable.");
        }

        let url = self.build_search_url(query);
        let response = self.make_request(&url)?;

        let products = response
            .products
            .iter()
            .map(|p| self.to_product_info(p))
            .collect();

        Ok(products)
    }

    fn fetch_by_upc(&self, upc: &str) -> Result<Option<ProductInfo>> {
        if !BestBuyFetcher::is_available(self) {
            bail!("Best Buy API key not configured. Set SP_BESTBUY_API_KEY environment variable.");
        }

        let url = self.build_upc_url(upc);
        let response = self.make_request(&url)?;

        Ok(response.products.first().map(|p| self.to_product_info(p)))
    }

    fn fetch_by_id(&self, sku: &str) -> Result<Option<ProductInfo>> {
        if !BestBuyFetcher::is_available(self) {
            bail!("Best Buy API key not configured. Set SP_BESTBUY_API_KEY environment variable.");
        }

        let url = self.build_sku_url(sku);
        let response = self.make_request(&url)?;

        Ok(response.products.first().map(|p| self.to_product_info(p)))
    }

    fn is_available(&self) -> bool {
        BestBuyFetcher::is_available(self)
    }

    fn source_name(&self) -> &'static str {
        "bestbuy"
    }
}

// API response types

#[derive(Debug, Deserialize)]
struct ApiResponse {
    #[serde(default)]
    products: Vec<ApiProduct>,
}

#[derive(Debug, Deserialize)]
struct ApiProduct {
    sku: u64,
    name: String,
    manufacturer: Option<String>,
    upc: Option<String>,
    #[serde(rename = "salePrice")]
    sale_price: Option<f64>,
    #[serde(rename = "regularPrice")]
    regular_price: Option<f64>,
    details: Option<Vec<ApiDetail>>,
    url: Option<String>,
    #[serde(rename = "categoryPath")]
    category_path: Option<Vec<ApiCategory>>,
}

#[derive(Debug, Deserialize)]
struct ApiDetail {
    name: String,
    value: String,
}

#[derive(Debug, Deserialize)]
struct ApiCategory {
    name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_search_url() {
        std::env::set_var("SP_BESTBUY_API_KEY", "test_key");
        let fetcher = BestBuyFetcher::new();
        let url = fetcher.build_search_url("samsung 870 evo");
        assert!(url.contains("search=samsung+870+evo"));
        assert!(url.contains("apiKey=test_key"));
        std::env::remove_var("SP_BESTBUY_API_KEY");
    }

    #[test]
    fn test_extract_specs() {
        let fetcher = BestBuyFetcher::new();
        let details = Some(vec![
            ApiDetail {
                name: "Total Capacity".to_string(),
                value: "4TB".to_string(),
            },
            ApiDetail {
                name: "Interface".to_string(),
                value: "SATA".to_string(),
            },
        ]);

        let specs = fetcher.extract_specs(&details);
        assert_eq!(specs["capacity"], "4TB");
        assert_eq!(specs["interface"], "SATA");
    }

    #[test]
    fn test_guess_category() {
        let fetcher = BestBuyFetcher::new();

        let ssd_path = Some(vec![
            ApiCategory {
                name: "Electronics".to_string(),
            },
            ApiCategory {
                name: "Solid State Drives".to_string(),
            },
        ]);
        assert_eq!(fetcher.guess_category(&ssd_path), Some("ssd".to_string()));

        let enclosure_path = Some(vec![ApiCategory {
            name: "Hard Drive Enclosures".to_string(),
        }]);
        assert_eq!(
            fetcher.guess_category(&enclosure_path),
            Some("enclosure".to_string())
        );
    }

    #[test]
    fn test_not_available_without_key() {
        std::env::remove_var("SP_BESTBUY_API_KEY");
        let fetcher = BestBuyFetcher::new();
        assert!(!fetcher.is_available());
    }
}

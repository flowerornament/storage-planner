//! Product information structures for API fetching
//!
//! Common types for product data returned by pricing APIs.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::core::models::ItemCondition;

/// Product information returned from APIs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductInfo {
    /// Product name
    pub name: String,

    /// Brand/manufacturer
    pub brand: Option<String>,

    /// Suggested category (ssd, enclosure, cable, etc.)
    pub category: Option<String>,

    /// Product specifications as key-value pairs
    pub specs: JsonValue,

    /// Identifiers (ASIN, UPC, SKU, etc.)
    pub identifiers: Identifiers,

    /// Current price if available
    pub price: Option<PriceInfo>,

    /// Source URL where product was found
    pub source_url: Option<String>,
}

impl ProductInfo {
    /// Create a minimal ProductInfo with just a name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            brand: None,
            category: None,
            specs: JsonValue::Object(Default::default()),
            identifiers: Identifiers::default(),
            price: None,
            source_url: None,
        }
    }

    /// Create a suggested item ID from the product info
    pub fn suggested_item_id(&self) -> String {
        // Build ID from brand and name, slugified
        let mut parts = Vec::new();

        if let Some(ref brand) = self.brand {
            parts.push(slugify(brand));
        }

        parts.push(slugify(&self.name));

        parts.join("-")
    }
}

/// Product identifiers from various sources
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Identifiers {
    /// Amazon Standard Identification Number
    pub asin: Option<String>,

    /// Universal Product Code
    pub upc: Option<String>,

    /// Best Buy SKU
    pub bestbuy_sku: Option<String>,

    /// eBay item ID
    pub ebay_item_id: Option<String>,

    /// Manufacturer part number
    pub mpn: Option<String>,
}

impl Identifiers {
    /// Check if any identifier is set
    pub fn has_any(&self) -> bool {
        self.asin.is_some()
            || self.upc.is_some()
            || self.bestbuy_sku.is_some()
            || self.ebay_item_id.is_some()
            || self.mpn.is_some()
    }

    /// Convert to JSON for storage in metadata
    pub fn to_json(&self) -> JsonValue {
        let mut obj = serde_json::Map::new();

        if let Some(ref asin) = self.asin {
            obj.insert("asin".to_string(), JsonValue::String(asin.clone()));
        }
        if let Some(ref upc) = self.upc {
            obj.insert("upc".to_string(), JsonValue::String(upc.clone()));
        }
        if let Some(ref sku) = self.bestbuy_sku {
            obj.insert("bestbuy_sku".to_string(), JsonValue::String(sku.clone()));
        }
        if let Some(ref item_id) = self.ebay_item_id {
            obj.insert(
                "ebay_item_id".to_string(),
                JsonValue::String(item_id.clone()),
            );
        }
        if let Some(ref mpn) = self.mpn {
            obj.insert("mpn".to_string(), JsonValue::String(mpn.clone()));
        }

        JsonValue::Object(obj)
    }

    /// Parse from JSON metadata
    pub fn from_json(value: &JsonValue) -> Self {
        let obj = value.as_object();

        Self {
            asin: obj
                .and_then(|o| o.get("asin"))
                .and_then(|v| v.as_str())
                .map(String::from),
            upc: obj
                .and_then(|o| o.get("upc"))
                .and_then(|v| v.as_str())
                .map(String::from),
            bestbuy_sku: obj
                .and_then(|o| o.get("bestbuy_sku"))
                .and_then(|v| v.as_str())
                .map(String::from),
            ebay_item_id: obj
                .and_then(|o| o.get("ebay_item_id"))
                .and_then(|v| v.as_str())
                .map(String::from),
            mpn: obj
                .and_then(|o| o.get("mpn"))
                .and_then(|v| v.as_str())
                .map(String::from),
        }
    }
}

/// Price information from an API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceInfo {
    pub amount: f64,
    pub currency: String,
    pub condition: ItemCondition,
}

impl PriceInfo {
    pub fn new(amount: f64) -> Self {
        Self {
            amount,
            currency: "USD".to_string(),
            condition: ItemCondition::New,
        }
    }

    pub fn with_condition(mut self, condition: ItemCondition) -> Self {
        self.condition = condition;
        self
    }
}

/// Trait for fetching product information from APIs
pub trait ProductFetcher {
    /// Fetch product info by search query
    fn fetch_by_query(&self, query: &str) -> Result<Vec<ProductInfo>>;

    /// Fetch product info by UPC
    fn fetch_by_upc(&self, upc: &str) -> Result<Option<ProductInfo>>;

    /// Fetch product info by retailer-specific identifier
    fn fetch_by_id(&self, id: &str) -> Result<Option<ProductInfo>>;

    /// Check if this fetcher is available (has API credentials)
    fn is_available(&self) -> bool;

    /// Get the source name for this fetcher
    fn source_name(&self) -> &'static str;
}

/// Convert a string to a URL-safe slug
fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_product_info_suggested_id() {
        let mut product = ProductInfo::new("870 EVO 4TB");
        product.brand = Some("Samsung".to_string());
        assert_eq!(product.suggested_item_id(), "samsung-870-evo-4tb");
    }

    #[test]
    fn test_identifiers_to_json() {
        let ids = Identifiers {
            asin: Some("B089C5P5SX".to_string()),
            upc: Some("887276458519".to_string()),
            bestbuy_sku: None,
            ebay_item_id: None,
            mpn: None,
        };

        let json = ids.to_json();
        assert_eq!(json["asin"], "B089C5P5SX");
        assert_eq!(json["upc"], "887276458519");
        assert!(json.get("bestbuy_sku").is_none());
    }

    #[test]
    fn test_identifiers_from_json() {
        let json = serde_json::json!({
            "asin": "B089C5P5SX",
            "bestbuy_sku": "6405087"
        });

        let ids = Identifiers::from_json(&json);
        assert_eq!(ids.asin, Some("B089C5P5SX".to_string()));
        assert_eq!(ids.bestbuy_sku, Some("6405087".to_string()));
        assert!(ids.upc.is_none());
    }

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Samsung 870 EVO"), "samsung-870-evo");
        assert_eq!(slugify("Test--Multiple   Spaces"), "test-multiple-spaces");
        assert_eq!(slugify("With/Special@Chars!"), "with-special-chars");
    }
}

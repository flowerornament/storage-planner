//! URL parsing for retailer product pages
//!
//! Extracts product identifiers from retailer URLs.

use anyhow::{bail, Result};

/// Supported retailer types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Retailer {
    Amazon,
    BestBuy,
    Ebay,
}

impl Retailer {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Amazon => "amazon",
            Self::BestBuy => "bestbuy",
            Self::Ebay => "ebay",
        }
    }
}

/// Parsed product URL containing retailer and identifier
#[derive(Debug, Clone)]
pub struct ParsedUrl {
    pub retailer: Retailer,
    pub identifier: String,
    pub original_url: String,
}

impl ParsedUrl {
    /// Get the identifier key name for metadata storage
    pub fn identifier_key(&self) -> &'static str {
        match self.retailer {
            Retailer::Amazon => "asin",
            Retailer::BestBuy => "bestbuy_sku",
            Retailer::Ebay => "ebay_item_id",
        }
    }
}

/// Parse a retailer URL and extract the product identifier
///
/// Supports:
/// - Amazon: `/dp/XXXXXXXXXX` or `/gp/product/XXXXXXXXXX`
/// - Best Buy: `/site/.../XXXXXXX.p`
/// - eBay: `/itm/XXXXXXXXXXX` or `/itm/.../XXXXXXXXXXX`
pub fn parse_url(url: &str) -> Result<ParsedUrl> {
    let url_lower = url.to_lowercase();

    // Amazon
    if url_lower.contains("amazon.com") || url_lower.contains("amzn.to") {
        if let Some(asin) = extract_amazon_asin(url) {
            return Ok(ParsedUrl {
                retailer: Retailer::Amazon,
                identifier: asin,
                original_url: url.to_string(),
            });
        }
        bail!("Could not extract ASIN from Amazon URL: {}", url);
    }

    // Best Buy
    if url_lower.contains("bestbuy.com") {
        if let Some(sku) = extract_bestbuy_sku(url) {
            return Ok(ParsedUrl {
                retailer: Retailer::BestBuy,
                identifier: sku,
                original_url: url.to_string(),
            });
        }
        bail!("Could not extract SKU from Best Buy URL: {}", url);
    }

    // eBay
    if url_lower.contains("ebay.com") {
        if let Some(item_id) = extract_ebay_item_id(url) {
            return Ok(ParsedUrl {
                retailer: Retailer::Ebay,
                identifier: item_id,
                original_url: url.to_string(),
            });
        }
        bail!("Could not extract item ID from eBay URL: {}", url);
    }

    bail!(
        "Unsupported URL. Supported retailers: Amazon, Best Buy, eBay. Got: {}",
        url
    );
}

/// Extract ASIN from Amazon URL
///
/// Patterns:
/// - https://www.amazon.com/dp/B089C5P5SX
/// - https://www.amazon.com/gp/product/B089C5P5SX
/// - https://www.amazon.com/Samsung-Internal-MZ-77E4T0B-AM/dp/B089C5P5SX
fn extract_amazon_asin(url: &str) -> Option<String> {
    // Pattern 1: /dp/ASIN
    if let Some(pos) = url.find("/dp/") {
        let start = pos + 4;
        let rest = &url[start..];
        let asin = rest
            .split(|c: char| c == '/' || c == '?' || c == '#')
            .next()?;
        if is_valid_asin(asin) {
            return Some(asin.to_uppercase());
        }
    }

    // Pattern 2: /gp/product/ASIN
    if let Some(pos) = url.find("/gp/product/") {
        let start = pos + 12;
        let rest = &url[start..];
        let asin = rest
            .split(|c: char| c == '/' || c == '?' || c == '#')
            .next()?;
        if is_valid_asin(asin) {
            return Some(asin.to_uppercase());
        }
    }

    None
}

/// Check if a string looks like a valid ASIN (10 alphanumeric chars, starts with B for products)
fn is_valid_asin(s: &str) -> bool {
    s.len() == 10 && s.chars().all(|c| c.is_ascii_alphanumeric())
}

/// Extract SKU from Best Buy URL
///
/// Pattern: https://www.bestbuy.com/site/samsung-870-evo-4tb/6405087.p
fn extract_bestbuy_sku(url: &str) -> Option<String> {
    // Pattern: /XXXXXXX.p at the end
    if let Some(pos) = url.rfind('/') {
        let segment = &url[pos + 1..];
        if let Some(sku) = segment.strip_suffix(".p") {
            // SKU should be numeric
            if sku.chars().all(|c| c.is_ascii_digit()) && !sku.is_empty() {
                return Some(sku.to_string());
            }
        }
    }

    // Alternative: skuId query parameter
    if let Some(pos) = url.find("skuId=") {
        let start = pos + 6;
        let rest = &url[start..];
        let sku = rest.split('&').next()?;
        if sku.chars().all(|c| c.is_ascii_digit()) && !sku.is_empty() {
            return Some(sku.to_string());
        }
    }

    None
}

/// Extract item ID from eBay URL
///
/// Patterns:
/// - https://www.ebay.com/itm/123456789012
/// - https://www.ebay.com/itm/Samsung-870-EVO-4TB/123456789012
fn extract_ebay_item_id(url: &str) -> Option<String> {
    // Find /itm/ and extract the numeric ID
    if let Some(pos) = url.find("/itm/") {
        let rest = &url[pos + 5..];

        // The item ID is the last numeric segment before ? or end
        let segments: Vec<&str> = rest.split('?').next()?.split('/').collect();

        // Try last segment first (most common)
        for segment in segments.iter().rev() {
            if segment.chars().all(|c| c.is_ascii_digit()) && segment.len() >= 10 {
                return Some(segment.to_string());
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_amazon_dp_url() {
        let parsed = parse_url("https://www.amazon.com/dp/B089C5P5SX").unwrap();
        assert_eq!(parsed.retailer, Retailer::Amazon);
        assert_eq!(parsed.identifier, "B089C5P5SX");
    }

    #[test]
    fn test_amazon_full_url() {
        let parsed = parse_url(
            "https://www.amazon.com/Samsung-Internal-MZ-77E4T0B-AM/dp/B089C5P5SX?ref=something",
        )
        .unwrap();
        assert_eq!(parsed.retailer, Retailer::Amazon);
        assert_eq!(parsed.identifier, "B089C5P5SX");
    }

    #[test]
    fn test_amazon_gp_product_url() {
        let parsed = parse_url("https://www.amazon.com/gp/product/B089C5P5SX").unwrap();
        assert_eq!(parsed.retailer, Retailer::Amazon);
        assert_eq!(parsed.identifier, "B089C5P5SX");
    }

    #[test]
    fn test_bestbuy_url() {
        let parsed =
            parse_url("https://www.bestbuy.com/site/samsung-870-evo-4tb/6405087.p").unwrap();
        assert_eq!(parsed.retailer, Retailer::BestBuy);
        assert_eq!(parsed.identifier, "6405087");
    }

    #[test]
    fn test_bestbuy_with_query() {
        let parsed = parse_url(
            "https://www.bestbuy.com/site/samsung-870-evo-4tb/6405087.p?skuId=6405087",
        )
        .unwrap();
        assert_eq!(parsed.retailer, Retailer::BestBuy);
        assert_eq!(parsed.identifier, "6405087");
    }

    #[test]
    fn test_ebay_simple_url() {
        let parsed = parse_url("https://www.ebay.com/itm/123456789012").unwrap();
        assert_eq!(parsed.retailer, Retailer::Ebay);
        assert_eq!(parsed.identifier, "123456789012");
    }

    #[test]
    fn test_ebay_with_title() {
        let parsed =
            parse_url("https://www.ebay.com/itm/Samsung-870-EVO-4TB/123456789012?hash=something")
                .unwrap();
        assert_eq!(parsed.retailer, Retailer::Ebay);
        assert_eq!(parsed.identifier, "123456789012");
    }

    #[test]
    fn test_unsupported_url() {
        let result = parse_url("https://newegg.com/product/123");
        assert!(result.is_err());
    }

    #[test]
    fn test_identifier_key() {
        let amazon = ParsedUrl {
            retailer: Retailer::Amazon,
            identifier: "B089C5P5SX".to_string(),
            original_url: "".to_string(),
        };
        assert_eq!(amazon.identifier_key(), "asin");

        let bestbuy = ParsedUrl {
            retailer: Retailer::BestBuy,
            identifier: "6405087".to_string(),
            original_url: "".to_string(),
        };
        assert_eq!(bestbuy.identifier_key(), "bestbuy_sku");
    }
}

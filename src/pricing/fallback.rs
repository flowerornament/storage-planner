//! Agent fallback prompts for when API fetching fails
//!
//! Generates structured prompts to help agents manually populate item data.

use console::style;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// Reasons why automatic fetching failed
#[derive(Debug, Clone)]
pub enum FallbackReason {
    /// No API keys configured
    NoApiKeys,
    /// API returned an error
    ApiError(String),
    /// Product not found
    NotFound,
    /// Retailer not supported for auto-fetch
    UnsupportedRetailer(String),
    /// Rate limited
    RateLimited,
}

impl FallbackReason {
    pub fn as_str(&self) -> &str {
        match self {
            Self::NoApiKeys => "no API keys configured",
            Self::ApiError(_) => "API error",
            Self::NotFound => "product not found",
            Self::UnsupportedRetailer(_) => "retailer not supported for auto-fetch",
            Self::RateLimited => "rate limited",
        }
    }
}

/// Schema information for agent mode JSON output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportSchema {
    pub name: SchemaField,
    pub brand: SchemaField,
    pub category: SchemaField,
    pub specs: SchemaField,
    pub price: SchemaField,
    pub condition: SchemaField,
    pub source: SchemaField,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaField {
    #[serde(rename = "type")]
    pub field_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example: Option<JsonValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "enum", skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<String>>,
}

impl Default for ImportSchema {
    fn default() -> Self {
        Self {
            name: SchemaField {
                field_type: "string".to_string(),
                required: Some(true),
                example: Some(JsonValue::String("Samsung 870 EVO 4TB".to_string())),
                description: Some("Product name".to_string()),
                enum_values: None,
            },
            brand: SchemaField {
                field_type: "string".to_string(),
                required: Some(false),
                example: Some(JsonValue::String("Samsung".to_string())),
                description: Some("Brand/manufacturer".to_string()),
                enum_values: None,
            },
            category: SchemaField {
                field_type: "string".to_string(),
                required: Some(true),
                example: Some(JsonValue::String("ssd".to_string())),
                description: Some("Product category".to_string()),
                enum_values: Some(vec![
                    "ssd".to_string(),
                    "enclosure".to_string(),
                    "cable".to_string(),
                    "adapter".to_string(),
                    "software".to_string(),
                    "other".to_string(),
                ]),
            },
            specs: SchemaField {
                field_type: "object".to_string(),
                required: Some(false),
                example: Some(serde_json::json!({
                    "capacity": "4TB",
                    "interface": "SATA",
                    "form_factor": "2.5\""
                })),
                description: Some("Product specifications".to_string()),
                enum_values: None,
            },
            price: SchemaField {
                field_type: "number".to_string(),
                required: Some(false),
                example: Some(JsonValue::Number(289.into())),
                description: Some("Price in USD".to_string()),
                enum_values: None,
            },
            condition: SchemaField {
                field_type: "string".to_string(),
                required: Some(false),
                example: Some(JsonValue::String("new".to_string())),
                description: Some("Item condition".to_string()),
                enum_values: Some(vec![
                    "new".to_string(),
                    "used".to_string(),
                    "refurbished".to_string(),
                    "open_box".to_string(),
                ]),
            },
            source: SchemaField {
                field_type: "string".to_string(),
                required: Some(false),
                example: Some(JsonValue::String("amazon".to_string())),
                description: Some("Price source".to_string()),
                enum_values: Some(vec![
                    "amazon".to_string(),
                    "ebay".to_string(),
                    "bestbuy".to_string(),
                    "manual".to_string(),
                ]),
            },
        }
    }
}

/// Response for agent mode when fallback is needed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentFallbackResponse {
    pub status: String,
    pub reason: String,
    pub search_query: String,
    pub schema: ImportSchema,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partial_data: Option<JsonValue>,
}

impl AgentFallbackResponse {
    pub fn new(reason: FallbackReason, search_query: &str) -> Self {
        Self {
            status: "fallback_required".to_string(),
            reason: reason.as_str().to_string(),
            search_query: search_query.to_string(),
            schema: ImportSchema::default(),
            suggested_id: None,
            partial_data: None,
        }
    }

    pub fn with_suggested_id(mut self, id: String) -> Self {
        self.suggested_id = Some(id);
        self
    }

    pub fn with_partial_data(mut self, data: JsonValue) -> Self {
        self.partial_data = Some(data);
        self
    }
}

/// Generate human-readable fallback instructions
pub fn print_fallback_instructions(reason: FallbackReason, search_query: &str) {
    println!();
    println!(
        "{} Could not auto-fetch (reason: {})",
        style("!").yellow(),
        reason.as_str()
    );
    println!();
    println!("Search manually for: {}", style(search_query).cyan().bold());
    println!();
    println!("Then run:");
    println!(
        "  {}",
        style(
            r#"sp item import --json='{"name":"...","brand":"...","category":"ssd","specs":{...},"price":289,"source":"amazon"}'"#
        )
        .dim()
    );
    println!();
    println!("Or use stdin:");
    println!(
        "  {}",
        style(r#"echo '{"name":"...", ...}' | sp item import --stdin"#).dim()
    );
    println!();
}

/// Generate agent-mode JSON response
pub fn generate_agent_response(
    reason: FallbackReason,
    search_query: &str,
    suggested_id: Option<&str>,
    partial_data: Option<JsonValue>,
) -> String {
    let mut response = AgentFallbackResponse::new(reason, search_query);

    if let Some(id) = suggested_id {
        response = response.with_suggested_id(id.to_string());
    }

    if let Some(data) = partial_data {
        response = response.with_partial_data(data);
    }

    serde_json::to_string_pretty(&response).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_response_serialization() {
        let response = AgentFallbackResponse::new(FallbackReason::NoApiKeys, "Samsung 870 EVO 4TB");

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("fallback_required"));
        assert!(json.contains("Samsung 870 EVO 4TB"));
    }

    #[test]
    fn test_agent_response_with_partial_data() {
        let response =
            AgentFallbackResponse::new(FallbackReason::ApiError("timeout".into()), "test")
                .with_suggested_id("samsung-870-evo-4tb".into())
                .with_partial_data(serde_json::json!({"brand": "Samsung"}));

        assert!(response.suggested_id.is_some());
        assert!(response.partial_data.is_some());
    }

    #[test]
    fn test_import_schema_default() {
        let schema = ImportSchema::default();
        assert!(schema.name.required.unwrap_or(false));
        assert!(schema.category.enum_values.is_some());
    }
}

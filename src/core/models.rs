//! Core domain models
//!
//! These are the fundamental abstractions that work across any purchase decision domain.

use chrono::{DateTime, Utc};
use rusqlite::{params, Row, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// A purchasable item with specs and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub category: String,
    pub brand: Option<String>,
    pub specs: JsonValue,
    pub tags: Vec<String>,
    pub metadata: JsonValue,
    pub archived: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Item {
    /// Create a new item with required fields
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        category: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: id.into(),
            name: name.into(),
            category: category.into(),
            brand: None,
            specs: JsonValue::Object(Default::default()),
            tags: Vec::new(),
            metadata: JsonValue::Object(Default::default()),
            archived: false,
            created_at: now,
            updated_at: now,
        }
    }

    /// Insert into database
    pub fn insert(&self, tx: &Transaction) -> rusqlite::Result<()> {
        tx.execute(
            "INSERT INTO items (id, name, category, brand, specs, tags, metadata, archived, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                self.id,
                self.name,
                self.category,
                self.brand,
                serde_json::to_string(&self.specs).unwrap(),
                serde_json::to_string(&self.tags).unwrap(),
                serde_json::to_string(&self.metadata).unwrap(),
                self.archived as i32,
                self.created_at.to_rfc3339(),
                self.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Load from database row
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let specs_str: String = row.get("specs")?;
        let tags_str: String = row.get("tags")?;
        let metadata_str: String = row.get("metadata")?;
        let created_str: String = row.get("created_at")?;
        let updated_str: String = row.get("updated_at")?;

        Ok(Self {
            id: row.get("id")?,
            name: row.get("name")?,
            category: row.get("category")?,
            brand: row.get("brand")?,
            specs: serde_json::from_str(&specs_str).unwrap_or_default(),
            tags: serde_json::from_str(&tags_str).unwrap_or_default(),
            metadata: serde_json::from_str(&metadata_str).unwrap_or_default(),
            archived: row.get::<_, i32>("archived")? != 0,
            created_at: DateTime::parse_from_rfc3339(&created_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_at: DateTime::parse_from_rfc3339(&updated_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        })
    }
}

/// A price observation at a point in time (append-only)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Price {
    pub id: String,
    pub item_id: String,
    pub source: PriceSource,
    pub price: f64,
    pub currency: String,
    pub condition: ItemCondition,
    pub url: Option<String>,
    pub observed_at: DateTime<Utc>,
    pub metadata: JsonValue,
}

impl Price {
    /// Create a new price observation
    pub fn new(
        item_id: impl Into<String>,
        source: PriceSource,
        price: f64,
        condition: ItemCondition,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            item_id: item_id.into(),
            source,
            price,
            currency: "USD".to_string(),
            condition,
            url: None,
            observed_at: Utc::now(),
            metadata: JsonValue::Object(Default::default()),
        }
    }

    /// Insert into database (append-only)
    pub fn insert(&self, tx: &Transaction) -> rusqlite::Result<()> {
        tx.execute(
            "INSERT INTO prices (id, item_id, source, price, currency, condition, url, observed_at, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                self.id,
                self.item_id,
                self.source.as_str(),
                self.price,
                self.currency,
                self.condition.as_str(),
                self.url,
                self.observed_at.to_rfc3339(),
                serde_json::to_string(&self.metadata).unwrap(),
            ],
        )?;
        Ok(())
    }

    /// Load from database row
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let source_str: String = row.get("source")?;
        let condition_str: String = row.get("condition")?;
        let observed_str: String = row.get("observed_at")?;
        let metadata_str: String = row.get("metadata")?;

        Ok(Self {
            id: row.get("id")?,
            item_id: row.get("item_id")?,
            source: PriceSource::parse(&source_str),
            price: row.get("price")?,
            currency: row.get("currency")?,
            condition: ItemCondition::parse(&condition_str),
            url: row.get("url")?,
            observed_at: DateTime::parse_from_rfc3339(&observed_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            metadata: serde_json::from_str(&metadata_str).unwrap_or_default(),
        })
    }
}

/// Where a price observation came from
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PriceSource {
    Ebay,
    BestBuy,
    Amazon,
    Manual,
    /// Custom source for retailers not in the known list
    #[serde(untagged)]
    Custom(String),
}

impl PriceSource {
    pub fn as_str(&self) -> String {
        match self {
            Self::Ebay => "ebay".to_string(),
            Self::BestBuy => "bestbuy".to_string(),
            Self::Amazon => "amazon".to_string(),
            Self::Manual => "manual".to_string(),
            Self::Custom(s) => s.clone(),
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "ebay" => Self::Ebay,
            "bestbuy" | "best_buy" => Self::BestBuy,
            "amazon" => Self::Amazon,
            "manual" => Self::Manual,
            _ => Self::Custom(s.to_string()),
        }
    }
}

/// Condition of an item for sale
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ItemCondition {
    New,
    Used,
    Refurbished,
    OpenBox,
}

impl ItemCondition {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Used => "used",
            Self::Refurbished => "refurbished",
            Self::OpenBox => "open_box",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "new" => Self::New,
            "used" => Self::Used,
            "refurbished" | "refurb" => Self::Refurbished,
            "open_box" | "openbox" => Self::OpenBox,
            _ => Self::New,
        }
    }
}

/// A named composition of items forming a system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Configuration {
    pub id: String,
    pub name: String,
    pub domain: String,
    pub items: Vec<ConfigItem>,
    pub domain_data: JsonValue,
    pub metadata: JsonValue,
    pub is_current: bool,
    pub archived: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// An item reference within a configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigItem {
    pub item_id: String,
    pub quantity: u32,
    pub unit_price: Option<f64>,
    pub notes: Option<String>,
}

impl Configuration {
    /// Create a new configuration
    pub fn new(id: impl Into<String>, name: impl Into<String>, domain: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: id.into(),
            name: name.into(),
            domain: domain.into(),
            items: Vec::new(),
            domain_data: JsonValue::Object(Default::default()),
            metadata: JsonValue::Object(Default::default()),
            is_current: false,
            archived: false,
            created_at: now,
            updated_at: now,
        }
    }

    /// Insert into database
    pub fn insert(&self, tx: &Transaction) -> rusqlite::Result<()> {
        tx.execute(
            "INSERT INTO configurations (id, name, domain, items, domain_data, metadata, is_current, archived, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                self.id,
                self.name,
                self.domain,
                serde_json::to_string(&self.items).unwrap(),
                serde_json::to_string(&self.domain_data).unwrap(),
                serde_json::to_string(&self.metadata).unwrap(),
                self.is_current as i32,
                self.archived as i32,
                self.created_at.to_rfc3339(),
                self.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Load from database row
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let items_str: String = row.get("items")?;
        let domain_data_str: String = row.get("domain_data")?;
        let metadata_str: String = row.get("metadata")?;
        let created_str: String = row.get("created_at")?;
        let updated_str: String = row.get("updated_at")?;

        Ok(Self {
            id: row.get("id")?,
            name: row.get("name")?,
            domain: row.get("domain")?,
            items: serde_json::from_str(&items_str).unwrap_or_default(),
            domain_data: serde_json::from_str(&domain_data_str).unwrap_or_default(),
            metadata: serde_json::from_str(&metadata_str).unwrap_or_default(),
            is_current: row.get::<_, i32>("is_current")? != 0,
            archived: row.get::<_, i32>("archived")? != 0,
            created_at: DateTime::parse_from_rfc3339(&created_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_at: DateTime::parse_from_rfc3339(&updated_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        })
    }

    /// Calculate total cost using item prices
    pub fn total_cost(&self) -> f64 {
        self.items
            .iter()
            .filter_map(|i| i.unit_price.map(|p| p * i.quantity as f64))
            .sum()
    }
}

/// An immutable event in the audit log
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub event_type: EventType,
    pub entity_type: EntityType,
    pub entity_id: String,
    pub payload: JsonValue,
    pub timestamp: DateTime<Utc>,
    pub actor: String,
}

impl Event {
    /// Create a new event
    pub fn new(
        event_type: EventType,
        entity_type: EntityType,
        entity_id: impl Into<String>,
        payload: JsonValue,
        actor: impl Into<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            event_type,
            entity_type,
            entity_id: entity_id.into(),
            payload,
            timestamp: Utc::now(),
            actor: actor.into(),
        }
    }

    /// Insert into database (append-only)
    pub fn insert(&self, tx: &Transaction) -> rusqlite::Result<()> {
        tx.execute(
            "INSERT INTO events (id, event_type, entity_type, entity_id, payload, timestamp, actor)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                self.id,
                self.event_type.as_str(),
                self.entity_type.as_str(),
                self.entity_id,
                serde_json::to_string(&self.payload).unwrap(),
                self.timestamp.to_rfc3339(),
                self.actor,
            ],
        )?;
        Ok(())
    }

    /// Load from database row
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let event_type_str: String = row.get("event_type")?;
        let entity_type_str: String = row.get("entity_type")?;
        let payload_str: String = row.get("payload")?;
        let timestamp_str: String = row.get("timestamp")?;

        Ok(Self {
            id: row.get("id")?,
            event_type: EventType::parse(&event_type_str),
            entity_type: EntityType::parse(&entity_type_str),
            entity_id: row.get("entity_id")?,
            payload: serde_json::from_str(&payload_str).unwrap_or_default(),
            timestamp: DateTime::parse_from_rfc3339(&timestamp_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            actor: row.get("actor")?,
        })
    }
}

/// Types of events that can occur
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    Created,
    Updated,
    Archived,
    PriceObserved,
    DecisionMade,
    ConfigDeployed,
}

impl EventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Updated => "updated",
            Self::Archived => "archived",
            Self::PriceObserved => "price_observed",
            Self::DecisionMade => "decision_made",
            Self::ConfigDeployed => "config_deployed",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "created" => Self::Created,
            "updated" => Self::Updated,
            "archived" => Self::Archived,
            "price_observed" => Self::PriceObserved,
            "decision_made" => Self::DecisionMade,
            "config_deployed" => Self::ConfigDeployed,
            _ => Self::Created,
        }
    }
}

/// Types of entities that events can reference
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    Item,
    Price,
    Configuration,
    Decision,
}

impl EntityType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Item => "item",
            Self::Price => "price",
            Self::Configuration => "configuration",
            Self::Decision => "decision",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "item" => Self::Item,
            "price" => Self::Price,
            "configuration" => Self::Configuration,
            "decision" => Self::Decision,
            _ => Self::Item,
        }
    }
}

/// A decision session with options and outcome
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub id: String,
    pub purpose: String,
    pub status: DecisionStatus,
    pub options: std::collections::HashMap<String, String>, // option name -> config id
    pub chosen_option: Option<String>,
    pub chosen_config_id: Option<String>,
    pub rationale: Option<String>,
    pub decided_at: Option<DateTime<Utc>>,
    pub decided_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub metadata: JsonValue,
}

impl Decision {
    /// Create a new decision session
    pub fn new(purpose: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            purpose: purpose.into(),
            status: DecisionStatus::Active,
            options: std::collections::HashMap::new(),
            chosen_option: None,
            chosen_config_id: None,
            rationale: None,
            decided_at: None,
            decided_by: None,
            created_at: Utc::now(),
            metadata: JsonValue::Object(Default::default()),
        }
    }

    /// Insert into database
    pub fn insert(&self, tx: &Transaction) -> rusqlite::Result<()> {
        tx.execute(
            "INSERT INTO decisions (id, purpose, status, options, chosen_option, chosen_config_id, rationale, decided_at, decided_by, created_at, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                self.id,
                self.purpose,
                self.status.as_str(),
                serde_json::to_string(&self.options).unwrap(),
                self.chosen_option,
                self.chosen_config_id,
                self.rationale,
                self.decided_at.map(|dt| dt.to_rfc3339()),
                self.decided_by,
                self.created_at.to_rfc3339(),
                serde_json::to_string(&self.metadata).unwrap(),
            ],
        )?;
        Ok(())
    }

    /// Load from database row
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let status_str: String = row.get("status")?;
        let options_str: String = row.get("options")?;
        let decided_at_str: Option<String> = row.get("decided_at")?;
        let created_str: String = row.get("created_at")?;
        let metadata_str: String = row.get("metadata")?;

        Ok(Self {
            id: row.get("id")?,
            purpose: row.get("purpose")?,
            status: DecisionStatus::parse(&status_str),
            options: serde_json::from_str(&options_str).unwrap_or_default(),
            chosen_option: row.get("chosen_option")?,
            chosen_config_id: row.get("chosen_config_id")?,
            rationale: row.get("rationale")?,
            decided_at: decided_at_str.and_then(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .ok()
            }),
            decided_by: row.get("decided_by")?,
            created_at: DateTime::parse_from_rfc3339(&created_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            metadata: serde_json::from_str(&metadata_str).unwrap_or_default(),
        })
    }
}

/// Status of a decision session
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DecisionStatus {
    Active,
    Decided,
    Abandoned,
}

impl DecisionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Decided => "decided",
            Self::Abandoned => "abandoned",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "active" => Self::Active,
            "decided" => Self::Decided,
            "abandoned" => Self::Abandoned,
            _ => Self::Active,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_item_new() {
        let item = Item::new("test-1", "Test Item", "ssd");
        assert_eq!(item.id, "test-1");
        assert_eq!(item.name, "Test Item");
        assert_eq!(item.category, "ssd");
        assert!(!item.archived);
    }

    #[test]
    fn test_price_source_roundtrip() {
        assert_eq!(PriceSource::Ebay.as_str(), "ebay");
        assert_eq!(PriceSource::parse("ebay"), PriceSource::Ebay);
        assert_eq!(PriceSource::parse("EBAY"), PriceSource::Ebay);
    }

    #[test]
    fn test_price_source_custom() {
        // Unknown sources should become Custom, not Manual
        let custom = PriceSource::parse("owc");
        assert_eq!(custom, PriceSource::Custom("owc".to_string()));
        assert_eq!(custom.as_str(), "owc");

        // Manual should still work explicitly
        assert_eq!(PriceSource::parse("manual"), PriceSource::Manual);
    }

    #[test]
    fn test_configuration_total_cost() {
        let mut config = Configuration::new("test", "Test Config", "storage");
        config.items.push(ConfigItem {
            item_id: "ssd-1".into(),
            quantity: 2,
            unit_price: Some(100.0),
            notes: None,
        });
        config.items.push(ConfigItem {
            item_id: "enclosure-1".into(),
            quantity: 1,
            unit_price: Some(50.0),
            notes: None,
        });
        assert_eq!(config.total_cost(), 250.0);
    }
}

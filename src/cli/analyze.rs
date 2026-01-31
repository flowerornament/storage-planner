//! sp analyze - Run analysis on configurations

use anyhow::{bail, Result};
use camino::Utf8PathBuf;
use clap::Args;
use console::style;
use serde::Serialize;

use crate::core::db::Database;
use crate::core::models::Configuration;
use crate::core::specs::{get_capacity, get_noise, Capacity};

use super::OutputFormat;

#[derive(Args)]
pub struct AnalyzeArgs {
    /// Configuration ID or name (defaults to current)
    pub config: Option<String>,

    /// Analysis checks to run (redundancy, capacity, noise, cost)
    #[arg(long, short = 'c', value_delimiter = ',')]
    pub check: Option<Vec<String>>,
}

#[derive(Serialize)]
struct AnalysisResult {
    config_id: String,
    config_name: String,
    checks: Vec<Check>,
    summary: Summary,
}

#[derive(Serialize)]
struct Check {
    name: String,
    status: CheckStatus,
    details: serde_json::Value,
}

#[derive(Serialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum CheckStatus {
    Pass,
    Warn,
    Fail,
}

impl CheckStatus {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Warn => "warn",
            Self::Fail => "fail",
        }
    }
}

#[derive(Serialize)]
struct Summary {
    total_items: usize,
    total_cost: f64,
    total_capacity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cost_per_tb: Option<f64>,
    passes: usize,
    warnings: usize,
    failures: usize,
}

pub fn run(db_path: Utf8PathBuf, args: AnalyzeArgs, format: OutputFormat) -> Result<()> {
    if !db_path.exists() {
        bail!("Database not found at {}. Run `sp init` first.", db_path);
    }

    let db = Database::open(&db_path)?;

    // Get configuration
    let config = if let Some(id_or_name) = args.config {
        find_config(&db, &id_or_name)?
    } else {
        // Get current configuration
        db.conn()
            .query_row(
                "SELECT id, name, domain, items, domain_data, metadata, is_current, archived, created_at, updated_at
                 FROM configurations WHERE is_current = 1 LIMIT 1",
                [],
                Configuration::from_row,
            )
            .map_err(|_| anyhow::anyhow!("No current configuration. Specify a config or set one as current."))?
    };

    // Determine which checks to run
    let checks_to_run: Vec<&str> = args
        .check
        .as_ref()
        .map(|c| c.iter().map(|s| s.as_str()).collect())
        .unwrap_or_else(|| vec!["cost", "capacity", "noise", "redundancy"]);

    let mut checks: Vec<Check> = Vec::new();

    // Get item specs for analysis
    let item_specs = get_item_specs(&db, &config)?;

    for check_name in checks_to_run {
        let check = match check_name {
            "cost" => analyze_cost(&config),
            "capacity" => analyze_capacity(&item_specs),
            "noise" => analyze_noise(&item_specs),
            "redundancy" => analyze_redundancy(&config, &item_specs),
            _ => {
                bail!("Unknown check: {}. Available: cost, capacity, noise, redundancy", check_name);
            }
        };
        checks.push(check);
    }

    // Calculate summary
    let total_capacity_bytes: u64 = item_specs
        .iter()
        .filter_map(|(_, specs)| get_capacity(specs))
        .fold(0u64, |acc, cap| acc + cap.bytes);

    let total_cost = config.total_cost();

    // Calculate $/TB if we have both cost and capacity
    let cost_per_tb = if total_cost > 0.0 && total_capacity_bytes > 0 {
        let tb = total_capacity_bytes as f64 / Capacity::TB as f64;
        Some(total_cost / tb)
    } else {
        None
    };

    let summary = Summary {
        total_items: config.items.len(),
        total_cost,
        total_capacity: if total_capacity_bytes > 0 {
            Some(Capacity::from_bytes(total_capacity_bytes).to_string())
        } else {
            None
        },
        cost_per_tb,
        passes: checks.iter().filter(|c| matches!(c.status, CheckStatus::Pass)).count(),
        warnings: checks.iter().filter(|c| matches!(c.status, CheckStatus::Warn)).count(),
        failures: checks.iter().filter(|c| matches!(c.status, CheckStatus::Fail)).count(),
    };

    let result = AnalysisResult {
        config_id: config.id.clone(),
        config_name: config.name.clone(),
        checks,
        summary,
    };

    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&result)?),
        OutputFormat::Yaml => println!("{}", serde_yaml::to_string(&result)?),
        OutputFormat::Text => print_analysis(&result),
    }

    Ok(())
}

fn find_config(db: &Database, id_or_name: &str) -> Result<Configuration> {
    if let Ok(config) = db.conn().query_row(
        "SELECT id, name, domain, items, domain_data, metadata, is_current, archived, created_at, updated_at
         FROM configurations WHERE id = ?1",
        [id_or_name],
        Configuration::from_row,
    ) {
        return Ok(config);
    }

    db.conn()
        .query_row(
            "SELECT id, name, domain, items, domain_data, metadata, is_current, archived, created_at, updated_at
             FROM configurations WHERE name = ?1",
            [id_or_name],
            Configuration::from_row,
        )
        .map_err(|_| anyhow::anyhow!("Configuration '{}' not found", id_or_name))
}

fn get_item_specs(
    db: &Database,
    config: &Configuration,
) -> Result<Vec<(String, serde_json::Value)>> {
    let mut specs = Vec::new();

    for config_item in &config.items {
        let item_specs: String = db
            .conn()
            .query_row(
                "SELECT specs FROM items WHERE id = ?1",
                [&config_item.item_id],
                |row| row.get(0),
            )
            .unwrap_or_else(|_| "{}".to_string());

        let parsed: serde_json::Value = serde_json::from_str(&item_specs).unwrap_or_default();

        // Add multiple times based on quantity
        for _ in 0..config_item.quantity {
            specs.push((config_item.item_id.clone(), parsed.clone()));
        }
    }

    Ok(specs)
}

fn analyze_cost(config: &Configuration) -> Check {
    let total = config.total_cost();
    let items_with_price = config.items.iter().filter(|i| i.unit_price.is_some()).count();
    let items_without_price = config.items.len() - items_with_price;

    let (status, message) = if items_without_price > 0 {
        (
            CheckStatus::Warn,
            format!(
                "{} item(s) missing price data",
                items_without_price
            ),
        )
    } else if total > 0.0 {
        (CheckStatus::Pass, format!("Total: ${:.2}", total))
    } else {
        (CheckStatus::Warn, "No pricing data available".to_string())
    };

    Check {
        name: "cost".to_string(),
        status,
        details: serde_json::json!({
            "total": total,
            "items_with_price": items_with_price,
            "items_without_price": items_without_price,
            "message": message,
        }),
    }
}

fn analyze_capacity(item_specs: &[(String, serde_json::Value)]) -> Check {
    let capacities: Vec<(String, Capacity)> = item_specs
        .iter()
        .filter_map(|(id, specs)| get_capacity(specs).map(|c| (id.clone(), c)))
        .collect();

    let total_bytes: u64 = capacities.iter().map(|(_, c)| c.bytes).sum();

    let status = if capacities.is_empty() {
        CheckStatus::Warn
    } else {
        CheckStatus::Pass
    };

    Check {
        name: "capacity".to_string(),
        status,
        details: serde_json::json!({
            "total": Capacity::from_bytes(total_bytes).to_string(),
            "total_bytes": total_bytes,
            "items": capacities.iter().map(|(id, c)| {
                serde_json::json!({
                    "item_id": id,
                    "capacity": c.to_string(),
                })
            }).collect::<Vec<_>>(),
        }),
    }
}

fn analyze_noise(item_specs: &[(String, serde_json::Value)]) -> Check {
    let noise_levels: Vec<(String, f64)> = item_specs
        .iter()
        .filter_map(|(id, specs)| get_noise(specs).map(|n| (id.clone(), n.db)))
        .collect();

    // Maximum noise in the system
    let max_noise = noise_levels
        .iter()
        .map(|(_, db)| *db)
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap_or(0.0);

    // Check against common threshold (30 dB for quiet operation)
    let threshold = 30.0;
    let (status, message) = if noise_levels.is_empty() {
        (CheckStatus::Warn, "No noise data available".to_string())
    } else if max_noise <= threshold {
        (
            CheckStatus::Pass,
            format!("Max noise {:.0}dB within {:.0}dB threshold", max_noise, threshold),
        )
    } else {
        (
            CheckStatus::Warn,
            format!("Max noise {:.0}dB exceeds {:.0}dB threshold", max_noise, threshold),
        )
    };

    Check {
        name: "noise".to_string(),
        status,
        details: serde_json::json!({
            "max_noise_db": max_noise,
            "threshold_db": threshold,
            "within_threshold": max_noise <= threshold,
            "message": message,
            "items": noise_levels.iter().map(|(id, db)| {
                serde_json::json!({
                    "item_id": id,
                    "noise_db": db,
                })
            }).collect::<Vec<_>>(),
        }),
    }
}

fn analyze_redundancy(config: &Configuration, item_specs: &[(String, serde_json::Value)]) -> Check {
    // Count storage devices
    let storage_items: Vec<&str> = item_specs
        .iter()
        .filter(|(_, specs)| get_capacity(specs).is_some())
        .map(|(id, _)| id.as_str())
        .collect();

    let storage_count = storage_items.len();

    // Simple redundancy check: need at least 2 drives for any redundancy
    let (status, message) = if storage_count == 0 {
        (CheckStatus::Warn, "No storage devices found".to_string())
    } else if storage_count == 1 {
        (
            CheckStatus::Fail,
            "Single point of failure: only 1 storage device".to_string(),
        )
    } else if storage_count == 2 {
        (
            CheckStatus::Pass,
            "Mirror redundancy possible with 2 devices".to_string(),
        )
    } else {
        (
            CheckStatus::Pass,
            format!("{} devices: RAID5+ or multiple mirrors possible", storage_count),
        )
    };

    Check {
        name: "redundancy".to_string(),
        status,
        details: serde_json::json!({
            "storage_device_count": storage_count,
            "storage_items": storage_items,
            "message": message,
            "domain_data": config.domain_data,
        }),
    }
}

fn print_analysis(result: &AnalysisResult) {
    println!(
        "{} {}",
        style("Analysis:").bold().cyan(),
        result.config_name
    );
    println!("{}", style("═".repeat(50)).dim());
    println!();

    // Summary
    println!("{}", style("Summary:").bold());
    println!("  Items: {}", result.summary.total_items);
    if result.summary.total_cost > 0.0 {
        println!("  Cost:  ${:.2}", result.summary.total_cost);
    }
    if let Some(ref cap) = result.summary.total_capacity {
        println!("  Capacity: {}", cap);
    }
    if let Some(cost_per_tb) = result.summary.cost_per_tb {
        println!("  $/TB: ${:.2}", cost_per_tb);
    }
    println!();

    // Checks
    println!("{}", style("Checks:").bold());
    for check in &result.checks {
        let status_icon = match check.status {
            CheckStatus::Pass => style("✓").green(),
            CheckStatus::Warn => style("!").yellow(),
            CheckStatus::Fail => style("✗").red(),
        };

        let message = check
            .details
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        println!(
            "  {} {:<12} {}",
            status_icon,
            check.name,
            style(message).dim()
        );
    }
    println!();

    // Overall status
    if result.summary.failures > 0 {
        println!(
            "{} {} failure(s) found",
            style("✗").red(),
            result.summary.failures
        );
    } else if result.summary.warnings > 0 {
        println!(
            "{} {} warning(s), no failures",
            style("!").yellow(),
            result.summary.warnings
        );
    } else {
        println!("{} All checks passed", style("✓").green());
    }
}

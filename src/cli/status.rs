//! sp status -- System health overview dashboard (CTX-01)
//! sp current -- Show/set current topology shortcut (CTX-03)
//!
//! Status surfaces problems first, then topology details, open decisions,
//! catalog stats, and recent activity. Designed for both humans and agents.
//!
//! Current provides a quick way to check or change the active topology.

use anyhow::Result;
use console::style;
use rusqlite::params;

use crate::core::db::Database;
use crate::core::events::{record_event, EventSource};
use crate::core::models::{Dataset, Volume};
use crate::core::resolve::{resolve_active_topology, resolve_topology};
use crate::domains::storage::analysis::{
    analyze_capacity, analyze_redundancy, load_placements_with_context,
};

use super::OutputFormat;

// ===========================================================================
// sp status
// ===========================================================================

/// Run the status dashboard. Surfaces problems first, then context.
pub fn run_status(db: &mut Database, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Text => run_status_text(db),
        OutputFormat::Json => run_status_json(db),
    }
}

fn run_status_text(db: &mut Database) -> Result<()> {
    // --- Section 1: Problems (only if any exist) ---
    let problems = gather_problems(db)?;

    if !problems.alerts.is_empty() {
        println!("{}", style("Problems").red().bold());
        for alert in &problems.alerts {
            println!("  {} {}", style("!").red().bold(), alert);
        }
        println!();
    }

    // --- Section 2: Current Topology ---
    println!("{}", style("Current Topology").bold());
    match resolve_active_topology(db, None).ok() {
        Some(topo) => {
            let desc = if topo.description.is_empty() {
                String::new()
            } else if topo.description.len() > 60 {
                format!(" -- {}...", &topo.description[..57])
            } else {
                format!(" -- {}", topo.description)
            };

            let tag_str = topo
                .tag
                .as_deref()
                .map(|t| format!(" [{}]", t))
                .unwrap_or_default();

            println!("  {}{}{}", style(&topo.name).bold(), tag_str, desc);

            // Entity counts
            let counts = count_topology_entities(db, &topo.id)?;
            println!(
                "  {} nodes, {} volumes, {} datasets, {} placements, {} links, {} sync regimes",
                counts.nodes,
                counts.volumes,
                counts.datasets,
                counts.placements,
                counts.links,
                counts.sync_regimes,
            );

            // Quick analysis scores
            if counts.datasets > 0 {
                let datasets = Dataset::load_for_topology(db, &topo.id)?;
                let volumes = Volume::load_for_topology(db, &topo.id)?;
                let placements = load_placements_with_context(db, &topo.id)?;

                let redundancy = analyze_redundancy(&datasets, &placements);
                let capacity = analyze_capacity(&datasets, &volumes, &placements, 12);

                println!(
                    "  Redundancy: {:.0}% | Capacity: {:.0}%",
                    redundancy.score, capacity.score,
                );
            }
        }
        None => {
            println!("  No current topology set. Use 'sp current <name>' to set one.");
        }
    }
    println!();

    // --- Section 3: Open Decisions ---
    println!("{}", style("Open Decisions").bold());
    let decisions = load_open_decisions(db)?;
    if decisions.is_empty() {
        println!("  No open decisions");
    } else {
        println!(
            "  {:<30} {:<10} {:<14} {:<12} Age",
            "Title", "Status", "Constraints", "Topologies"
        );
        for d in &decisions {
            println!(
                "  {:<30} {:<10} {:<14} {:<12} {}",
                truncate(&d.title, 28),
                d.status,
                format!("{}", d.constraint_count),
                format!("{}", d.topology_count),
                format_age(d.age_days),
            );
        }
    }
    println!();

    // --- Section 4: Catalog Stats ---
    println!("{}", style("Catalog").bold());
    let catalog = load_catalog_stats(db)?;
    if catalog.total_items == 0 {
        println!("  No catalog items. Add one with 'sp catalog add <name> --category=<cat>'");
    } else {
        println!(
            "  {} items, {} price observations",
            catalog.total_items, catalog.total_prices
        );
        if !catalog.categories.is_empty() {
            let cat_str: Vec<String> = catalog
                .categories
                .iter()
                .map(|(cat, count)| format!("{}: {}", cat, count))
                .collect();
            println!("  Categories: {}", cat_str.join(", "));
        }
        if let Some(ref latest) = catalog.latest_price_date {
            println!("  Latest price: {}", latest);
        }
    }
    println!();

    // --- Section 5: Recent Activity ---
    println!("{}", style("Recent Activity").bold());
    let events = load_recent_events(db)?;
    if events.is_empty() {
        println!("  No recent activity");
    } else {
        for evt in &events {
            println!("  {} {}", style(&evt.timestamp).dim(), evt.summary);
        }
    }

    Ok(())
}

fn run_status_json(db: &mut Database) -> Result<()> {
    let problems = gather_problems(db)?;

    let current_topo = resolve_active_topology(db, None).ok();
    let topology_json = match &current_topo {
        Some(topo) => {
            let counts = count_topology_entities(db, &topo.id)?;
            let (redundancy_score, capacity_score) = if counts.datasets > 0 {
                let datasets = Dataset::load_for_topology(db, &topo.id)?;
                let volumes = Volume::load_for_topology(db, &topo.id)?;
                let placements = load_placements_with_context(db, &topo.id)?;
                let redundancy = analyze_redundancy(&datasets, &placements);
                let capacity = analyze_capacity(&datasets, &volumes, &placements, 12);
                (Some(redundancy.score), Some(capacity.score))
            } else {
                (None, None)
            };

            serde_json::json!({
                "name": topo.name,
                "id": topo.id,
                "tag": topo.tag,
                "description": topo.description,
                "counts": {
                    "nodes": counts.nodes,
                    "volumes": counts.volumes,
                    "datasets": counts.datasets,
                    "placements": counts.placements,
                    "links": counts.links,
                    "sync_regimes": counts.sync_regimes,
                },
                "redundancy_score": redundancy_score,
                "capacity_score": capacity_score,
            })
        }
        None => serde_json::json!(null),
    };

    let decisions: Vec<serde_json::Value> = load_open_decisions(db)?
        .iter()
        .map(|d| {
            serde_json::json!({
                "title": d.title,
                "status": d.status,
                "constraint_count": d.constraint_count,
                "topology_count": d.topology_count,
                "age_days": d.age_days,
            })
        })
        .collect();

    let catalog = load_catalog_stats(db)?;
    let events: Vec<serde_json::Value> = load_recent_events(db)?
        .iter()
        .map(|e| {
            serde_json::json!({
                "timestamp": e.timestamp,
                "summary": e.summary,
            })
        })
        .collect();

    let json = serde_json::json!({
        "problems": problems.alerts,
        "current_topology": topology_json,
        "open_decisions": decisions,
        "catalog": {
            "total_items": catalog.total_items,
            "total_prices": catalog.total_prices,
            "categories": catalog.categories,
            "latest_price_date": catalog.latest_price_date,
        },
        "recent_activity": events,
    });

    println!("{}", serde_json::to_string_pretty(&json)?);
    Ok(())
}

// ===========================================================================
// sp current
// ===========================================================================

/// Run the current topology command. No args = show, with arg = set.
pub fn run_current(
    db: &mut Database,
    topology_name: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    match topology_name {
        None => show_current(db, format),
        Some(name) => set_current(db, name, format),
    }
}

fn show_current(db: &mut Database, format: OutputFormat) -> Result<()> {
    match resolve_active_topology(db, None).ok() {
        Some(topo) => match format {
            OutputFormat::Text => {
                println!("{} ({})", topo.name, &topo.id[..8]);
            }
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&topo)?);
            }
        },
        None => match format {
            OutputFormat::Text => {
                println!("No current topology set. Use 'sp current <name>' to set one.");
            }
            OutputFormat::Json => {
                println!("null");
            }
        },
    }
    Ok(())
}

fn set_current(db: &mut Database, name: &str, format: OutputFormat) -> Result<()> {
    let topo = resolve_topology(db, name)?;
    let topo_name = topo.name.clone();
    let topo_id = topo.id.clone();

    if topo.tag.as_deref() == Some("current") {
        match format {
            OutputFormat::Text => {
                println!("'{}' is already the current topology", topo_name);
            }
            OutputFormat::Json => {
                let json = serde_json::json!({
                    "action": "unchanged",
                    "topology": topo_name,
                    "id": topo_id,
                });
                println!("{}", serde_json::to_string_pretty(&json)?);
            }
        }
        return Ok(());
    }

    let before_json = topo.to_json()?;

    db.transaction(|tx| {
        // Clear any existing current tag (per D020)
        tx.execute(
            "UPDATE topologies SET tag = NULL, updated_at = datetime('now') WHERE tag = 'current'",
            [],
        )?;

        // Tag the target as current
        tx.execute(
            "UPDATE topologies SET tag = 'current', updated_at = datetime('now') WHERE id = ?1",
            [&topo_id],
        )?;

        // Build after state
        let mut after = topo.clone();
        after.tag = Some("current".to_string());
        let after_json = after.to_json()?;

        record_event(
            tx,
            "topology.updated",
            "topology",
            &topo_id,
            &format!("Set topology '{}' as current via sp current", topo_name),
            Some(&before_json),
            Some(&after_json),
            &EventSource::User,
        )?;

        Ok(())
    })?;

    match format {
        OutputFormat::Text => {
            println!("Switched to '{}'", topo_name);
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "action": "switched",
                "topology": topo_name,
                "id": topo_id,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

// ===========================================================================
// Problem gathering
// ===========================================================================

struct Problems {
    alerts: Vec<String>,
}

fn gather_problems(db: &mut Database) -> Result<Problems> {
    let mut alerts = Vec::new();

    // Only analyze if there's a current topology
    if let Ok(topo) = resolve_active_topology(db, None) {
        let datasets = Dataset::load_for_topology(db, &topo.id)?;
        let volumes = Volume::load_for_topology(db, &topo.id)?;
        let placements = load_placements_with_context(db, &topo.id)?;

        // Redundancy: datasets with insufficient copies/locations
        if !datasets.is_empty() {
            let redundancy = analyze_redundancy(&datasets, &placements);
            if !redundancy.issues.is_empty() {
                alerts.push(format!(
                    "{} dataset(s) at risk (redundancy issues)",
                    redundancy.issues.len()
                ));
            }

            // Capacity: volumes projected full within 6 months
            let capacity = analyze_capacity(&datasets, &volumes, &placements, 6);
            if !capacity.issues.is_empty() {
                for issue in &capacity.issues {
                    alerts.push(format!(
                        "Volume '{}' on '{}' projected full in {:.0} months",
                        issue.volume_name, issue.node_name, issue.months_until_full
                    ));
                }
            }
        }
    }

    // Decisions open 30+ days
    let stale_decisions = count_stale_decisions(db)?;
    if stale_decisions > 0 {
        alerts.push(format!("{} decision(s) open for 30+ days", stale_decisions));
    }

    Ok(Problems { alerts })
}

struct EntityCounts {
    nodes: i64,
    volumes: i64,
    datasets: i64,
    placements: i64,
    links: i64,
    sync_regimes: i64,
}

fn count_topology_entities(db: &Database, topology_id: &str) -> Result<EntityCounts> {
    let nodes: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM nodes WHERE topology_id = ?1",
        params![topology_id],
        |row| row.get(0),
    )?;
    let volumes: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM volumes WHERE topology_id = ?1",
        params![topology_id],
        |row| row.get(0),
    )?;
    let datasets: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM datasets WHERE topology_id = ?1",
        params![topology_id],
        |row| row.get(0),
    )?;
    let placements: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM placements WHERE topology_id = ?1",
        params![topology_id],
        |row| row.get(0),
    )?;
    let links: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM links WHERE topology_id = ?1",
        params![topology_id],
        |row| row.get(0),
    )?;
    let sync_regimes: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM sync_regimes WHERE topology_id = ?1",
        params![topology_id],
        |row| row.get(0),
    )?;

    Ok(EntityCounts {
        nodes,
        volumes,
        datasets,
        placements,
        links,
        sync_regimes,
    })
}

struct OpenDecision {
    title: String,
    status: String,
    constraint_count: i64,
    topology_count: i64,
    age_days: i64,
}

fn load_open_decisions(db: &Database) -> Result<Vec<OpenDecision>> {
    let mut stmt = db.conn().prepare(
        "SELECT d.title, d.status, d.created_at,
                (SELECT COUNT(*) FROM decision_constraints WHERE decision_id = d.id) AS constraint_count,
                (SELECT COUNT(*) FROM decision_topologies WHERE decision_id = d.id) AS topology_count
         FROM decisions d
         WHERE d.status IN ('draft', 'open')
         ORDER BY d.created_at",
    )?;

    let results = stmt
        .query_map([], |row| {
            let created_str: String = row.get("created_at")?;
            let age_days = chrono::DateTime::parse_from_rfc3339(&created_str)
                .map(|dt| (chrono::Utc::now() - dt.with_timezone(&chrono::Utc)).num_days())
                .unwrap_or(0);

            Ok(OpenDecision {
                title: row.get("title")?,
                status: row.get("status")?,
                constraint_count: row.get("constraint_count")?,
                topology_count: row.get("topology_count")?,
                age_days,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(results)
}

fn count_stale_decisions(db: &Database) -> Result<i64> {
    let count: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM decisions
         WHERE status IN ('draft', 'open')
         AND julianday('now') - julianday(created_at) >= 30",
        [],
        |row| row.get(0),
    )?;
    Ok(count)
}

struct CatalogStats {
    total_items: i64,
    total_prices: i64,
    categories: Vec<(String, i64)>,
    latest_price_date: Option<String>,
}

fn load_catalog_stats(db: &Database) -> Result<CatalogStats> {
    let total_items: i64 =
        db.conn()
            .query_row("SELECT COUNT(*) FROM catalog_items", [], |row| row.get(0))?;

    let total_prices: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM prices", [], |row| row.get(0))?;

    let categories: Vec<(String, i64)> = {
        let mut stmt = db.conn().prepare(
            "SELECT category, COUNT(*) as cnt FROM catalog_items GROUP BY category ORDER BY cnt DESC",
        )?;
        let result = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        result
    };

    let latest_price_date: Option<String> = db
        .conn()
        .query_row(
            "SELECT observed_at FROM prices ORDER BY observed_at DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .ok();

    Ok(CatalogStats {
        total_items,
        total_prices,
        categories,
        latest_price_date,
    })
}

struct RecentEvent {
    timestamp: String,
    summary: String,
}

fn load_recent_events(db: &Database) -> Result<Vec<RecentEvent>> {
    let mut stmt = db
        .conn()
        .prepare("SELECT timestamp, summary FROM events ORDER BY sequence DESC LIMIT 5")?;

    let results = stmt
        .query_map([], |row| {
            Ok(RecentEvent {
                timestamp: row.get(0)?,
                summary: row.get(1)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(results)
}

// ===========================================================================
// Helpers
// ===========================================================================

fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}...", &s[..max.saturating_sub(3)])
    } else {
        s.to_string()
    }
}

fn format_age(days: i64) -> String {
    if days == 0 {
        "today".to_string()
    } else if days == 1 {
        "1 day".to_string()
    } else {
        format!("{} days", days)
    }
}

//! sp dataset -- Manage logical datasets with replication requirements
//!
//! Subcommands: add, list, show, remove, update
//! All mutating commands log events for undo/redo support.
//! All lookups support name-or-ID resolution via the entity resolver.

use anyhow::{bail, Result};
use clap::Subcommand;
use rusqlite::params;

use crate::core::db::Database;
use crate::core::events::{record_event, EventSource};
use crate::core::models::Dataset;
use crate::core::resolve::{resolve_active_topology, resolve_dataset, validate_slug};
use crate::core::specs::Capacity;

use super::OutputFormat;

#[derive(Subcommand)]
pub enum DatasetCommands {
    /// Add a dataset to the active topology
    Add {
        /// Dataset name (must be unique within topology)
        name: String,

        /// Current size (e.g., "500GB", "2TB")
        #[arg(long)]
        size: String,

        /// Criticality level (normal, important, critical)
        #[arg(long, default_value = "normal")]
        criticality: String,

        /// Minimum number of copies required
        #[arg(long, default_value_t = 1)]
        min_copies: i32,

        /// Minimum number of distinct locations
        #[arg(long, default_value_t = 1)]
        min_locations: i32,

        /// Maximum recovery point objective in hours
        #[arg(long)]
        max_rpo: Option<i32>,

        /// Growth rate per month (e.g., "10GB", "500MB")
        #[arg(long)]
        growth_rate: Option<String>,

        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },

    /// List datasets in the active topology
    List {
        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },

    /// Show details of a specific dataset
    Show {
        /// Dataset name or ID
        name: String,

        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },

    /// Remove a dataset from the active topology
    Remove {
        /// Dataset name or ID to remove
        name: String,

        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },

    /// Update a dataset's properties
    Update {
        /// Dataset name or ID
        name: String,

        /// Rename the dataset (must be a valid slug)
        #[arg(long)]
        rename: Option<String>,

        /// New size (e.g., "1TB", "750GB")
        #[arg(long)]
        size: Option<String>,

        /// New criticality level (normal, important, critical)
        #[arg(long)]
        criticality: Option<String>,

        /// New minimum copies
        #[arg(long)]
        min_copies: Option<i32>,

        /// New minimum locations
        #[arg(long)]
        min_locations: Option<i32>,

        /// New max RPO in hours
        #[arg(long)]
        max_rpo: Option<i32>,

        /// New growth rate per month (e.g., "10GB")
        #[arg(long)]
        growth_rate: Option<String>,

        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },
}

pub fn run(cmd: DatasetCommands, db: &mut Database, format: OutputFormat) -> Result<()> {
    match cmd {
        DatasetCommands::Add {
            name,
            size,
            criticality,
            min_copies,
            min_locations,
            max_rpo,
            growth_rate,
            topology,
        } => add(
            db,
            &name,
            &size,
            &criticality,
            min_copies,
            min_locations,
            max_rpo,
            growth_rate.as_deref(),
            topology.as_deref(),
            format,
        ),
        DatasetCommands::List { topology } => list(db, topology.as_deref(), format),
        DatasetCommands::Show { name, topology } => show(db, &name, topology.as_deref(), format),
        DatasetCommands::Remove { name, topology } => {
            remove(db, &name, topology.as_deref(), format)
        }
        DatasetCommands::Update {
            name,
            rename,
            size,
            criticality,
            min_copies,
            min_locations,
            max_rpo,
            growth_rate,
            topology,
        } => update(
            db,
            &name,
            rename.as_deref(),
            size.as_deref(),
            criticality.as_deref(),
            min_copies,
            min_locations,
            max_rpo,
            growth_rate.as_deref(),
            topology.as_deref(),
            format,
        ),
    }
}

/// Validate that a criticality value is one of the allowed values.
fn validate_criticality(value: &str) -> Result<()> {
    match value {
        "normal" | "important" | "critical" => Ok(()),
        _ => bail!(
            "Invalid criticality '{}': must be one of: normal, important, critical",
            value
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn add(
    db: &mut Database,
    name: &str,
    size: &str,
    criticality: &str,
    min_copies: i32,
    min_locations: i32,
    max_rpo: Option<i32>,
    growth_rate: Option<&str>,
    topology_override: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    validate_slug(name)?;
    validate_criticality(criticality)?;

    let capacity = Capacity::parse(size)?;
    let size_bytes = capacity.bytes as i64;

    let growth_bytes: Option<f64> = growth_rate
        .map(|g| Capacity::parse(g).map(|c| c.bytes as f64))
        .transpose()?;

    let topo = resolve_active_topology(db, topology_override)?;

    // Pre-insert uniqueness check
    let existing: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM datasets WHERE topology_id = ?1 AND name = ?2",
        params![topo.id, name],
        |row| row.get(0),
    )?;
    if existing > 0 {
        bail!("Dataset '{}' already exists in topology '{}'", name, topo.name);
    }

    let mut dataset = Dataset::new(&topo.id, name, size_bytes);
    dataset.criticality = criticality.to_string();
    dataset.min_copies = min_copies;
    dataset.min_locations = min_locations;
    dataset.max_rpo_hours = max_rpo;
    dataset.growth_rate_bytes_month = growth_bytes;

    let after_json = dataset.to_json()?;
    let ds_id = dataset.id.clone();
    let ds_name = dataset.name.clone();

    db.transaction(|tx| {
        dataset.insert(tx)?;

        record_event(
            tx,
            "dataset.created",
            "dataset",
            &ds_id,
            &format!("Created dataset '{}'", ds_name),
            None,
            Some(&after_json),
            &EventSource::User,
        )?;

        Ok(())
    })?;

    let size_formatted = Capacity::from_bytes(size_bytes as u64);
    let id_prefix = &ds_id[..8];

    match format {
        OutputFormat::Text => {
            println!(
                "Created dataset '{}' ({}, {}) (id: {})",
                name, size_formatted, criticality, id_prefix
            );
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "action": "created",
                "dataset": name,
                "id": ds_id,
                "size_bytes": size_bytes,
                "criticality": criticality,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

fn list(db: &mut Database, topology_override: Option<&str>, format: OutputFormat) -> Result<()> {
    let topo = resolve_active_topology(db, topology_override)?;

    let mut stmt = db.conn().prepare(
        "SELECT id, topology_id, name, size_bytes, growth_rate_bytes_month, criticality, \
         min_copies, min_locations, max_rpo_hours, created_at, updated_at \
         FROM datasets WHERE topology_id = ?1 ORDER BY name",
    )?;

    let datasets: Vec<Dataset> = stmt
        .query_map(params![topo.id], Dataset::from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    match format {
        OutputFormat::Text => {
            if datasets.is_empty() {
                println!("No datasets found. Add one with 'sp dataset add <name> --size=<size>'");
            } else {
                for ds in &datasets {
                    let cap = Capacity::from_bytes(ds.size_bytes as u64);
                    let copies_info = if ds.min_copies > 1 {
                        format!(" ({}x copies)", ds.min_copies)
                    } else {
                        String::new()
                    };
                    println!("  {}: {} [{}]{}", ds.name, cap, ds.criticality, copies_info);
                }
            }
        }
        OutputFormat::Json => {
            let json: Vec<serde_json::Value> = datasets
                .iter()
                .map(|d| {
                    serde_json::json!({
                        "id": d.id,
                        "name": d.name,
                        "size_bytes": d.size_bytes,
                        "criticality": d.criticality,
                        "min_copies": d.min_copies,
                        "min_locations": d.min_locations,
                        "max_rpo_hours": d.max_rpo_hours,
                        "growth_rate_bytes_month": d.growth_rate_bytes_month,
                        "created_at": d.created_at.to_rfc3339(),
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

fn show(
    db: &mut Database,
    name: &str,
    topology_override: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    let topo = resolve_active_topology(db, topology_override)?;
    let ds = resolve_dataset(db, &topo.id, name)?;

    // Query placements for this dataset, joined with volumes and nodes
    let mut stmt = db.conn().prepare(
        "SELECT p.id, p.role, p.priority, v.name AS volume_name, n.name AS node_name, \
         v.capacity_bytes \
         FROM placements p \
         JOIN volumes v ON p.volume_id = v.id \
         JOIN nodes n ON v.node_id = n.id \
         WHERE p.dataset_id = ?1 \
         ORDER BY p.priority DESC, p.role",
    )?;

    let placements: Vec<(String, String, i32, String, String, i64)> = stmt
        .query_map(params![ds.id], |row| {
            Ok((
                row.get::<_, String>("id")?,
                row.get::<_, String>("role")?,
                row.get::<_, i32>("priority")?,
                row.get::<_, String>("volume_name")?,
                row.get::<_, String>("node_name")?,
                row.get::<_, i64>("capacity_bytes")?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    match format {
        OutputFormat::Text => {
            let cap = Capacity::from_bytes(ds.size_bytes as u64);
            println!("Dataset: {} [{}]", ds.name, ds.criticality);
            println!("  Size:          {}", cap);
            if let Some(growth) = ds.growth_rate_bytes_month {
                let growth_cap = Capacity::from_bytes(growth as u64);
                println!("  Growth rate:   {}/month", growth_cap);
            }
            println!("  Min copies:    {}", ds.min_copies);
            println!("  Min locations: {}", ds.min_locations);
            if let Some(rpo) = ds.max_rpo_hours {
                println!("  Max RPO:       {}h", rpo);
            }
            println!("  ID:            {}", ds.id);
            println!(
                "  Created:       {}",
                ds.created_at.format("%Y-%m-%d %H:%M:%S")
            );

            if placements.is_empty() {
                println!("\n  Placements: none");
            } else {
                println!("\n  Placements:");
                for (_, role, priority, vol_name, node_name, _) in &placements {
                    println!(
                        "    {} on {} [{}] (priority: {})",
                        vol_name, node_name, role, priority
                    );
                }
            }
        }
        OutputFormat::Json => {
            let placement_json: Vec<serde_json::Value> = placements
                .iter()
                .map(|(id, role, priority, vol_name, node_name, _)| {
                    serde_json::json!({
                        "id": id,
                        "volume": vol_name,
                        "node": node_name,
                        "role": role,
                        "priority": priority,
                    })
                })
                .collect();

            let json = serde_json::json!({
                "id": ds.id,
                "name": ds.name,
                "size_bytes": ds.size_bytes,
                "criticality": ds.criticality,
                "min_copies": ds.min_copies,
                "min_locations": ds.min_locations,
                "max_rpo_hours": ds.max_rpo_hours,
                "growth_rate_bytes_month": ds.growth_rate_bytes_month,
                "placements": placement_json,
                "created_at": ds.created_at.to_rfc3339(),
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

fn remove(
    db: &mut Database,
    name: &str,
    topology_override: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    let topo = resolve_active_topology(db, topology_override)?;
    let ds = resolve_dataset(db, &topo.id, name)?;

    let before_json = ds.to_json()?;
    let ds_id = ds.id.clone();
    let ds_name = ds.name.clone();

    // Count dependents for informational output
    let placement_count: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM placements WHERE dataset_id = ?1",
        [&ds_id],
        |row| row.get(0),
    )?;
    let sync_count: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM sync_regimes WHERE dataset_id = ?1",
        [&ds_id],
        |row| row.get(0),
    )?;

    db.transaction(|tx| {
        tx.execute("DELETE FROM datasets WHERE id = ?1", [&ds_id])?;

        record_event(
            tx,
            "dataset.deleted",
            "dataset",
            &ds_id,
            &format!("Deleted dataset '{}'", ds_name),
            Some(&before_json),
            None,
            &EventSource::User,
        )?;

        Ok(())
    })?;

    match format {
        OutputFormat::Text => {
            let mut msg = format!("Deleted dataset '{}'", ds_name);
            if placement_count > 0 || sync_count > 0 {
                msg.push_str(&format!(
                    " (cascaded: {} placements, {} sync regimes)",
                    placement_count, sync_count
                ));
            }
            println!("{}", msg);
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "action": "deleted",
                "dataset": ds_name,
                "id": ds_id,
                "cascaded_placements": placement_count,
                "cascaded_sync_regimes": sync_count,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn update(
    db: &mut Database,
    name: &str,
    rename: Option<&str>,
    size: Option<&str>,
    criticality: Option<&str>,
    min_copies: Option<i32>,
    min_locations: Option<i32>,
    max_rpo: Option<i32>,
    growth_rate: Option<&str>,
    topology_override: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    if rename.is_none()
        && size.is_none()
        && criticality.is_none()
        && min_copies.is_none()
        && min_locations.is_none()
        && max_rpo.is_none()
        && growth_rate.is_none()
    {
        bail!("Nothing to update. Provide at least one field to change.");
    }

    // Validate inputs
    if let Some(new_name) = rename {
        validate_slug(new_name)?;
    }
    if let Some(crit) = criticality {
        validate_criticality(crit)?;
    }

    // Parse size/growth if provided
    let new_size_bytes: Option<i64> = size
        .map(|s| Capacity::parse(s).map(|c| c.bytes as i64))
        .transpose()?;
    let new_growth: Option<f64> = growth_rate
        .map(|g| Capacity::parse(g).map(|c| c.bytes as f64))
        .transpose()?;

    // Resolve outside transaction
    let topo = resolve_active_topology(db, topology_override)?;
    let ds = resolve_dataset(db, &topo.id, name)?;
    let before_json = ds.to_json()?;
    let ds_id = ds.id.clone();
    let original_name = ds.name.clone();

    // Check uniqueness of new name if renaming
    if let Some(new_name) = rename {
        if new_name != original_name {
            let existing: i64 = db.conn().query_row(
                "SELECT COUNT(*) FROM datasets WHERE topology_id = ?1 AND name = ?2 AND id != ?3",
                params![topo.id, new_name, ds_id],
                |row| row.get(0),
            )?;
            if existing > 0 {
                bail!(
                    "Dataset name '{}' is already taken in this topology",
                    new_name
                );
            }
        }
    }

    // Build the updated state for after_json
    let final_name = rename.unwrap_or(&original_name).to_string();
    let final_size = new_size_bytes.unwrap_or(ds.size_bytes);
    let final_criticality = criticality
        .map(|c| c.to_string())
        .unwrap_or_else(|| ds.criticality.clone());
    let final_min_copies = min_copies.unwrap_or(ds.min_copies);
    let final_min_locations = min_locations.unwrap_or(ds.min_locations);
    let final_max_rpo = if max_rpo.is_some() {
        max_rpo
    } else {
        ds.max_rpo_hours
    };
    let final_growth = if new_growth.is_some() {
        new_growth
    } else {
        ds.growth_rate_bytes_month
    };

    db.transaction(|tx| {
        // Apply updates dynamically
        if let Some(new_name) = rename {
            tx.execute(
                "UPDATE datasets SET name = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![new_name, ds_id],
            )?;
        }
        if let Some(bytes) = new_size_bytes {
            tx.execute(
                "UPDATE datasets SET size_bytes = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![bytes, ds_id],
            )?;
        }
        if let Some(crit) = criticality {
            tx.execute(
                "UPDATE datasets SET criticality = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![crit, ds_id],
            )?;
        }
        if let Some(copies) = min_copies {
            tx.execute(
                "UPDATE datasets SET min_copies = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![copies, ds_id],
            )?;
        }
        if let Some(locations) = min_locations {
            tx.execute(
                "UPDATE datasets SET min_locations = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![locations, ds_id],
            )?;
        }
        if let Some(rpo) = max_rpo {
            tx.execute(
                "UPDATE datasets SET max_rpo_hours = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![rpo, ds_id],
            )?;
        }
        if let Some(growth) = new_growth {
            tx.execute(
                "UPDATE datasets SET growth_rate_bytes_month = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![growth, ds_id],
            )?;
        }

        // Build after state
        let mut after = ds.clone();
        after.name = final_name.clone();
        after.size_bytes = final_size;
        after.criticality = final_criticality.clone();
        after.min_copies = final_min_copies;
        after.min_locations = final_min_locations;
        after.max_rpo_hours = final_max_rpo;
        after.growth_rate_bytes_month = final_growth;
        let after_json = after.to_json()?;

        record_event(
            tx,
            "dataset.updated",
            "dataset",
            &ds_id,
            &format!("Updated dataset '{}'", original_name),
            Some(&before_json),
            Some(&after_json),
            &EventSource::User,
        )?;

        Ok(())
    })?;

    match format {
        OutputFormat::Text => {
            if rename.is_some() {
                println!(
                    "Updated dataset '{}': renamed to '{}'",
                    original_name, final_name
                );
            } else {
                println!("Updated dataset '{}'", original_name);
            }
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "action": "updated",
                "dataset": final_name,
                "id": ds_id,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

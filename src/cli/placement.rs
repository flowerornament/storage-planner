//! sp placement -- Manage dataset placements on volumes
//!
//! Subcommands: add, list, remove
//! All mutating commands log events for undo/redo support.
//! All lookups support name-or-ID resolution via the entity resolver.

use anyhow::{bail, Result};
use clap::Subcommand;
use rusqlite::params;

use crate::core::db::Database;
use crate::core::events::{record_event, EventSource};
use crate::core::models::Placement;
use crate::core::resolve::{resolve_active_topology, resolve_dataset, resolve_volume};
use crate::core::specs::Capacity;

use super::OutputFormat;

#[derive(Subcommand)]
pub enum PlacementCommands {
    /// Place a dataset on a volume
    Add {
        /// Dataset name or ID
        dataset: String,

        /// Volume name or ID
        volume: String,

        /// Node to disambiguate volume (if name is shared across nodes)
        #[arg(long)]
        node: Option<String>,

        /// Placement role (primary, replica, backup, archive)
        #[arg(long, default_value = "primary")]
        role: String,

        /// Priority (higher = preferred for reads)
        #[arg(long, default_value_t = 0)]
        priority: i32,

        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },

    /// List placements in the active topology
    List {
        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },

    /// Remove a dataset placement from a volume
    Remove {
        /// Dataset name or ID
        dataset: String,

        /// Volume name or ID
        volume: String,

        /// Node to disambiguate volume
        #[arg(long)]
        node: Option<String>,

        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },
}

pub fn run(cmd: PlacementCommands, db: &mut Database, format: OutputFormat) -> Result<()> {
    match cmd {
        PlacementCommands::Add {
            dataset,
            volume,
            node,
            role,
            priority,
            topology,
        } => add(
            db,
            &dataset,
            &volume,
            node.as_deref(),
            &role,
            priority,
            topology.as_deref(),
            format,
        ),
        PlacementCommands::List { topology } => list(db, topology.as_deref(), format),
        PlacementCommands::Remove {
            dataset,
            volume,
            node,
            topology,
        } => remove(
            db,
            &dataset,
            &volume,
            node.as_deref(),
            topology.as_deref(),
            format,
        ),
    }
}

/// Validate that a placement role is one of the allowed values.
fn validate_role(value: &str) -> Result<()> {
    match value {
        "primary" | "replica" | "backup" | "archive" => Ok(()),
        _ => bail!(
            "Invalid role '{}': must be one of: primary, replica, backup, archive",
            value
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn add(
    db: &mut Database,
    dataset_ref: &str,
    volume_ref: &str,
    node_hint: Option<&str>,
    role: &str,
    priority: i32,
    topology_override: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    validate_role(role)?;

    let topo = resolve_active_topology(db, topology_override)?;
    let dataset = resolve_dataset(db, &topo.id, dataset_ref)?;
    let volume = resolve_volume(db, &topo.id, volume_ref, node_hint)?;

    // Check for duplicate placement
    let existing: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM placements WHERE dataset_id = ?1 AND volume_id = ?2",
        params![dataset.id, volume.id],
        |row| row.get(0),
    )?;
    if existing > 0 {
        bail!(
            "Dataset '{}' is already placed on volume '{}'",
            dataset.name,
            volume.name
        );
    }

    let mut placement = Placement::new(&topo.id, &dataset.id, &volume.id);
    placement.role = role.to_string();
    placement.priority = priority;

    let after_json = placement.to_json()?;
    let pl_id = placement.id.clone();
    let ds_name = dataset.name.clone();
    let vol_name = volume.name.clone();

    db.transaction(|tx| {
        placement.insert(tx)?;

        record_event(
            tx,
            "placement.created",
            "placement",
            &pl_id,
            &format!(
                "Placed dataset '{}' on volume '{}'",
                ds_name, vol_name
            ),
            None,
            Some(&after_json),
            &EventSource::User,
        )?;

        Ok(())
    })?;

    match format {
        OutputFormat::Text => {
            println!(
                "Placed dataset '{}' on volume '{}' [{}]",
                ds_name, vol_name, role
            );
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "action": "created",
                "placement_id": pl_id,
                "dataset": ds_name,
                "volume": vol_name,
                "role": role,
                "priority": priority,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

fn list(db: &mut Database, topology_override: Option<&str>, format: OutputFormat) -> Result<()> {
    let topo = resolve_active_topology(db, topology_override)?;

    let mut stmt = db.conn().prepare(
        "SELECT p.id, p.role, p.priority, p.created_at, \
         d.name AS dataset_name, d.size_bytes, d.criticality, \
         v.name AS volume_name, v.capacity_bytes, \
         n.name AS node_name \
         FROM placements p \
         JOIN datasets d ON p.dataset_id = d.id \
         JOIN volumes v ON p.volume_id = v.id \
         JOIN nodes n ON v.node_id = n.id \
         WHERE p.topology_id = ?1 \
         ORDER BY d.name, p.priority DESC",
    )?;

    struct PlacementRow {
        id: String,
        role: String,
        priority: i32,
        dataset_name: String,
        dataset_size: i64,
        criticality: String,
        volume_name: String,
        volume_capacity: i64,
        node_name: String,
    }

    let placements: Vec<PlacementRow> = stmt
        .query_map(params![topo.id], |row| {
            Ok(PlacementRow {
                id: row.get("id")?,
                role: row.get("role")?,
                priority: row.get("priority")?,
                dataset_name: row.get("dataset_name")?,
                dataset_size: row.get("size_bytes")?,
                criticality: row.get("criticality")?,
                volume_name: row.get("volume_name")?,
                volume_capacity: row.get("capacity_bytes")?,
                node_name: row.get("node_name")?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    match format {
        OutputFormat::Text => {
            if placements.is_empty() {
                println!(
                    "No placements found. Place a dataset with 'sp placement add <dataset> <volume>'"
                );
            } else {
                for p in &placements {
                    let ds_cap = Capacity::from_bytes(p.dataset_size as u64);
                    println!(
                        "  {} ({}) -> {}/{} [{}] (priority: {})",
                        p.dataset_name, ds_cap, p.node_name, p.volume_name, p.role, p.priority
                    );
                }
            }
        }
        OutputFormat::Json => {
            let json: Vec<serde_json::Value> = placements
                .iter()
                .map(|p| {
                    serde_json::json!({
                        "id": p.id,
                        "dataset": p.dataset_name,
                        "dataset_size_bytes": p.dataset_size,
                        "criticality": p.criticality,
                        "volume": p.volume_name,
                        "volume_capacity_bytes": p.volume_capacity,
                        "node": p.node_name,
                        "role": p.role,
                        "priority": p.priority,
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

fn remove(
    db: &mut Database,
    dataset_ref: &str,
    volume_ref: &str,
    node_hint: Option<&str>,
    topology_override: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    let topo = resolve_active_topology(db, topology_override)?;
    let dataset = resolve_dataset(db, &topo.id, dataset_ref)?;
    let volume = resolve_volume(db, &topo.id, volume_ref, node_hint)?;

    // Find the placement
    let placement_result = db.conn().query_row(
        "SELECT id, topology_id, dataset_id, volume_id, role, priority, created_at \
         FROM placements WHERE dataset_id = ?1 AND volume_id = ?2",
        params![dataset.id, volume.id],
        Placement::from_row,
    );

    let placement = match placement_result {
        Ok(p) => p,
        Err(_) => bail!(
            "No placement found for dataset '{}' on volume '{}'",
            dataset.name,
            volume.name
        ),
    };

    let before_json = placement.to_json()?;
    let pl_id = placement.id.clone();
    let ds_name = dataset.name.clone();
    let vol_name = volume.name.clone();

    db.transaction(|tx| {
        tx.execute("DELETE FROM placements WHERE id = ?1", [&pl_id])?;

        record_event(
            tx,
            "placement.deleted",
            "placement",
            &pl_id,
            &format!(
                "Removed dataset '{}' from volume '{}'",
                ds_name, vol_name
            ),
            Some(&before_json),
            None,
            &EventSource::User,
        )?;

        Ok(())
    })?;

    match format {
        OutputFormat::Text => {
            println!(
                "Removed dataset '{}' from volume '{}'",
                ds_name, vol_name
            );
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "action": "deleted",
                "placement_id": pl_id,
                "dataset": ds_name,
                "volume": vol_name,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

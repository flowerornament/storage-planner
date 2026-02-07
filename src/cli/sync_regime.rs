//! sp sync -- Manage data sync regimes between volumes
//!
//! Subcommands: add, list, show, remove
//! Sync regimes are immutable -- delete and recreate to change.
//! All mutating commands log events for undo/redo support.
//! All lookups support name-or-ID resolution via the entity resolver.

use anyhow::{bail, Result};
use clap::Subcommand;
use rusqlite::params;

use crate::core::db::Database;
use crate::core::events::{record_event, EventSource};
use crate::core::models::SyncRegime;
use crate::core::resolve::{
    resolve_active_topology, resolve_dataset, resolve_volume, validate_slug,
};

use super::OutputFormat;

#[derive(Subcommand)]
pub enum SyncCommands {
    /// Add a sync regime for a dataset between two volumes
    Add {
        /// Sync regime name (must be unique within topology)
        name: String,

        /// Dataset to sync
        #[arg(long)]
        dataset: String,

        /// Source volume
        #[arg(long)]
        from: String,

        /// Target volume
        #[arg(long)]
        to: String,

        /// Sync type (e.g., rsync, zfs-send, rclone, time-machine)
        #[arg(long, name = "type")]
        sync_type: String,

        /// Sync schedule (cron expression)
        #[arg(long)]
        schedule: Option<String>,

        /// Sync direction (push, pull, bidirectional)
        #[arg(long, default_value = "push")]
        direction: String,

        /// Source node (to disambiguate source volume)
        #[arg(long)]
        from_node: Option<String>,

        /// Target node (to disambiguate target volume)
        #[arg(long)]
        to_node: Option<String>,

        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },

    /// List sync regimes in the active topology
    List {
        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },

    /// Show details of a specific sync regime
    Show {
        /// Sync regime name or ID
        name: String,

        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },

    /// Remove a sync regime
    Remove {
        /// Sync regime name or ID to remove
        name: String,

        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },
}

pub fn run(cmd: SyncCommands, db: &mut Database, format: OutputFormat) -> Result<()> {
    match cmd {
        SyncCommands::Add {
            name,
            dataset,
            from,
            to,
            sync_type,
            schedule,
            direction,
            from_node,
            to_node,
            topology,
        } => add(
            db,
            &name,
            &dataset,
            &from,
            &to,
            &sync_type,
            schedule.as_deref(),
            &direction,
            from_node.as_deref(),
            to_node.as_deref(),
            topology.as_deref(),
            format,
        ),
        SyncCommands::List { topology } => list(db, topology.as_deref(), format),
        SyncCommands::Show { name, topology } => show(db, &name, topology.as_deref(), format),
        SyncCommands::Remove { name, topology } => remove(db, &name, topology.as_deref(), format),
    }
}

/// Find a sync regime by name (exact match) or UUID prefix within a topology.
fn find_sync_regime(db: &Database, topology_id: &str, name_or_id: &str) -> Result<SyncRegime> {
    // Try exact name match within topology
    let name_result = db.conn().query_row(
        "SELECT id, topology_id, name, dataset_id, source_volume_id, target_volume_id, \
         sync_type, schedule, direction, created_at, updated_at \
         FROM sync_regimes WHERE topology_id = ?1 AND name = ?2",
        params![topology_id, name_or_id],
        SyncRegime::from_row,
    );

    if let Ok(sr) = name_result {
        return Ok(sr);
    }

    // Try UUID prefix match (minimum 4 chars)
    if name_or_id.len() < 4 {
        bail!(
            "Sync regime '{}' not found. UUID prefix must be at least 4 characters.",
            name_or_id
        );
    }

    let pattern = format!("{}%", name_or_id);
    let mut stmt = db.conn().prepare(
        "SELECT id, topology_id, name, dataset_id, source_volume_id, target_volume_id, \
         sync_type, schedule, direction, created_at, updated_at \
         FROM sync_regimes WHERE topology_id = ?1 AND id LIKE ?2",
    )?;

    let matches: Vec<SyncRegime> = stmt
        .query_map(params![topology_id, pattern], SyncRegime::from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    match matches.len() {
        0 => bail!("Sync regime '{}' not found", name_or_id),
        1 => Ok(matches.into_iter().next().unwrap()),
        _ => {
            let names: Vec<String> = matches.iter().map(|s| s.name.clone()).collect();
            bail!(
                "Ambiguous sync regime prefix '{}': matches {}",
                name_or_id,
                names.join(", ")
            )
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn add(
    db: &mut Database,
    name: &str,
    dataset_name: &str,
    from: &str,
    to: &str,
    sync_type: &str,
    schedule: Option<&str>,
    direction: &str,
    from_node: Option<&str>,
    to_node: Option<&str>,
    topology_override: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    validate_slug(name)?;

    // Validate direction
    match direction {
        "push" | "pull" | "bidirectional" => {}
        _ => bail!(
            "Invalid direction '{}'. Must be one of: push, pull, bidirectional",
            direction
        ),
    }

    // Resolve active topology
    let topo = resolve_active_topology(db, topology_override)?;

    // Resolve dataset
    let dataset = resolve_dataset(db, &topo.id, dataset_name)?;

    // Resolve source and target volumes (with optional node hints)
    let source_volume = resolve_volume(db, &topo.id, from, from_node)?;
    let target_volume = resolve_volume(db, &topo.id, to, to_node)?;

    if source_volume.id == target_volume.id {
        bail!("Source and target volumes must be different");
    }

    // Check for duplicate name within topology
    let existing: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM sync_regimes WHERE topology_id = ?1 AND name = ?2",
        params![topo.id, name],
        |row| row.get(0),
    )?;
    if existing > 0 {
        bail!("Sync regime '{}' already exists in this topology", name);
    }

    let mut sr = SyncRegime::new(
        &topo.id,
        name,
        &dataset.id,
        &source_volume.id,
        &target_volume.id,
        sync_type,
    );
    sr.schedule = schedule.map(|s| s.to_string());
    sr.direction = direction.to_string();

    let after_json = sr.to_json()?;
    let sr_id = sr.id.clone();
    let sr_name = sr.name.clone();

    db.transaction(|tx| {
        sr.insert(tx)?;

        record_event(
            tx,
            "sync_regime.created",
            "sync_regime",
            &sr_id,
            &format!("Created sync regime '{}'", sr_name),
            None,
            Some(&after_json),
            &EventSource::User,
        )?;

        Ok(())
    })?;

    let id_prefix = &sr_id[..8];
    match format {
        OutputFormat::Text => {
            println!(
                "Created sync regime '{}' [{}] {} (id: {})",
                name, sync_type, direction, id_prefix
            );
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "action": "created",
                "sync_regime": name,
                "id": sr_id,
                "sync_type": sync_type,
                "direction": direction,
                "dataset": dataset.name,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

fn list(db: &mut Database, topology_override: Option<&str>, format: OutputFormat) -> Result<()> {
    let topo = resolve_active_topology(db, topology_override)?;

    let mut stmt = db.conn().prepare(
        "SELECT sr.id, sr.topology_id, sr.name, sr.dataset_id, sr.source_volume_id, \
         sr.target_volume_id, sr.sync_type, sr.schedule, sr.direction, sr.created_at, sr.updated_at, \
         d.name AS dataset_name, \
         sv.name AS source_volume_name, sn.name AS source_node_name, \
         tv.name AS target_volume_name, tn.name AS target_node_name \
         FROM sync_regimes sr \
         JOIN datasets d ON sr.dataset_id = d.id \
         JOIN volumes sv ON sr.source_volume_id = sv.id \
         JOIN nodes sn ON sv.node_id = sn.id \
         JOIN volumes tv ON sr.target_volume_id = tv.id \
         JOIN nodes tn ON tv.node_id = tn.id \
         WHERE sr.topology_id = ?1 \
         ORDER BY sr.name",
    )?;

    struct SyncRow {
        sr: SyncRegime,
        dataset_name: String,
        source_volume_name: String,
        source_node_name: String,
        target_volume_name: String,
        target_node_name: String,
    }

    let regimes: Vec<SyncRow> = stmt
        .query_map(params![topo.id], |row| {
            Ok(SyncRow {
                sr: SyncRegime::from_row(row)?,
                dataset_name: row.get("dataset_name")?,
                source_volume_name: row.get("source_volume_name")?,
                source_node_name: row.get("source_node_name")?,
                target_volume_name: row.get("target_volume_name")?,
                target_node_name: row.get("target_node_name")?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    match format {
        OutputFormat::Text => {
            if regimes.is_empty() {
                println!(
                    "No sync regimes found. Create one with 'sp sync add <name> --dataset=<ds> --from=<vol> --to=<vol> --type=<type>'"
                );
            } else {
                for r in &regimes {
                    let schedule_str =
                        r.sr.schedule
                            .as_deref()
                            .map(|s| format!(" ({})", s))
                            .unwrap_or_default();
                    println!(
                        "  {} [{}] {}: {}:{} -> {}:{}{}",
                        r.sr.name,
                        r.sr.sync_type,
                        r.dataset_name,
                        r.source_node_name,
                        r.source_volume_name,
                        r.target_node_name,
                        r.target_volume_name,
                        schedule_str,
                    );
                }
            }
        }
        OutputFormat::Json => {
            let json: Vec<serde_json::Value> = regimes
                .iter()
                .map(|r| {
                    serde_json::json!({
                        "id": r.sr.id,
                        "name": r.sr.name,
                        "sync_type": r.sr.sync_type,
                        "direction": r.sr.direction,
                        "schedule": r.sr.schedule,
                        "dataset": r.dataset_name,
                        "source_volume": r.source_volume_name,
                        "source_node": r.source_node_name,
                        "target_volume": r.target_volume_name,
                        "target_node": r.target_node_name,
                        "created_at": r.sr.created_at.to_rfc3339(),
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
    let sr = find_sync_regime(db, &topo.id, name)?;

    // Resolve all related entity names
    let dataset_name: String = db.conn().query_row(
        "SELECT name FROM datasets WHERE id = ?1",
        params![sr.dataset_id],
        |row| row.get(0),
    )?;

    let (source_vol_name, source_node_name): (String, String) = db.conn().query_row(
        "SELECT v.name, n.name FROM volumes v JOIN nodes n ON v.node_id = n.id WHERE v.id = ?1",
        params![sr.source_volume_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    let (target_vol_name, target_node_name): (String, String) = db.conn().query_row(
        "SELECT v.name, n.name FROM volumes v JOIN nodes n ON v.node_id = n.id WHERE v.id = ?1",
        params![sr.target_volume_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    match format {
        OutputFormat::Text => {
            println!("Sync Regime: {} [{}]", sr.name, sr.sync_type);
            println!("  Dataset:         {}", dataset_name);
            println!(
                "  Source:          {}:{} ({})",
                source_node_name, source_vol_name, sr.source_volume_id
            );
            println!(
                "  Target:          {}:{} ({})",
                target_node_name, target_vol_name, sr.target_volume_id
            );
            println!("  Direction:       {}", sr.direction);
            if let Some(ref sched) = sr.schedule {
                println!("  Schedule:        {}", sched);
            }
            println!("  ID:              {}", sr.id);
            println!(
                "  Created:         {}",
                sr.created_at.format("%Y-%m-%d %H:%M:%S")
            );
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "id": sr.id,
                "name": sr.name,
                "sync_type": sr.sync_type,
                "direction": sr.direction,
                "schedule": sr.schedule,
                "dataset": dataset_name,
                "source_volume": source_vol_name,
                "source_node": source_node_name,
                "target_volume": target_vol_name,
                "target_node": target_node_name,
                "created_at": sr.created_at.to_rfc3339(),
                "updated_at": sr.updated_at.to_rfc3339(),
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
    let sr = find_sync_regime(db, &topo.id, name)?;

    let before_json = sr.to_json()?;
    let sr_id = sr.id.clone();
    let sr_name = sr.name.clone();

    db.transaction(|tx| {
        tx.execute("DELETE FROM sync_regimes WHERE id = ?1", params![sr_id])?;

        record_event(
            tx,
            "sync_regime.deleted",
            "sync_regime",
            &sr_id,
            &format!("Deleted sync regime '{}'", sr_name),
            Some(&before_json),
            None,
            &EventSource::User,
        )?;

        Ok(())
    })?;

    match format {
        OutputFormat::Text => {
            println!("Removed sync regime '{}'", sr_name);
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "action": "deleted",
                "sync_regime": sr_name,
                "id": sr_id,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

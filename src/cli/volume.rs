//! sp volume -- Manage storage volumes attached to nodes
//!
//! Subcommands: add, list, show, remove, update
//! All mutating commands log events for undo/redo support.
//! All lookups support name-or-ID resolution via the entity resolver.
//! Volume names can be disambiguated via --node when the same name exists on multiple nodes.

use anyhow::{bail, Result};
use clap::Subcommand;
use rusqlite::params;

use crate::core::db::Database;
use crate::core::events::{record_event, EventSource};
use crate::core::models::Volume;
use crate::core::resolve::{resolve_active_topology, resolve_node, resolve_volume, validate_slug};
use crate::core::specs::Capacity;

use super::OutputFormat;

#[derive(Subcommand)]
pub enum VolumeCommands {
    /// Add a storage volume to a node
    Add {
        /// Volume name (must be unique within node)
        name: String,

        /// Node to attach this volume to
        #[arg(long)]
        node: String,

        /// Total capacity (e.g., "4TB", "500GB")
        #[arg(long)]
        capacity: String,

        /// Usable capacity after overhead (e.g., "3.6TB")
        #[arg(long)]
        usable: Option<String>,

        /// Filesystem type (e.g., apfs, ext4, zfs, btrfs)
        #[arg(long)]
        filesystem: Option<String>,

        /// RAID level if applicable (e.g., raid1, raid5, raidz2)
        #[arg(long)]
        raid: Option<String>,

        /// Pool type (e.g., stripe, mirror, raidz)
        #[arg(long)]
        pool_type: Option<String>,

        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },

    /// List volumes in the active topology
    List {
        /// Filter by node name or ID
        #[arg(long)]
        node: Option<String>,

        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },

    /// Show details of a specific volume
    Show {
        /// Volume name or ID
        name: String,

        /// Node hint for disambiguation
        #[arg(long)]
        node: Option<String>,

        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },

    /// Remove a volume from a node
    Remove {
        /// Volume name or ID to remove
        name: String,

        /// Node hint for disambiguation
        #[arg(long)]
        node: Option<String>,

        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },

    /// Update a volume's properties
    Update {
        /// Volume name or ID to update
        name: String,

        /// Node hint for disambiguation
        #[arg(long)]
        node: Option<String>,

        /// Rename the volume (must be a valid slug)
        #[arg(long)]
        rename: Option<String>,

        /// Change total capacity (e.g., "8TB")
        #[arg(long)]
        capacity: Option<String>,

        /// Change usable capacity (e.g., "7.2TB")
        #[arg(long)]
        usable: Option<String>,

        /// Change filesystem type
        #[arg(long)]
        filesystem: Option<String>,

        /// Change RAID level
        #[arg(long)]
        raid: Option<String>,

        /// Change pool type
        #[arg(long)]
        pool_type: Option<String>,

        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },
}

pub fn run(cmd: VolumeCommands, db: &mut Database, format: OutputFormat) -> Result<()> {
    match cmd {
        VolumeCommands::Add {
            name,
            node,
            capacity,
            usable,
            filesystem,
            raid,
            pool_type,
            topology,
        } => add(
            db,
            &name,
            &node,
            &capacity,
            usable.as_deref(),
            filesystem.as_deref(),
            raid.as_deref(),
            pool_type.as_deref(),
            topology.as_deref(),
            format,
        ),
        VolumeCommands::List { node, topology } => {
            list(db, node.as_deref(), topology.as_deref(), format)
        }
        VolumeCommands::Show {
            name,
            node,
            topology,
        } => show(db, &name, node.as_deref(), topology.as_deref(), format),
        VolumeCommands::Remove {
            name,
            node,
            topology,
        } => remove(db, &name, node.as_deref(), topology.as_deref(), format),
        VolumeCommands::Update {
            name,
            node,
            rename,
            capacity,
            usable,
            filesystem,
            raid,
            pool_type,
            topology,
        } => update(
            db,
            &name,
            node.as_deref(),
            rename.as_deref(),
            capacity.as_deref(),
            usable.as_deref(),
            filesystem.as_deref(),
            raid.as_deref(),
            pool_type.as_deref(),
            topology.as_deref(),
            format,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn add(
    db: &mut Database,
    name: &str,
    node_name: &str,
    capacity_str: &str,
    usable_str: Option<&str>,
    filesystem: Option<&str>,
    raid: Option<&str>,
    pool_type: Option<&str>,
    topology_override: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    validate_slug(name)?;

    // Parse capacity
    let capacity = Capacity::parse(capacity_str)?;
    let usable_bytes = usable_str.map(Capacity::parse).transpose()?;

    // Resolve active topology and node
    let topo = resolve_active_topology(db, topology_override)?;
    let node = resolve_node(db, &topo.id, node_name)?;

    let mut vol = Volume::new(&topo.id, &node.id, name, capacity.bytes as i64);
    vol.usable_bytes = usable_bytes.map(|u| u.bytes as i64);
    vol.filesystem = filesystem.map(|s| s.to_string());
    vol.raid_level = raid.map(|s| s.to_string());
    vol.pool_type = pool_type.map(|s| s.to_string());

    let after_json = vol.to_json()?;
    let vol_id = vol.id.clone();
    let vol_name = vol.name.clone();

    db.transaction(|tx| {
        vol.insert(tx)?;

        record_event(
            tx,
            "volume.created",
            "volume",
            &vol_id,
            &format!("Created volume '{}' on node '{}'", vol_name, node.name),
            None,
            Some(&after_json),
            &EventSource::User,
        )?;

        Ok(())
    })?;

    let id_prefix = &vol_id[..8];
    match format {
        OutputFormat::Text => {
            println!(
                "Created volume '{}' ({}) on node '{}' (id: {})",
                name, capacity, node.name, id_prefix
            );
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "action": "created",
                "volume": name,
                "node": node.name,
                "capacity_bytes": capacity.bytes,
                "id": vol_id,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

fn list(
    db: &mut Database,
    node_filter: Option<&str>,
    topology_override: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    let topo = resolve_active_topology(db, topology_override)?;

    let volumes: Vec<Volume> = if let Some(node_name) = node_filter {
        let node = resolve_node(db, &topo.id, node_name)?;
        let mut stmt = db.conn().prepare(
            "SELECT id, topology_id, node_id, name, capacity_bytes, usable_bytes, \
             filesystem, raid_level, pool_type, item_id, created_at, updated_at \
             FROM volumes WHERE topology_id = ?1 AND node_id = ?2 ORDER BY name",
        )?;
        let result = stmt
            .query_map(params![topo.id, node.id], Volume::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        result
    } else {
        let mut stmt = db.conn().prepare(
            "SELECT id, topology_id, node_id, name, capacity_bytes, usable_bytes, \
             filesystem, raid_level, pool_type, item_id, created_at, updated_at \
             FROM volumes WHERE topology_id = ?1 ORDER BY name",
        )?;
        let result = stmt
            .query_map(params![topo.id], Volume::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        result
    };

    match format {
        OutputFormat::Text => {
            if volumes.is_empty() {
                println!(
                    "No volumes found. Create one with 'sp volume add <name> --node=<node> --capacity=<size>'"
                );
            } else {
                for vol in &volumes {
                    let cap = Capacity::from_bytes(vol.capacity_bytes as u64);
                    let node_name = node_name_for_id(db, &vol.node_id);
                    let fs = vol
                        .filesystem
                        .as_deref()
                        .map(|f| format!(" {}", f))
                        .unwrap_or_default();
                    let raid = vol
                        .raid_level
                        .as_ref()
                        .map(|r| format!("/{}", r))
                        .unwrap_or_default();
                    println!(
                        "  {} on {} [{}{}{}]",
                        vol.name, node_name, cap, fs, raid
                    );
                }
            }
        }
        OutputFormat::Json => {
            let json: Vec<serde_json::Value> = volumes
                .iter()
                .map(|v| {
                    let cap = Capacity::from_bytes(v.capacity_bytes as u64);
                    let node_name = node_name_for_id(db, &v.node_id);
                    serde_json::json!({
                        "id": v.id,
                        "name": v.name,
                        "node": node_name,
                        "capacity_bytes": v.capacity_bytes,
                        "capacity_formatted": cap.to_string(),
                        "usable_bytes": v.usable_bytes,
                        "filesystem": v.filesystem,
                        "raid_level": v.raid_level,
                        "pool_type": v.pool_type,
                        "created_at": v.created_at.to_rfc3339(),
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
    node_hint: Option<&str>,
    topology_override: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    let topo = resolve_active_topology(db, topology_override)?;
    let vol = resolve_volume(db, &topo.id, name, node_hint)?;
    let node_name = node_name_for_id(db, &vol.node_id);

    let cap = Capacity::from_bytes(vol.capacity_bytes as u64);
    let usable = vol
        .usable_bytes
        .map(|b| Capacity::from_bytes(b as u64).to_string());

    match format {
        OutputFormat::Text => {
            println!("Volume: {}", vol.name);
            println!("  Node:            {}", node_name);
            println!("  Capacity:        {}", cap);
            if let Some(ref usable_str) = usable {
                println!("  Usable:          {}", usable_str);
            }
            if let Some(ref fs) = vol.filesystem {
                println!("  Filesystem:      {}", fs);
            }
            if let Some(ref raid) = vol.raid_level {
                println!("  RAID:            {}", raid);
            }
            if let Some(ref pool) = vol.pool_type {
                println!("  Pool type:       {}", pool);
            }
            if let Some(ref item) = vol.item_id {
                println!("  Item:            {}", item);
            }
            println!("  ID:              {}", vol.id);
            println!(
                "  Created:         {}",
                vol.created_at.format("%Y-%m-%d %H:%M:%S")
            );
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&vol)?);
        }
    }

    Ok(())
}

fn remove(
    db: &mut Database,
    name: &str,
    node_hint: Option<&str>,
    topology_override: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    let topo = resolve_active_topology(db, topology_override)?;
    let vol = resolve_volume(db, &topo.id, name, node_hint)?;

    // Count dependent placements
    let placement_count: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM placements WHERE volume_id = ?1",
        params![vol.id],
        |row| row.get(0),
    )?;

    let before_json = vol.to_json()?;
    let vol_id = vol.id.clone();
    let vol_name = vol.name.clone();
    let node_name = node_name_for_id(db, &vol.node_id);

    if placement_count > 0 {
        eprintln!(
            "Warning: Removing volume '{}' (and {} placement{})",
            vol_name,
            placement_count,
            if placement_count == 1 { "" } else { "s" }
        );
    }

    db.transaction(|tx| {
        tx.execute("DELETE FROM volumes WHERE id = ?1", params![vol_id])?;

        record_event(
            tx,
            "volume.deleted",
            "volume",
            &vol_id,
            &format!(
                "Deleted volume '{}' from node '{}'",
                vol_name, node_name
            ),
            Some(&before_json),
            None,
            &EventSource::User,
        )?;

        Ok(())
    })?;

    match format {
        OutputFormat::Text => {
            println!("Removed volume '{}' from node '{}'", vol_name, node_name);
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "action": "deleted",
                "volume": vol_name,
                "node": node_name,
                "id": vol_id,
                "placements_removed": placement_count,
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
    node_hint: Option<&str>,
    rename: Option<&str>,
    capacity_str: Option<&str>,
    usable_str: Option<&str>,
    filesystem: Option<&str>,
    raid: Option<&str>,
    pool_type: Option<&str>,
    topology_override: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    if rename.is_none()
        && capacity_str.is_none()
        && usable_str.is_none()
        && filesystem.is_none()
        && raid.is_none()
        && pool_type.is_none()
    {
        bail!("Nothing to update. Provide --rename, --capacity, --usable, --filesystem, --raid, or --pool-type.");
    }

    // Validate new name if renaming
    if let Some(new_name) = rename {
        validate_slug(new_name)?;
    }

    // Parse capacity values if provided
    let new_capacity = capacity_str.map(Capacity::parse).transpose()?;
    let new_usable = usable_str.map(Capacity::parse).transpose()?;

    // Resolve outside transaction
    let topo = resolve_active_topology(db, topology_override)?;
    let vol = resolve_volume(db, &topo.id, name, node_hint)?;
    let before_json = vol.to_json()?;
    let vol_id = vol.id.clone();
    let original_name = vol.name.clone();

    // Check uniqueness of new name if renaming
    if let Some(new_name) = rename {
        if new_name != original_name {
            let existing: i64 = db.conn().query_row(
                "SELECT COUNT(*) FROM volumes WHERE topology_id = ?1 AND node_id = ?2 AND name = ?3 AND id != ?4",
                params![topo.id, vol.node_id, new_name, vol_id],
                |row| row.get(0),
            )?;
            if existing > 0 {
                bail!(
                    "Volume name '{}' is already taken on this node",
                    new_name
                );
            }
        }
    }

    // Build after state for event
    let mut after = vol.clone();
    if let Some(new_name) = rename {
        after.name = new_name.to_string();
    }
    if let Some(cap) = &new_capacity {
        after.capacity_bytes = cap.bytes as i64;
    }
    if let Some(usable) = &new_usable {
        after.usable_bytes = Some(usable.bytes as i64);
    }
    if let Some(fs) = filesystem {
        after.filesystem = Some(fs.to_string());
    }
    if let Some(r) = raid {
        after.raid_level = Some(r.to_string());
    }
    if let Some(pt) = pool_type {
        after.pool_type = Some(pt.to_string());
    }
    let after_json = after.to_json()?;
    let final_name = after.name.clone();

    db.transaction(|tx| {
        if let Some(new_name) = rename {
            tx.execute(
                "UPDATE volumes SET name = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![new_name, vol_id],
            )?;
        }
        if let Some(cap) = &new_capacity {
            tx.execute(
                "UPDATE volumes SET capacity_bytes = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![cap.bytes as i64, vol_id],
            )?;
        }
        if let Some(usable) = &new_usable {
            tx.execute(
                "UPDATE volumes SET usable_bytes = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![usable.bytes as i64, vol_id],
            )?;
        }
        if let Some(fs) = filesystem {
            tx.execute(
                "UPDATE volumes SET filesystem = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![fs, vol_id],
            )?;
        }
        if let Some(r) = raid {
            tx.execute(
                "UPDATE volumes SET raid_level = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![r, vol_id],
            )?;
        }
        if let Some(pt) = pool_type {
            tx.execute(
                "UPDATE volumes SET pool_type = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![pt, vol_id],
            )?;
        }

        record_event(
            tx,
            "volume.updated",
            "volume",
            &vol_id,
            &format!("Updated volume '{}'", original_name),
            Some(&before_json),
            Some(&after_json),
            &EventSource::User,
        )?;

        Ok(())
    })?;

    match format {
        OutputFormat::Text => {
            println!("Updated volume '{}'", final_name);
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "action": "updated",
                "volume": final_name,
                "id": vol_id,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

/// Look up a node's name by its ID (for display purposes).
fn node_name_for_id(db: &Database, node_id: &str) -> String {
    db.conn()
        .query_row(
            "SELECT name FROM nodes WHERE id = ?1",
            params![node_id],
            |row| row.get::<_, String>(0),
        )
        .unwrap_or_else(|_| "unknown".to_string())
}

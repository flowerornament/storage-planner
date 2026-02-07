//! sp link -- Manage network links between nodes
//!
//! Subcommands: add, list, show, remove
//! Links are immutable -- delete and recreate to change.
//! All mutating commands log events for undo/redo support.
//! All lookups support name-or-ID resolution via the entity resolver.

use anyhow::{bail, Result};
use clap::Subcommand;
use rusqlite::params;

use crate::core::db::Database;
use crate::core::events::{record_event, EventSource};
use crate::core::models::Link;
use crate::core::resolve::{resolve_active_topology, resolve_node};
use crate::core::specs::Speed;

use super::OutputFormat;

#[derive(Subcommand)]
pub enum LinkCommands {
    /// Add a network link between two nodes
    Add {
        /// Source node name
        #[arg(long)]
        from: String,

        /// Target node name
        #[arg(long)]
        to: String,

        /// Connection type (e.g., lan, wan, usb, thunderbolt)
        #[arg(long, name = "type")]
        connection_type: String,

        /// Bandwidth (e.g., "1GB/s", "100MB/s")
        #[arg(long)]
        bandwidth: Option<String>,

        /// Latency in milliseconds
        #[arg(long)]
        latency: Option<f64>,

        /// Whether the connection is metered
        #[arg(long)]
        metered: bool,

        /// Cost per GB in cents (for metered connections)
        #[arg(long)]
        cost_per_gb: Option<i32>,

        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },

    /// List links in the active topology
    List {
        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },

    /// Show details of a specific link
    Show {
        /// Link identifier (source--target)
        name: String,

        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },

    /// Remove a link between nodes
    Remove {
        /// Link identifier (source--target)
        name: String,

        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },
}

pub fn run(cmd: LinkCommands, db: &mut Database, format: OutputFormat) -> Result<()> {
    match cmd {
        LinkCommands::Add {
            from,
            to,
            connection_type,
            bandwidth,
            latency,
            metered,
            cost_per_gb,
            topology,
        } => add(
            db,
            &from,
            &to,
            &connection_type,
            bandwidth.as_deref(),
            latency,
            metered,
            cost_per_gb,
            topology.as_deref(),
            format,
        ),
        LinkCommands::List { topology } => list(db, topology.as_deref(), format),
        LinkCommands::Show { name, topology } => show(db, &name, topology.as_deref(), format),
        LinkCommands::Remove { name, topology } => remove(db, &name, topology.as_deref(), format),
    }
}

#[allow(clippy::too_many_arguments)]
fn add(
    db: &mut Database,
    from: &str,
    to: &str,
    connection_type: &str,
    bandwidth: Option<&str>,
    latency: Option<f64>,
    metered: bool,
    cost_per_gb: Option<i32>,
    topology_override: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    // Resolve active topology
    let topo = resolve_active_topology(db, topology_override)?;

    // Resolve source and target nodes
    let source_node = resolve_node(db, &topo.id, from)?;
    let target_node = resolve_node(db, &topo.id, to)?;

    if source_node.id == target_node.id {
        bail!("Cannot create a link from a node to itself");
    }

    // Check for existing link between same node pair
    let existing: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM links WHERE topology_id = ?1 AND source_node_id = ?2 AND target_node_id = ?3",
        params![topo.id, source_node.id, target_node.id],
        |row| row.get(0),
    )?;
    if existing > 0 {
        bail!(
            "Link already exists between '{}' and '{}'",
            source_node.name,
            target_node.name
        );
    }

    // Parse bandwidth if provided
    let bandwidth_bytes_sec = if let Some(bw) = bandwidth {
        Some(Speed::parse(bw)?.bytes_per_sec as i64)
    } else {
        None
    };

    let mut link = Link::new(&topo.id, &source_node.id, &target_node.id, connection_type);
    link.bandwidth_bytes_sec = bandwidth_bytes_sec;
    link.latency_ms = latency;
    link.is_metered = metered;
    link.cost_per_gb_cents = cost_per_gb;

    let after_json = link.to_json()?;
    let link_id = link.id.clone();
    let display_name = format!("{}--{}", source_node.name, target_node.name);

    db.transaction(|tx| {
        link.insert(tx)?;

        record_event(
            tx,
            "link.created",
            "link",
            &link_id,
            &format!("Created link '{}'", display_name),
            None,
            Some(&after_json),
            &EventSource::User,
        )?;

        Ok(())
    })?;

    let id_prefix = &link_id[..8];
    match format {
        OutputFormat::Text => {
            let bw_str = bandwidth_bytes_sec
                .map(|b| {
                    format!(
                        " {}",
                        Speed {
                            bytes_per_sec: b as u64
                        }
                    )
                })
                .unwrap_or_default();
            println!(
                "Created link '{}' [{}]{} (id: {})",
                display_name, connection_type, bw_str, id_prefix
            );
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "action": "created",
                "link": display_name,
                "id": link_id,
                "connection_type": connection_type,
                "bandwidth_bytes_sec": bandwidth_bytes_sec,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

fn list(db: &mut Database, topology_override: Option<&str>, format: OutputFormat) -> Result<()> {
    let topo = resolve_active_topology(db, topology_override)?;

    let mut stmt = db.conn().prepare(
        "SELECT l.id, l.topology_id, l.source_node_id, l.target_node_id, \
         l.bandwidth_bytes_sec, l.connection_type, l.latency_ms, l.is_metered, \
         l.cost_per_gb_cents, l.created_at, l.updated_at, \
         sn.name AS source_name, tn.name AS target_name \
         FROM links l \
         JOIN nodes sn ON l.source_node_id = sn.id \
         JOIN nodes tn ON l.target_node_id = tn.id \
         WHERE l.topology_id = ?1 \
         ORDER BY sn.name, tn.name",
    )?;

    struct LinkRow {
        link: Link,
        source_name: String,
        target_name: String,
    }

    let links: Vec<LinkRow> = stmt
        .query_map(params![topo.id], |row| {
            Ok(LinkRow {
                link: Link::from_row(row)?,
                source_name: row.get("source_name")?,
                target_name: row.get("target_name")?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    match format {
        OutputFormat::Text => {
            if links.is_empty() {
                println!(
                    "No links found. Create one with 'sp link add --from=<node> --to=<node> --type=<type>'"
                );
            } else {
                for lr in &links {
                    let bw_str = lr
                        .link
                        .bandwidth_bytes_sec
                        .map(|b| {
                            format!(
                                " {}",
                                Speed {
                                    bytes_per_sec: b as u64
                                }
                            )
                        })
                        .unwrap_or_default();
                    println!(
                        "  {} -> {} [{}]{}",
                        lr.source_name, lr.target_name, lr.link.connection_type, bw_str
                    );
                }
            }
        }
        OutputFormat::Json => {
            let json: Vec<serde_json::Value> = links
                .iter()
                .map(|lr| {
                    serde_json::json!({
                        "id": lr.link.id,
                        "source_node": lr.source_name,
                        "target_node": lr.target_name,
                        "connection_type": lr.link.connection_type,
                        "bandwidth_bytes_sec": lr.link.bandwidth_bytes_sec,
                        "latency_ms": lr.link.latency_ms,
                        "is_metered": lr.link.is_metered,
                        "cost_per_gb_cents": lr.link.cost_per_gb_cents,
                        "created_at": lr.link.created_at.to_rfc3339(),
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

/// Parse a link name in the format "source--target" into the two node names.
fn parse_link_name(name: &str) -> Result<(&str, &str)> {
    let parts: Vec<&str> = name.splitn(2, "--").collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        bail!(
            "Invalid link name '{}'. Expected format: source--target (e.g., mac-mini--nas)",
            name
        );
    }
    Ok((parts[0], parts[1]))
}

/// Find a link by source and target node IDs within a topology.
fn find_link(
    db: &Database,
    topology_id: &str,
    source_node_id: &str,
    target_node_id: &str,
) -> Result<Link> {
    db.conn()
        .query_row(
            "SELECT id, topology_id, source_node_id, target_node_id, bandwidth_bytes_sec, \
             connection_type, latency_ms, is_metered, cost_per_gb_cents, created_at, updated_at \
             FROM links WHERE topology_id = ?1 AND source_node_id = ?2 AND target_node_id = ?3",
            params![topology_id, source_node_id, target_node_id],
            Link::from_row,
        )
        .map_err(|_| anyhow::anyhow!("Link not found between the specified nodes in this topology"))
}

fn show(
    db: &mut Database,
    name: &str,
    topology_override: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    let topo = resolve_active_topology(db, topology_override)?;
    let (source_name, target_name) = parse_link_name(name)?;

    // Resolve both nodes
    let source_node = resolve_node(db, &topo.id, source_name)?;
    let target_node = resolve_node(db, &topo.id, target_name)?;

    // Find the link
    let link = find_link(db, &topo.id, &source_node.id, &target_node.id)?;

    match format {
        OutputFormat::Text => {
            println!(
                "Link: {} -> {} [{}]",
                source_node.name, target_node.name, link.connection_type
            );
            if let Some(bw) = link.bandwidth_bytes_sec {
                println!(
                    "  Bandwidth:       {}",
                    Speed {
                        bytes_per_sec: bw as u64
                    }
                );
            }
            if let Some(lat) = link.latency_ms {
                println!("  Latency:         {:.1}ms", lat);
            }
            println!(
                "  Metered:         {}",
                if link.is_metered { "yes" } else { "no" }
            );
            if let Some(cost) = link.cost_per_gb_cents {
                println!("  Cost per GB:     {} cents", cost);
            }
            println!("  ID:              {}", link.id);
            println!(
                "  Created:         {}",
                link.created_at.format("%Y-%m-%d %H:%M:%S")
            );
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "id": link.id,
                "source_node": source_node.name,
                "target_node": target_node.name,
                "connection_type": link.connection_type,
                "bandwidth_bytes_sec": link.bandwidth_bytes_sec,
                "latency_ms": link.latency_ms,
                "is_metered": link.is_metered,
                "cost_per_gb_cents": link.cost_per_gb_cents,
                "created_at": link.created_at.to_rfc3339(),
                "updated_at": link.updated_at.to_rfc3339(),
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
    let (source_name, target_name) = parse_link_name(name)?;

    // Resolve both nodes
    let source_node = resolve_node(db, &topo.id, source_name)?;
    let target_node = resolve_node(db, &topo.id, target_name)?;

    // Find the link
    let link = find_link(db, &topo.id, &source_node.id, &target_node.id)?;

    // Count dependent sync_regimes (syncs between volumes on these nodes)
    let sync_count: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM sync_regimes sr \
         JOIN volumes sv ON sr.source_volume_id = sv.id \
         JOIN volumes tv ON sr.target_volume_id = tv.id \
         WHERE sr.topology_id = ?1 \
         AND ((sv.node_id = ?2 AND tv.node_id = ?3) OR (sv.node_id = ?3 AND tv.node_id = ?2))",
        params![topo.id, source_node.id, target_node.id],
        |row| row.get(0),
    )?;

    if sync_count > 0 {
        eprintln!(
            "Warning: {} sync regime{} use volumes on these nodes and may be affected",
            sync_count,
            if sync_count == 1 { "" } else { "s" }
        );
    }

    let before_json = link.to_json()?;
    let link_id = link.id.clone();
    let display_name = format!("{}--{}", source_node.name, target_node.name);

    db.transaction(|tx| {
        tx.execute("DELETE FROM links WHERE id = ?1", params![link_id])?;

        record_event(
            tx,
            "link.deleted",
            "link",
            &link_id,
            &format!("Deleted link '{}'", display_name),
            Some(&before_json),
            None,
            &EventSource::User,
        )?;

        Ok(())
    })?;

    match format {
        OutputFormat::Text => {
            println!("Removed link '{}'", display_name);
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "action": "deleted",
                "link": display_name,
                "id": link_id,
                "sync_regimes_affected": sync_count,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

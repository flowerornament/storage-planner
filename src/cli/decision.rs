//! sp decision -- Track purchase decisions with lifecycle management
//!
//! Subcommands: create, show, list, update, constrain, unconstrain, consider, unconsider,
//!              choose, abandon, reopen
//! All mutating commands log events for undo/redo support.
//! All lookups support title-or-ID resolution via the entity resolver.

use anyhow::{bail, Result};
use chrono::Utc;
use clap::Subcommand;
use rusqlite::params;

use crate::core::db::Database;
use crate::core::events::{record_event, EventSource};
use crate::core::models::{Dataset, Decision, DecisionConstraint, DecisionTopology, Node, Volume};
use crate::core::resolve::{resolve_decision, resolve_topology};
use crate::domains::storage::analysis::{
    check_constraints, compute_topology_metrics, load_placements_with_context,
    load_sync_regimes_with_context,
};

use super::OutputFormat;

#[derive(Subcommand)]
pub enum DecisionCommands {
    /// Create a new decision
    Create {
        /// Decision title (free-text, spaces allowed)
        title: String,

        /// Optional description
        #[arg(long, default_value = "")]
        description: String,

        /// Optional parent decision (title or ID prefix)
        #[arg(long)]
        parent: Option<String>,
    },

    /// Show details of a decision
    Show {
        /// Decision title or ID prefix
        decision: String,
    },

    /// List decisions with optional status filter
    List {
        /// Filter by status (draft, open, decided, abandoned)
        #[arg(long)]
        status: Option<String>,
    },

    /// Update a decision's title, description, or status
    Update {
        /// Decision title or ID prefix
        decision: String,

        /// New title
        #[arg(long)]
        title: Option<String>,

        /// New description
        #[arg(long)]
        description: Option<String>,

        /// Transition from draft to open
        #[arg(long)]
        open: bool,
    },

    /// Add a typed constraint to a decision
    Constrain {
        /// Decision title or ID prefix
        decision: String,

        /// Constraint type: budget, noise, power, rack_units
        #[arg(long, value_name = "TYPE")]
        r#type: String,

        /// Maximum allowed value
        #[arg(long)]
        max: f64,
    },

    /// Remove a constraint from a decision
    Unconstrain {
        /// Decision title or ID prefix
        decision: String,

        /// Constraint type to remove: budget, noise, power, rack_units
        #[arg(long, value_name = "TYPE")]
        r#type: String,
    },

    /// Add a topology to a decision's comparison set
    Consider {
        /// Decision title or ID prefix
        decision: String,

        /// Topology name or ID prefix
        topology: String,
    },

    /// Remove a topology from a decision's comparison set
    Unconsider {
        /// Decision title or ID prefix
        decision: String,

        /// Topology name or ID prefix
        topology: String,
    },

    /// Choose a topology for a decision (closes the decision)
    Choose {
        /// Decision title or ID prefix
        decision: String,

        /// Topology name or ID prefix to choose
        topology: String,

        /// Rationale for the choice
        #[arg(long)]
        rationale: String,
    },

    /// Abandon a decision
    Abandon {
        /// Decision title or ID prefix
        decision: String,

        /// Optional reason for abandoning
        #[arg(long)]
        reason: Option<String>,
    },

    /// Reopen a decided or abandoned decision
    Reopen {
        /// Decision title or ID prefix
        decision: String,
    },
}

pub fn run(cmd: DecisionCommands, db: &mut Database, format: OutputFormat) -> Result<()> {
    match cmd {
        DecisionCommands::Create {
            title,
            description,
            parent,
        } => create(db, &title, &description, parent.as_deref(), format),
        DecisionCommands::Show { decision } => show(db, &decision, format),
        DecisionCommands::List { status } => list(db, status.as_deref(), format),
        DecisionCommands::Update {
            decision,
            title,
            description,
            open,
        } => update(
            db,
            &decision,
            title.as_deref(),
            description.as_deref(),
            open,
            format,
        ),
        DecisionCommands::Constrain {
            decision,
            r#type,
            max,
        } => constrain(db, &decision, &r#type, max, format),
        DecisionCommands::Unconstrain { decision, r#type } => {
            unconstrain(db, &decision, &r#type, format)
        }
        DecisionCommands::Consider { decision, topology } => {
            consider(db, &decision, &topology, format)
        }
        DecisionCommands::Unconsider { decision, topology } => {
            unconsider(db, &decision, &topology, format)
        }
        DecisionCommands::Choose {
            decision,
            topology,
            rationale,
        } => choose(db, &decision, &topology, &rationale, format),
        DecisionCommands::Abandon { decision, reason } => {
            abandon(db, &decision, reason.as_deref(), format)
        }
        DecisionCommands::Reopen { decision } => reopen(db, &decision, format),
    }
}

// ---------------------------------------------------------------------------
// DEC-01: Create
// ---------------------------------------------------------------------------

fn create(
    db: &mut Database,
    title: &str,
    description: &str,
    parent: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    // Validate title uniqueness
    let existing: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM decisions WHERE title = ?1",
        params![title],
        |row| row.get(0),
    )?;
    if existing > 0 {
        bail!("Decision title '{}' already exists", title);
    }

    // Resolve parent if provided
    let parent_id = match parent {
        Some(p) => {
            let parent_decision = resolve_decision(db, p)?;
            Some(parent_decision.id)
        }
        None => None,
    };

    let mut decision = Decision::new(title);
    decision.description = description.to_string();
    decision.parent_id = parent_id;

    let after_json = decision.to_json()?;
    let decision_id = decision.id.clone();
    let decision_title = decision.title.clone();

    db.transaction(|tx| {
        decision.insert(tx)?;

        record_event(
            tx,
            "decision.created",
            "decision",
            &decision_id,
            &format!("Created decision '{}'", decision_title),
            None,
            Some(&after_json),
            &EventSource::User,
        )?;

        Ok(())
    })?;

    let id_prefix = &decision_id[..8];
    match format {
        OutputFormat::Text => {
            println!("Created decision '{}' (id: {})", decision_title, id_prefix);
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "action": "created",
                "decision": decision_title,
                "id": decision_id,
                "status": "draft",
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// DEC-02: Show
// ---------------------------------------------------------------------------

fn show(db: &mut Database, name: &str, format: OutputFormat) -> Result<()> {
    let decision = resolve_decision(db, name)?;

    // Query constraints
    let constraints: Vec<DecisionConstraint> = {
        let mut stmt = db.conn().prepare(
            "SELECT id, decision_id, constraint_type, max_value, created_at \
             FROM decision_constraints WHERE decision_id = ?1 ORDER BY constraint_type",
        )?;
        let result = stmt
            .query_map(params![decision.id], DecisionConstraint::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        result
    };

    // Query considered topologies (with topology name via JOIN)
    let considered: Vec<(String, String, String)> = {
        let mut stmt = db.conn().prepare(
            "SELECT dt.id, dt.topology_id, t.name, dt.added_at \
             FROM decision_topologies dt \
             JOIN topologies t ON dt.topology_id = t.id \
             WHERE dt.decision_id = ?1 ORDER BY t.name",
        )?;
        let result = stmt
            .query_map(params![decision.id], |row| {
                let topo_name: String = row.get(2)?;
                let added_at: String = row.get(3)?;
                let topo_id: String = row.get(1)?;
                Ok((topo_name, added_at, topo_id))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        result
    };

    // Query parent title if parent_id set
    let parent_title: Option<String> = decision.parent_id.as_ref().and_then(|pid| {
        db.conn()
            .query_row("SELECT title FROM decisions WHERE id = ?1", [pid], |row| {
                row.get(0)
            })
            .ok()
    });

    // Query chosen topology name if set
    let chosen_topo_name: Option<String> = decision.chosen_topology_id.as_ref().and_then(|tid| {
        db.conn()
            .query_row("SELECT name FROM topologies WHERE id = ?1", [tid], |row| {
                row.get(0)
            })
            .ok()
    });

    match format {
        OutputFormat::Text => {
            println!("Decision: {}", decision.title);
            println!("  Status:          {}", decision.status);
            if !decision.description.is_empty() {
                println!("  Description:     {}", decision.description);
            }
            if let Some(ref pt) = parent_title {
                println!("  Parent:          {}", pt);
            }
            println!("  ID:              {}", decision.id);
            println!(
                "  Created:         {}",
                decision.created_at.format("%Y-%m-%d %H:%M:%S")
            );
            println!(
                "  Updated:         {}",
                decision.updated_at.format("%Y-%m-%d %H:%M:%S")
            );
            if let Some(closed) = decision.closed_at {
                println!("  Closed:          {}", closed.format("%Y-%m-%d %H:%M:%S"));
            }
            if let Some(ref tn) = chosen_topo_name {
                println!("  Chosen topology: {}", tn);
            }
            if let Some(ref r) = decision.rationale {
                println!("  Rationale:       {}", r);
            }

            println!();
            println!("  Constraints:");
            if constraints.is_empty() {
                println!("    (none)");
            } else {
                for c in &constraints {
                    let unit = constraint_unit(&c.constraint_type);
                    println!(
                        "    {}: max {}{}",
                        c.constraint_type,
                        format_constraint_value(&c.constraint_type, c.max_value),
                        unit
                    );
                }
            }

            println!();
            println!("  Considered topologies:");
            if considered.is_empty() {
                println!("    (none)");
            } else {
                for (topo_name, added_at, _topo_id) in &considered {
                    let date = &added_at[..10]; // YYYY-MM-DD prefix
                    println!("    {} (added {})", topo_name, date);
                }
            }
        }
        OutputFormat::Json => {
            let constraint_json: Vec<serde_json::Value> = constraints
                .iter()
                .map(|c| {
                    serde_json::json!({
                        "id": c.id,
                        "constraint_type": c.constraint_type,
                        "max_value": c.max_value,
                    })
                })
                .collect();

            let topo_json: Vec<serde_json::Value> = considered
                .iter()
                .map(|(name, added_at, topo_id)| {
                    serde_json::json!({
                        "topology_id": topo_id,
                        "name": name,
                        "added_at": added_at,
                    })
                })
                .collect();

            let json = serde_json::json!({
                "id": decision.id,
                "title": decision.title,
                "description": decision.description,
                "status": decision.status,
                "parent_id": decision.parent_id,
                "parent_title": parent_title,
                "chosen_topology_id": decision.chosen_topology_id,
                "chosen_topology_name": chosen_topo_name,
                "rationale": decision.rationale,
                "created_at": decision.created_at.to_rfc3339(),
                "updated_at": decision.updated_at.to_rfc3339(),
                "closed_at": decision.closed_at.map(|dt| dt.to_rfc3339()),
                "constraints": constraint_json,
                "considered_topologies": topo_json,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// DEC-03: List
// ---------------------------------------------------------------------------

fn list(db: &mut Database, status_filter: Option<&str>, format: OutputFormat) -> Result<()> {
    // Validate status filter
    if let Some(s) = status_filter {
        match s {
            "draft" | "open" | "decided" | "abandoned" => {}
            _ => bail!(
                "Invalid status '{}'. Must be one of: draft, open, decided, abandoned",
                s
            ),
        }
    }

    let decisions: Vec<Decision> = if let Some(status) = status_filter {
        let mut stmt = db.conn().prepare(
            "SELECT id, title, description, status, parent_id, chosen_topology_id, \
             rationale, snapshot, created_at, updated_at, closed_at \
             FROM decisions WHERE status = ?1 ORDER BY updated_at DESC",
        )?;
        let result = stmt
            .query_map(params![status], Decision::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        result
    } else {
        let mut stmt = db.conn().prepare(
            "SELECT id, title, description, status, parent_id, chosen_topology_id, \
             rationale, snapshot, created_at, updated_at, closed_at \
             FROM decisions ORDER BY updated_at DESC",
        )?;
        let result = stmt
            .query_map([], Decision::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        result
    };

    match format {
        OutputFormat::Text => {
            if decisions.is_empty() {
                println!("No decisions found. Create one with 'sp decision create <title>'");
            } else {
                for d in &decisions {
                    // Count constraints and topologies
                    let constraint_count: i64 = db.conn().query_row(
                        "SELECT COUNT(*) FROM decision_constraints WHERE decision_id = ?1",
                        params![d.id],
                        |row| row.get(0),
                    )?;
                    let topo_count: i64 = db.conn().query_row(
                        "SELECT COUNT(*) FROM decision_topologies WHERE decision_id = ?1",
                        params![d.id],
                        |row| row.get(0),
                    )?;

                    if d.status == "decided" {
                        // Show chosen topology name
                        let chosen_name: Option<String> =
                            d.chosen_topology_id.as_ref().and_then(|tid| {
                                db.conn()
                                    .query_row(
                                        "SELECT name FROM topologies WHERE id = ?1",
                                        [tid],
                                        |row| row.get(0),
                                    )
                                    .ok()
                            });
                        if let Some(ref name) = chosen_name {
                            println!("  [{}]  {}  (chose: {})", d.status, d.title, name);
                        } else {
                            println!("  [{}]  {}", d.status, d.title);
                        }
                    } else {
                        println!(
                            "  [{}]  {}  ({} constraint{}, {} topolog{})",
                            d.status,
                            d.title,
                            constraint_count,
                            if constraint_count == 1 { "" } else { "s" },
                            topo_count,
                            if topo_count == 1 { "y" } else { "ies" },
                        );
                    }
                }
            }
        }
        OutputFormat::Json => {
            let json: Vec<serde_json::Value> = decisions
                .iter()
                .map(|d| {
                    serde_json::json!({
                        "id": d.id,
                        "title": d.title,
                        "description": d.description,
                        "status": d.status,
                        "parent_id": d.parent_id,
                        "chosen_topology_id": d.chosen_topology_id,
                        "rationale": d.rationale,
                        "created_at": d.created_at.to_rfc3339(),
                        "updated_at": d.updated_at.to_rfc3339(),
                        "closed_at": d.closed_at.map(|dt| dt.to_rfc3339()),
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// DEC-04: Update
// ---------------------------------------------------------------------------

fn update(
    db: &mut Database,
    name: &str,
    title: Option<&str>,
    description: Option<&str>,
    open: bool,
    format: OutputFormat,
) -> Result<()> {
    if title.is_none() && description.is_none() && !open {
        bail!("Nothing to update. Provide --title, --description, or --open.");
    }

    let decision = resolve_decision(db, name)?;
    let before_json = decision.to_json()?;
    let decision_id = decision.id.clone();
    let original_title = decision.title.clone();

    // Check title uniqueness if renaming
    if let Some(new_title) = title {
        if new_title != original_title {
            let existing: i64 = db.conn().query_row(
                "SELECT COUNT(*) FROM decisions WHERE title = ?1 AND id != ?2",
                params![new_title, decision_id],
                |row| row.get(0),
            )?;
            if existing > 0 {
                bail!("Decision title '{}' already exists", new_title);
            }
        }
    }

    // Validate --open transition: only draft -> open
    if open && decision.status != "draft" {
        bail!(
            "Cannot open decision '{}': current status is '{}' (must be 'draft')",
            original_title,
            decision.status
        );
    }

    // Build after state
    let mut after = decision.clone();
    if let Some(t) = title {
        after.title = t.to_string();
    }
    if let Some(d) = description {
        after.description = d.to_string();
    }
    if open {
        after.status = "open".to_string();
    }
    let after_json = after.to_json()?;
    let final_title = after.title.clone();

    db.transaction(|tx| {
        if let Some(new_title) = title {
            tx.execute(
                "UPDATE decisions SET title = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![new_title, decision_id],
            )?;
        }
        if let Some(desc) = description {
            tx.execute(
                "UPDATE decisions SET description = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![desc, decision_id],
            )?;
        }
        if open {
            tx.execute(
                "UPDATE decisions SET status = 'open', updated_at = datetime('now') WHERE id = ?1",
                params![decision_id],
            )?;
        }

        record_event(
            tx,
            "decision.updated",
            "decision",
            &decision_id,
            &format!("Updated decision '{}'", original_title),
            Some(&before_json),
            Some(&after_json),
            &EventSource::User,
        )?;

        Ok(())
    })?;

    match format {
        OutputFormat::Text => {
            println!("Updated decision '{}'", final_title);
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "action": "updated",
                "decision": final_title,
                "id": decision_id,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// DEC-05: Constrain
// ---------------------------------------------------------------------------

const VALID_CONSTRAINT_TYPES: &[&str] = &["budget", "noise", "power", "rack_units"];

fn constraint_unit(constraint_type: &str) -> &'static str {
    match constraint_type {
        "budget" => "",
        "noise" => " dB",
        "power" => " W",
        "rack_units" => " U",
        _ => "",
    }
}

fn format_constraint_value(constraint_type: &str, value: f64) -> String {
    match constraint_type {
        "budget" => format!("${:.2}", value),
        _ => format!("{}", value),
    }
}

fn constrain(
    db: &mut Database,
    name: &str,
    constraint_type: &str,
    max_value: f64,
    format: OutputFormat,
) -> Result<()> {
    // Validate constraint type
    if !VALID_CONSTRAINT_TYPES.contains(&constraint_type) {
        bail!(
            "Invalid constraint type '{}'. Must be one of: {}",
            constraint_type,
            VALID_CONSTRAINT_TYPES.join(", ")
        );
    }

    let decision = resolve_decision(db, name)?;
    let decision_id = decision.id.clone();
    let decision_title = decision.title.clone();

    // Check if constraint already exists (upsert behavior)
    let existing: Option<(String, f64)> = db
        .conn()
        .query_row(
            "SELECT id, max_value FROM decision_constraints \
             WHERE decision_id = ?1 AND constraint_type = ?2",
            params![decision_id, constraint_type],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .ok();

    if let Some((existing_id, old_value)) = existing {
        // Update existing: delete old + insert new
        let old_constraint_json = serde_json::to_string(&serde_json::json!({
            "id": existing_id,
            "decision_id": decision_id,
            "constraint_type": constraint_type,
            "max_value": old_value,
        }))?;

        let new_constraint = DecisionConstraint::new(&decision_id, constraint_type, max_value);
        let new_constraint_json = new_constraint.to_json()?;
        let new_constraint_id = new_constraint.id.clone();

        db.transaction(|tx| {
            tx.execute(
                "DELETE FROM decision_constraints WHERE id = ?1",
                params![existing_id],
            )?;
            new_constraint.insert(tx)?;

            record_event(
                tx,
                "decision_constraint.updated",
                "decision_constraint",
                &new_constraint_id,
                &format!(
                    "Updated constraint '{}' on decision '{}': {} -> {}",
                    constraint_type, decision_title, old_value, max_value
                ),
                Some(&old_constraint_json),
                Some(&new_constraint_json),
                &EventSource::User,
            )?;

            Ok(())
        })?;

        match format {
            OutputFormat::Text => {
                println!(
                    "Updated constraint: {} max {} (was {})",
                    constraint_type,
                    format_constraint_value(constraint_type, max_value),
                    format_constraint_value(constraint_type, old_value),
                );
            }
            OutputFormat::Json => {
                let json = serde_json::json!({
                    "action": "updated",
                    "decision": decision_title,
                    "constraint_type": constraint_type,
                    "max_value": max_value,
                    "previous_max_value": old_value,
                });
                println!("{}", serde_json::to_string_pretty(&json)?);
            }
        }
    } else {
        // Insert new constraint
        let constraint = DecisionConstraint::new(&decision_id, constraint_type, max_value);
        let after_json = constraint.to_json()?;
        let constraint_id = constraint.id.clone();

        db.transaction(|tx| {
            constraint.insert(tx)?;

            record_event(
                tx,
                "decision_constraint.created",
                "decision_constraint",
                &constraint_id,
                &format!(
                    "Added constraint '{}' max {} to decision '{}'",
                    constraint_type, max_value, decision_title
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
                    "Added constraint: {} max {}",
                    constraint_type,
                    format_constraint_value(constraint_type, max_value),
                );
            }
            OutputFormat::Json => {
                let json = serde_json::json!({
                    "action": "constrained",
                    "decision": decision_title,
                    "constraint_type": constraint_type,
                    "max_value": max_value,
                });
                println!("{}", serde_json::to_string_pretty(&json)?);
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// DEC-06: Unconstrain
// ---------------------------------------------------------------------------

fn unconstrain(
    db: &mut Database,
    name: &str,
    constraint_type: &str,
    format: OutputFormat,
) -> Result<()> {
    // Validate constraint type
    if !VALID_CONSTRAINT_TYPES.contains(&constraint_type) {
        bail!(
            "Invalid constraint type '{}'. Must be one of: {}",
            constraint_type,
            VALID_CONSTRAINT_TYPES.join(", ")
        );
    }

    let decision = resolve_decision(db, name)?;
    let decision_id = decision.id.clone();
    let decision_title = decision.title.clone();

    // Find the constraint
    let constraint: DecisionConstraint = {
        let mut stmt = db.conn().prepare(
            "SELECT id, decision_id, constraint_type, max_value, created_at \
             FROM decision_constraints WHERE decision_id = ?1 AND constraint_type = ?2",
        )?;
        stmt.query_row(
            params![decision_id, constraint_type],
            DecisionConstraint::from_row,
        )
        .map_err(|_| {
            anyhow::anyhow!(
                "No '{}' constraint found on decision '{}'",
                constraint_type,
                decision_title
            )
        })?
    };

    let before_json = constraint.to_json()?;
    let constraint_id = constraint.id.clone();

    db.transaction(|tx| {
        tx.execute(
            "DELETE FROM decision_constraints WHERE id = ?1",
            params![constraint_id],
        )?;

        record_event(
            tx,
            "decision_constraint.deleted",
            "decision_constraint",
            &constraint_id,
            &format!(
                "Removed constraint '{}' from decision '{}'",
                constraint_type, decision_title
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
                "Removed constraint: {} from '{}'",
                constraint_type, decision_title
            );
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "action": "unconstrained",
                "decision": decision_title,
                "constraint_type": constraint_type,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// DEC-07: Consider
// ---------------------------------------------------------------------------

fn consider(
    db: &mut Database,
    name: &str,
    topology_name: &str,
    format: OutputFormat,
) -> Result<()> {
    let decision = resolve_decision(db, name)?;
    let topo = resolve_topology(db, topology_name)?;

    let decision_id = decision.id.clone();
    let decision_title = decision.title.clone();
    let topo_id = topo.id.clone();
    let topo_name = topo.name.clone();

    // Check if already considered
    let already: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM decision_topologies \
         WHERE decision_id = ?1 AND topology_id = ?2",
        params![decision_id, topo_id],
        |row| row.get(0),
    )?;
    if already > 0 {
        bail!(
            "Topology '{}' is already under consideration for decision '{}'",
            topo_name,
            decision_title
        );
    }

    let dt = DecisionTopology::new(&decision_id, &topo_id);
    let after_json = dt.to_json()?;
    let dt_id = dt.id.clone();

    db.transaction(|tx| {
        dt.insert(tx)?;

        record_event(
            tx,
            "decision_topology.created",
            "decision_topology",
            &dt_id,
            &format!(
                "Considering topology '{}' for decision '{}'",
                topo_name, decision_title
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
                "Considering topology '{}' for decision '{}'",
                topo_name, decision_title
            );
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "action": "considered",
                "decision": decision_title,
                "topology": topo_name,
                "topology_id": topo_id,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// DEC-08: Unconsider
// ---------------------------------------------------------------------------

fn unconsider(
    db: &mut Database,
    name: &str,
    topology_name: &str,
    format: OutputFormat,
) -> Result<()> {
    let decision = resolve_decision(db, name)?;
    let topo = resolve_topology(db, topology_name)?;

    let decision_id = decision.id.clone();
    let decision_title = decision.title.clone();
    let topo_id = topo.id.clone();
    let topo_name = topo.name.clone();

    // Find the junction row
    let dt_id: String = db
        .conn()
        .query_row(
            "SELECT id FROM decision_topologies \
             WHERE decision_id = ?1 AND topology_id = ?2",
            params![decision_id, topo_id],
            |row| row.get(0),
        )
        .map_err(|_| {
            anyhow::anyhow!(
                "Topology '{}' is not under consideration for decision '{}'",
                topo_name,
                decision_title
            )
        })?;

    // Get the full record for before_state
    let dt: DecisionTopology = {
        let mut stmt = db.conn().prepare(
            "SELECT id, decision_id, topology_id, added_at \
             FROM decision_topologies WHERE id = ?1",
        )?;
        stmt.query_row(params![dt_id], DecisionTopology::from_row)?
    };
    let before_json = dt.to_json()?;

    db.transaction(|tx| {
        tx.execute(
            "DELETE FROM decision_topologies WHERE id = ?1",
            params![dt_id],
        )?;

        record_event(
            tx,
            "decision_topology.deleted",
            "decision_topology",
            &dt_id,
            &format!(
                "Removed topology '{}' from consideration for decision '{}'",
                topo_name, decision_title
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
                "Removed topology '{}' from consideration for decision '{}'",
                topo_name, decision_title
            );
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "action": "unconsidered",
                "decision": decision_title,
                "topology": topo_name,
                "topology_id": topo_id,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// DEC-09: Choose
// ---------------------------------------------------------------------------

fn choose(
    db: &mut Database,
    name: &str,
    topology_name: &str,
    rationale: &str,
    format: OutputFormat,
) -> Result<()> {
    let decision = resolve_decision(db, name)?;
    let topo = resolve_topology(db, topology_name)?;

    let decision_id = decision.id.clone();
    let decision_title = decision.title.clone();
    let topo_id = topo.id.clone();
    let topo_name = topo.name.clone();

    // Validate state machine: must be open
    if decision.status != "open" {
        bail!(
            "Cannot choose for decision in '{}' state (must be 'open')",
            decision.status
        );
    }

    // Validate topology is being considered
    let is_considered: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM decision_topologies WHERE decision_id = ?1 AND topology_id = ?2",
        params![decision_id, topo_id],
        |row| row.get(0),
    )?;
    if is_considered == 0 {
        bail!(
            "Topology '{}' is not being considered for this decision",
            topo_name
        );
    }

    // Generate snapshot: load constraints and compute metrics for all considered topologies
    let constraints: Vec<DecisionConstraint> = {
        let mut stmt = db.conn().prepare(
            "SELECT id, decision_id, constraint_type, max_value, created_at \
             FROM decision_constraints WHERE decision_id = ?1 ORDER BY constraint_type",
        )?;
        let result = stmt
            .query_map(params![decision_id], DecisionConstraint::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        result
    };

    // Get all considered topology IDs and names
    let considered_topos: Vec<(String, String)> = {
        let mut stmt = db.conn().prepare(
            "SELECT dt.topology_id, t.name \
             FROM decision_topologies dt \
             JOIN topologies t ON dt.topology_id = t.id \
             WHERE dt.decision_id = ?1 ORDER BY t.name",
        )?;
        let result = stmt
            .query_map(params![decision_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        result
    };

    let mut snapshot_topos = Vec::new();
    for (ct_id, ct_name) in &considered_topos {
        let nodes = load_nodes_for_topology(db, ct_id)?;
        let volumes = load_volumes_for_topology(db, ct_id)?;
        let datasets = load_datasets_for_topology(db, ct_id)?;
        let placements = load_placements_with_context(db, ct_id)?;
        let sync_regimes = load_sync_regimes_with_context(db, ct_id)?;

        let metrics = compute_topology_metrics(
            ct_name,
            ct_id,
            &nodes,
            &volumes,
            &datasets,
            &placements,
            &sync_regimes,
            12,
        );

        let constraint_report = if !constraints.is_empty() {
            Some(check_constraints(&constraints, &nodes))
        } else {
            None
        };

        snapshot_topos.push(serde_json::json!({
            "name": ct_name,
            "id": ct_id,
            "metrics": metrics,
            "constraints": constraint_report,
        }));
    }

    let snapshot_json = serde_json::json!({
        "considered_topologies": snapshot_topos,
    });
    let snapshot_str = serde_json::to_string(&snapshot_json)?;

    let before_json = decision.to_json()?;

    // Build after state
    let mut after = decision.clone();
    after.status = "decided".to_string();
    after.chosen_topology_id = Some(topo_id.clone());
    after.rationale = Some(rationale.to_string());
    after.snapshot = Some(snapshot_str.clone());
    let after_json = after.to_json()?;

    let now = Utc::now().to_rfc3339();

    db.transaction(|tx| {
        tx.execute(
            "UPDATE decisions SET status = 'decided', chosen_topology_id = ?1, rationale = ?2, \
             snapshot = ?3, closed_at = ?4, updated_at = ?5 WHERE id = ?6",
            params![topo_id, rationale, snapshot_str, now, now, decision_id],
        )?;

        record_event(
            tx,
            "decision.updated",
            "decision",
            &decision_id,
            &format!(
                "Decided '{}' -- chose topology '{}'",
                decision_title, topo_name
            ),
            Some(&before_json),
            Some(&after_json),
            &EventSource::User,
        )?;

        Ok(())
    })?;

    match format {
        OutputFormat::Text => {
            println!(
                "Decided: '{}' -- chose topology '{}'",
                decision_title, topo_name
            );
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "action": "decided",
                "decision": decision_title,
                "id": decision_id,
                "chosen_topology": topo_name,
                "chosen_topology_id": topo_id,
                "rationale": rationale,
                "status": "decided",
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// DEC-10: Abandon
// ---------------------------------------------------------------------------

fn abandon(
    db: &mut Database,
    name: &str,
    reason: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    let decision = resolve_decision(db, name)?;

    let decision_id = decision.id.clone();
    let decision_title = decision.title.clone();

    // Validate state machine: must be draft or open
    match decision.status.as_str() {
        "draft" | "open" => {}
        "decided" => bail!(
            "Cannot abandon decision '{}': already decided. Use 'reopen' first.",
            decision_title
        ),
        "abandoned" => bail!("Decision '{}' is already abandoned", decision_title),
        other => bail!("Cannot abandon decision in '{}' state", other),
    }

    // Generate snapshot (same as choose but no chosen topology)
    let constraints: Vec<DecisionConstraint> = {
        let mut stmt = db.conn().prepare(
            "SELECT id, decision_id, constraint_type, max_value, created_at \
             FROM decision_constraints WHERE decision_id = ?1 ORDER BY constraint_type",
        )?;
        let result = stmt
            .query_map(params![decision_id], DecisionConstraint::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        result
    };

    let considered_topos: Vec<(String, String)> = {
        let mut stmt = db.conn().prepare(
            "SELECT dt.topology_id, t.name \
             FROM decision_topologies dt \
             JOIN topologies t ON dt.topology_id = t.id \
             WHERE dt.decision_id = ?1 ORDER BY t.name",
        )?;
        let result = stmt
            .query_map(params![decision_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        result
    };

    let mut snapshot_topos = Vec::new();
    for (ct_id, ct_name) in &considered_topos {
        let nodes = load_nodes_for_topology(db, ct_id)?;
        let volumes = load_volumes_for_topology(db, ct_id)?;
        let datasets = load_datasets_for_topology(db, ct_id)?;
        let placements = load_placements_with_context(db, ct_id)?;
        let sync_regimes = load_sync_regimes_with_context(db, ct_id)?;

        let metrics = compute_topology_metrics(
            ct_name,
            ct_id,
            &nodes,
            &volumes,
            &datasets,
            &placements,
            &sync_regimes,
            12,
        );

        let constraint_report = if !constraints.is_empty() {
            Some(check_constraints(&constraints, &nodes))
        } else {
            None
        };

        snapshot_topos.push(serde_json::json!({
            "name": ct_name,
            "id": ct_id,
            "metrics": metrics,
            "constraints": constraint_report,
        }));
    }

    let snapshot_json = serde_json::json!({
        "considered_topologies": snapshot_topos,
    });
    let snapshot_str = serde_json::to_string(&snapshot_json)?;

    let before_json = decision.to_json()?;

    let mut after = decision.clone();
    after.status = "abandoned".to_string();
    after.rationale = reason.map(|s| s.to_string());
    after.snapshot = Some(snapshot_str.clone());
    let after_json = after.to_json()?;

    let now = Utc::now().to_rfc3339();

    db.transaction(|tx| {
        tx.execute(
            "UPDATE decisions SET status = 'abandoned', rationale = ?1, \
             snapshot = ?2, closed_at = ?3, updated_at = ?4 WHERE id = ?5",
            params![reason, snapshot_str, now, now, decision_id],
        )?;

        record_event(
            tx,
            "decision.updated",
            "decision",
            &decision_id,
            &format!("Abandoned decision '{}'", decision_title),
            Some(&before_json),
            Some(&after_json),
            &EventSource::User,
        )?;

        Ok(())
    })?;

    match format {
        OutputFormat::Text => {
            println!("Abandoned decision '{}'", decision_title);
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "action": "abandoned",
                "decision": decision_title,
                "id": decision_id,
                "reason": reason,
                "status": "abandoned",
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// DEC-11: Reopen
// ---------------------------------------------------------------------------

fn reopen(db: &mut Database, name: &str, format: OutputFormat) -> Result<()> {
    let decision = resolve_decision(db, name)?;

    let decision_id = decision.id.clone();
    let decision_title = decision.title.clone();

    // Validate state machine: must be decided or abandoned
    match decision.status.as_str() {
        "decided" | "abandoned" => {}
        "draft" | "open" => bail!(
            "Decision '{}' is already {}",
            decision_title,
            decision.status
        ),
        other => bail!("Cannot reopen decision in '{}' state", other),
    }

    let before_json = decision.to_json()?;

    let mut after = decision.clone();
    after.status = "open".to_string();
    after.chosen_topology_id = None;
    after.rationale = None;
    after.closed_at = None;
    let after_json = after.to_json()?;

    let now = Utc::now().to_rfc3339();

    db.transaction(|tx| {
        tx.execute(
            "UPDATE decisions SET status = 'open', chosen_topology_id = NULL, rationale = NULL, \
             closed_at = NULL, updated_at = ?1 WHERE id = ?2",
            params![now, decision_id],
        )?;

        record_event(
            tx,
            "decision.updated",
            "decision",
            &decision_id,
            &format!("Reopened decision '{}'", decision_title),
            Some(&before_json),
            Some(&after_json),
            &EventSource::User,
        )?;

        Ok(())
    })?;

    match format {
        OutputFormat::Text => {
            println!("Reopened decision '{}'", decision_title);
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "action": "reopened",
                "decision": decision_title,
                "id": decision_id,
                "status": "open",
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Shared helpers for decision commands
// ---------------------------------------------------------------------------

fn load_nodes_for_topology(db: &Database, topology_id: &str) -> Result<Vec<Node>> {
    let results = {
        let mut stmt = db.conn().prepare(
            "SELECT id, topology_id, name, role, location, available_bays, interface_types, \
             power_draw_watts, cost_estimate, noise_db, rack_units, item_id, created_at, updated_at \
             FROM nodes WHERE topology_id = ?1 ORDER BY name",
        )?;
        let rows = stmt.query_map(params![topology_id], Node::from_row)?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    Ok(results)
}

fn load_volumes_for_topology(db: &Database, topology_id: &str) -> Result<Vec<Volume>> {
    let results = {
        let mut stmt = db.conn().prepare(
            "SELECT id, topology_id, node_id, name, capacity_bytes, usable_bytes, \
             filesystem, raid_level, pool_type, item_id, created_at, updated_at \
             FROM volumes WHERE topology_id = ?1 ORDER BY name",
        )?;
        let rows = stmt.query_map(params![topology_id], Volume::from_row)?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    Ok(results)
}

fn load_datasets_for_topology(db: &Database, topology_id: &str) -> Result<Vec<Dataset>> {
    let results = {
        let mut stmt = db.conn().prepare(
            "SELECT id, topology_id, name, size_bytes, growth_rate_bytes_month, \
             criticality, min_copies, min_locations, max_rpo_hours, created_at, updated_at \
             FROM datasets WHERE topology_id = ?1 ORDER BY name",
        )?;
        let rows = stmt.query_map(params![topology_id], Dataset::from_row)?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    Ok(results)
}

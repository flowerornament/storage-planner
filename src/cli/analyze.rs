//! sp analyze -- Run analysis reports against topology data
//!
//! Subcommands: redundancy, capacity
//! (RPO and failure subcommands added in Plan 02)
//!
//! Each subcommand resolves the active topology, loads data, calls the
//! corresponding pure analysis function, and formats output.

use anyhow::Result;
use clap::Subcommand;
use console::style;
use rusqlite::params;

use crate::core::db::Database;
use crate::core::models::{Dataset, Volume};
use crate::core::resolve::resolve_active_topology;
use crate::core::specs::Capacity;
use crate::domains::storage::analysis::{
    analyze_capacity, analyze_redundancy, load_placements_with_context, CapacityReport,
    RedundancyReport,
};

use super::OutputFormat;

#[derive(Subcommand)]
pub enum AnalyzeCommands {
    /// Check dataset redundancy against min_copies and min_locations requirements
    Redundancy {
        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,

        /// Show all datasets, not just issues
        #[arg(long)]
        verbose: bool,
    },

    /// Project capacity usage and estimate time until volumes are full
    Capacity {
        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,

        /// Show all volumes with timeline, not just issues
        #[arg(long)]
        verbose: bool,

        /// Warn when volume is projected full within N months
        #[arg(long, default_value = "12")]
        warn_months: i32,
    },
}

pub fn run(cmd: AnalyzeCommands, db: &mut Database, format: OutputFormat) -> Result<()> {
    match cmd {
        AnalyzeCommands::Redundancy { topology, verbose } => {
            run_redundancy(db, topology.as_deref(), verbose, format)
        }
        AnalyzeCommands::Capacity {
            topology,
            verbose,
            warn_months,
        } => run_capacity(db, topology.as_deref(), verbose, warn_months, format),
    }
}

// ---------------------------------------------------------------------------
// Redundancy
// ---------------------------------------------------------------------------

fn run_redundancy(
    db: &mut Database,
    topology_override: Option<&str>,
    verbose: bool,
    format: OutputFormat,
) -> Result<()> {
    let topo = resolve_active_topology(db, topology_override)?;
    let datasets = load_datasets(db, &topo.id)?;
    let placements = load_placements_with_context(db, &topo.id)?;

    let report = analyze_redundancy(&datasets, &placements);
    let has_issues = !report.issues.is_empty();

    match format {
        OutputFormat::Text => print_redundancy_text(&report, &datasets, &placements, verbose),
        OutputFormat::Json => print_redundancy_json(&report, &topo.name, &topo.id)?,
    }

    if has_issues {
        std::process::exit(1);
    }
    Ok(())
}

fn print_redundancy_text(
    report: &RedundancyReport,
    datasets: &[Dataset],
    placements: &[crate::domains::storage::analysis::PlacementWithContext],
    verbose: bool,
) {
    let issue_count = report.issues.len();
    let detail = format!("{} issue(s)", issue_count);
    print_analysis_header("Redundancy", report.score, &detail);

    if report.dataset_count == 0 {
        println!("  No datasets found.");
        return;
    }

    if verbose {
        // Show all datasets
        let issue_names: std::collections::HashSet<&str> = report
            .issues
            .iter()
            .map(|i| i.dataset_name.as_str())
            .collect();

        for ds in datasets {
            if issue_names.contains(ds.name.as_str()) {
                // Find the issue for this dataset
                if let Some(issue) = report.issues.iter().find(|i| i.dataset_name == ds.name) {
                    print!(
                        "  {} {} [{}]:",
                        style("[FAIL]").red().bold(),
                        ds.name,
                        ds.criticality
                    );
                    for problem in &issue.problems {
                        print!(" {}", problem);
                    }
                    println!();
                    if let Some(ref suggestion) = issue.suggestion {
                        println!("         -> {}", style(suggestion).yellow());
                    }
                }
            } else {
                // Count copies and locations for passing dataset
                let ds_placements: Vec<_> =
                    placements.iter().filter(|p| p.dataset_id == ds.id).collect();
                let copies = ds_placements.len();
                println!(
                    "  {} {} [{}]: {} copy/copies",
                    style("[OK]").green().bold(),
                    ds.name,
                    ds.criticality,
                    copies
                );
            }
        }
    } else {
        // Show only issues
        for issue in &report.issues {
            print!(
                "  {} [{}]:",
                issue.dataset_name, issue.criticality
            );
            for problem in &issue.problems {
                print!(" {}", problem);
            }
            println!();
            if let Some(ref suggestion) = issue.suggestion {
                println!("    -> {}", style(suggestion).yellow());
            }
        }
    }
}

fn print_redundancy_json(report: &RedundancyReport, topo_name: &str, topo_id: &str) -> Result<()> {
    let json = serde_json::json!({
        "topology": topo_name,
        "topology_id": topo_id,
        "analysis": "redundancy",
        "report": report,
    });
    println!("{}", serde_json::to_string_pretty(&json)?);
    Ok(())
}

// ---------------------------------------------------------------------------
// Capacity
// ---------------------------------------------------------------------------

fn run_capacity(
    db: &mut Database,
    topology_override: Option<&str>,
    verbose: bool,
    warn_months: i32,
    format: OutputFormat,
) -> Result<()> {
    let topo = resolve_active_topology(db, topology_override)?;
    let datasets = load_datasets(db, &topo.id)?;
    let volumes = load_volumes(db, &topo.id)?;
    let placements = load_placements_with_context(db, &topo.id)?;

    let report = analyze_capacity(&datasets, &volumes, &placements, warn_months);
    let has_issues = !report.issues.is_empty();

    match format {
        OutputFormat::Text => print_capacity_text(&report, verbose),
        OutputFormat::Json => print_capacity_json(&report, &topo.name, &topo.id)?,
    }

    if has_issues {
        std::process::exit(1);
    }
    Ok(())
}

fn print_capacity_text(report: &CapacityReport, verbose: bool) {
    let warning_count = report.issues.len();
    let detail = format!("{} warning(s)", warning_count);
    print_analysis_header("Capacity", report.score, &detail);

    if report.projections.is_empty() {
        println!("  No volumes found.");
        return;
    }

    if verbose {
        // Timeline table for all volumes
        println!(
            "  {:<14}{:<12}{:<12}{:<12}{:<12}{:<12}{}",
            "Volume", "Node", "Now", "3mo", "6mo", "12mo", "Full"
        );

        for proj in &report.projections {
            let current = format_capacity_fraction(proj.current_used_bytes, proj.ceiling_bytes);

            let (col_3mo, col_6mo, col_12mo) = if proj.monthly_growth_bytes > 0.0 {
                let labels: Vec<String> = proj
                    .timeline
                    .iter()
                    .map(|tp| format_capacity_fraction(tp.projected_bytes, proj.ceiling_bytes))
                    .collect();
                (
                    labels.first().cloned().unwrap_or_else(|| "N/A".to_string()),
                    labels.get(1).cloned().unwrap_or_else(|| "N/A".to_string()),
                    labels.get(2).cloned().unwrap_or_else(|| "N/A".to_string()),
                )
            } else {
                ("N/A".to_string(), "N/A".to_string(), "N/A".to_string())
            };

            let full_col = match proj.months_until_full {
                Some(mtf) => format!("{:.0}mo", mtf),
                None => "N/A".to_string(),
            };

            // Determine if this volume has an issue
            let is_issue = report
                .issues
                .iter()
                .any(|i| i.volume_name == proj.volume_name && i.node_name == proj.node_name);

            let line = format!(
                "  {:<14}{:<12}{:<12}{:<12}{:<12}{:<12}{}",
                proj.volume_name, proj.node_name, current, col_3mo, col_6mo, col_12mo, full_col
            );

            if is_issue {
                println!("{}", style(line).yellow());
            } else {
                println!("{}", style(line).green());
            }
        }
    } else {
        // Show only issues
        for issue in &report.issues {
            let current_used = report
                .projections
                .iter()
                .find(|p| p.volume_name == issue.volume_name && p.node_name == issue.node_name)
                .map(|p| p.current_used_bytes)
                .unwrap_or(0);

            let used_str = Capacity::from_bytes(current_used as u64);
            let ceiling_str = Capacity::from_bytes(issue.ceiling_bytes as u64);

            println!(
                "  {} on {}: full in {:.0} months ({}/{})",
                issue.volume_name,
                issue.node_name,
                issue.months_until_full,
                used_str,
                ceiling_str
            );
        }
    }

    if !report.skipped_datasets.is_empty() {
        println!(
            "  Note: {} dataset(s) lack growth_rate data: {}",
            report.skipped_datasets.len(),
            report.skipped_datasets.join(", ")
        );
    }
}

fn print_capacity_json(report: &CapacityReport, topo_name: &str, topo_id: &str) -> Result<()> {
    let json = serde_json::json!({
        "topology": topo_name,
        "topology_id": topo_id,
        "analysis": "capacity",
        "report": report,
    });
    println!("{}", serde_json::to_string_pretty(&json)?);
    Ok(())
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Load all datasets for a topology.
fn load_datasets(db: &Database, topology_id: &str) -> Result<Vec<Dataset>> {
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

/// Load all volumes for a topology.
fn load_volumes(db: &Database, topology_id: &str) -> Result<Vec<Volume>> {
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

/// Print a colored analysis header line.
fn print_analysis_header(name: &str, score: f64, detail: &str) {
    let score_str = format!("{:.0}%", score);
    let colored_score = if score >= 100.0 {
        style(score_str).green().bold()
    } else if score >= 75.0 {
        style(score_str).yellow().bold()
    } else {
        style(score_str).red().bold()
    };

    println!("{}: {} ({})", name, colored_score, detail);
}

/// Format a capacity fraction like "2.1/4.0TB".
fn format_capacity_fraction(used: i64, total: i64) -> String {
    let used_cap = Capacity::from_bytes(used as u64);
    let total_cap = Capacity::from_bytes(total as u64);

    // Use the total's unit for both to keep consistent
    if total as u64 >= Capacity::TB {
        format!(
            "{:.1}/{:.1}TB",
            used as f64 / Capacity::TB as f64,
            total as f64 / Capacity::TB as f64
        )
    } else if total as u64 >= Capacity::GB {
        format!(
            "{:.1}/{:.1}GB",
            used as f64 / Capacity::GB as f64,
            total as f64 / Capacity::GB as f64
        )
    } else {
        format!("{}/{}", used_cap, total_cap)
    }
}

//! sp analyze -- Run analysis reports against topology data
//!
//! Subcommands: redundancy, capacity, rpo, failure, bandwidth, cost
//! Running `sp analyze` with no subcommand shows a combined dashboard.
//!
//! Each subcommand resolves the active topology, loads data, calls the
//! corresponding pure analysis function, and formats output.

use anyhow::{bail, Result};
use clap::Subcommand;
use console::style;
use rusqlite::params;

use crate::core::db::Database;
use crate::core::models::{Dataset, DecisionConstraint, Link, Node, Price, Volume};
use crate::core::resolve::{resolve_active_topology, resolve_decision, resolve_topology};
use crate::core::specs::Capacity;
use crate::domains::storage::analysis::{
    analyze_bandwidth, analyze_capacity, analyze_cost, analyze_redundancy, analyze_rpo,
    check_constraints, compare_topologies, compute_tco_cents, compute_topology_metrics,
    load_placements_with_context, load_sync_regimes_with_context, simulate_failure,
    BandwidthReport, BandwidthStatus, CapacityReport, ConstraintReport, ConstraintStatus,
    CostReport, EntityCost, FailureReport, PlacementWithContext, RedundancyReport, RpoReport,
    SyncRegimeWithContext,
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

    /// Check RPO compliance -- do sync schedules satisfy dataset max_rpo requirements?
    Rpo {
        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,

        /// Show all datasets with RPO requirements, not just issues
        #[arg(long)]
        verbose: bool,
    },

    /// Simulate node failure -- what volumes and datasets are impacted?
    Failure {
        /// Node name(s) to simulate failing (required, supports multiple)
        #[arg(required = true)]
        nodes: Vec<String>,

        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,

        /// Show remaining placement details per dataset
        #[arg(long)]
        verbose: bool,
    },

    /// Check decision constraints against a topology
    Constraints {
        /// Decision with constraints to check
        #[arg(long)]
        decision: String,

        /// Target topology to check (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },

    /// Check if network links support sync regime bandwidth requirements
    Bandwidth {
        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },

    /// Analyze topology cost (one-time + recurring from catalog items)
    Cost {
        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,

        /// Show category summary instead of per-entity breakdown
        #[arg(long)]
        summary: bool,

        /// Show total cost of ownership projection (e.g., --tco=3yr)
        #[arg(long, value_name = "PERIOD")]
        tco: Option<String>,
    },

    /// Compare two topologies side-by-side
    Compare {
        /// First topology name or ID
        a: String,

        /// Second topology name or ID
        b: String,

        /// Include structural diff
        #[arg(long)]
        diff: bool,

        /// Decision context for constraint checking
        #[arg(long)]
        decision: Option<String>,

        /// Warn threshold months for capacity scoring
        #[arg(long, default_value = "12")]
        warn_months: i32,
    },
}

/// Run analysis with optional subcommand. When None, runs the combined dashboard.
pub fn run(
    cmd: Option<AnalyzeCommands>,
    db: &mut Database,
    format: OutputFormat,
    topology_override: Option<String>,
    verbose: bool,
    warn_months: i32,
) -> Result<()> {
    match cmd {
        None => run_all(
            db,
            topology_override.as_deref(),
            verbose,
            warn_months,
            format,
        ),
        Some(AnalyzeCommands::Redundancy { topology, verbose }) => {
            run_redundancy(db, topology.as_deref(), verbose, format)
        }
        Some(AnalyzeCommands::Capacity {
            topology,
            verbose,
            warn_months,
        }) => run_capacity(db, topology.as_deref(), verbose, warn_months, format),
        Some(AnalyzeCommands::Rpo { topology, verbose }) => {
            run_rpo(db, topology.as_deref(), verbose, format)
        }
        Some(AnalyzeCommands::Failure {
            nodes,
            topology,
            verbose,
        }) => run_failure(db, topology.as_deref(), &nodes, verbose, format),
        Some(AnalyzeCommands::Bandwidth { topology }) => {
            run_bandwidth(db, topology.as_deref(), format)
        }
        Some(AnalyzeCommands::Cost {
            topology,
            summary,
            tco,
        }) => run_cost(db, topology.as_deref(), summary, tco.as_deref(), format),
        Some(AnalyzeCommands::Constraints { decision, topology }) => {
            run_constraints(db, &decision, topology.as_deref(), format)
        }
        Some(AnalyzeCommands::Compare {
            a,
            b,
            diff,
            decision,
            warn_months,
        }) => run_compare(db, &a, &b, diff, decision.as_deref(), warn_months, format),
    }
}

// ---------------------------------------------------------------------------
// Combined dashboard ("sp analyze" with no subcommand)
// ---------------------------------------------------------------------------

fn run_all(
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
    let sync_regimes = load_sync_regimes(db, &topo.id)?;

    let redundancy = analyze_redundancy(&datasets, &placements);
    let rpo = analyze_rpo(&datasets, &placements, &sync_regimes);
    let capacity = analyze_capacity(&datasets, &volumes, &placements, warn_months);

    match format {
        OutputFormat::Text => {
            let tag_str = topo
                .tag
                .as_deref()
                .map(|t| format!(" [{}]", t))
                .unwrap_or_default();
            println!(
                "Analysis: {}{}",
                style(&topo.name).bold(),
                style(&tag_str).dim()
            );
            println!();

            print_redundancy_text(&redundancy, &datasets, &placements, verbose);
            print_rpo_text(&rpo, verbose);
            print_capacity_text(&capacity, verbose);

            // One-line cost summary if any catalog items are linked
            let cost_entities = build_entity_costs(db, &load_nodes(db, &topo.id)?, &volumes)?;
            if cost_entities.iter().any(|e| e.item_id.is_some()) {
                let cost_report = analyze_cost(&cost_entities);
                let one_time_str =
                    format!("${:.2}", cost_report.one_time_total_cents as f64 / 100.0);
                let monthly_str =
                    format!("${:.2}/mo", cost_report.monthly_total_cents as f64 / 100.0);
                println!(
                    "Cost: {} one-time, {} recurring",
                    style(one_time_str).bold(),
                    style(monthly_str).bold(),
                );
            }
        }
        OutputFormat::Json => {
            let cost_entities = build_entity_costs(db, &load_nodes(db, &topo.id)?, &volumes)?;
            let cost_report = if cost_entities.iter().any(|e| e.item_id.is_some()) {
                Some(analyze_cost(&cost_entities))
            } else {
                None
            };
            let json = serde_json::json!({
                "topology": topo.name,
                "topology_id": topo.id,
                "analysis": "all",
                "redundancy": redundancy,
                "rpo": rpo,
                "capacity": capacity,
                "cost": cost_report,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
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

    match format {
        OutputFormat::Text => print_redundancy_text(&report, &datasets, &placements, verbose),
        OutputFormat::Json => print_redundancy_json(&report, &topo.name, &topo.id)?,
    }

    Ok(())
}

fn print_redundancy_text(
    report: &RedundancyReport,
    datasets: &[Dataset],
    placements: &[PlacementWithContext],
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
                let ds_placements: Vec<_> = placements
                    .iter()
                    .filter(|p| p.dataset_id == ds.id)
                    .collect();
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
            print!("  {} [{}]:", issue.dataset_name, issue.criticality);
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
// RPO
// ---------------------------------------------------------------------------

fn run_rpo(
    db: &mut Database,
    topology_override: Option<&str>,
    verbose: bool,
    format: OutputFormat,
) -> Result<()> {
    let topo = resolve_active_topology(db, topology_override)?;
    let datasets = load_datasets(db, &topo.id)?;
    let placements = load_placements_with_context(db, &topo.id)?;
    let sync_regimes = load_sync_regimes(db, &topo.id)?;

    let report = analyze_rpo(&datasets, &placements, &sync_regimes);

    match format {
        OutputFormat::Text => print_rpo_text(&report, verbose),
        OutputFormat::Json => print_rpo_json(&report, &topo.name, &topo.id)?,
    }

    Ok(())
}

fn print_rpo_text(report: &RpoReport, verbose: bool) {
    let issue_count = report.issues.len();
    let detail = format!("{} issue(s)", issue_count);
    print_analysis_header("RPO", report.score, &detail);

    if report.datasets_analyzed == 0 && report.datasets_skipped.is_empty() {
        println!("  No datasets found.");
        return;
    }

    if report.datasets_analyzed == 0 {
        println!("  No datasets have max_rpo_hours set.");
        return;
    }

    if verbose {
        // Show all datasets with RPO requirements
        for issue in &report.issues {
            let interval_str = match issue.best_sync_interval_hours {
                Some(h) => format!("{:.1}h", h),
                None => "N/A".to_string(),
            };
            println!(
                "  {} {} [{}]: sync={}, max_rpo={}h -- {}",
                style("[FAIL]").red().bold(),
                issue.dataset_name,
                issue.criticality,
                interval_str,
                issue.max_rpo_hours,
                issue.problem,
            );
            if let Some(ref suggestion) = issue.suggestion {
                println!("         -> {}", style(suggestion).yellow());
            }
        }

        // Note: in verbose, we don't have the OK datasets listed since
        // analyze_rpo doesn't return them -- we'd need to cross-reference.
        // Instead, show datasets that passed implicitly.
        if report.datasets_ok > 0 {
            let ok_count = report.datasets_ok;
            println!(
                "  {} {ok_count} dataset(s) meeting RPO requirements",
                style("[OK]").green().bold(),
            );
        }

        if !report.datasets_skipped.is_empty() {
            println!(
                "  Skipped (no max_rpo_hours): {}",
                report.datasets_skipped.join(", ")
            );
        }
    } else {
        // Show only issues
        for issue in &report.issues {
            println!(
                "  {} [{}]: {}",
                issue.dataset_name, issue.criticality, issue.problem,
            );
            if let Some(ref suggestion) = issue.suggestion {
                println!("    -> {}", style(suggestion).yellow());
            }
        }
    }
}

fn print_rpo_json(report: &RpoReport, topo_name: &str, topo_id: &str) -> Result<()> {
    let json = serde_json::json!({
        "topology": topo_name,
        "topology_id": topo_id,
        "analysis": "rpo",
        "report": report,
    });
    println!("{}", serde_json::to_string_pretty(&json)?);
    Ok(())
}

// ---------------------------------------------------------------------------
// Failure simulation
// ---------------------------------------------------------------------------

fn run_failure(
    db: &mut Database,
    topology_override: Option<&str>,
    node_names: &[String],
    verbose: bool,
    format: OutputFormat,
) -> Result<()> {
    let topo = resolve_active_topology(db, topology_override)?;
    let nodes = load_nodes(db, &topo.id)?;
    let datasets = load_datasets(db, &topo.id)?;
    let placements = load_placements_with_context(db, &topo.id)?;

    let report = simulate_failure(node_names, &nodes, &datasets, &placements)?;

    match format {
        OutputFormat::Text => print_failure_text(&report, &placements, verbose),
        OutputFormat::Json => print_failure_json(&report, &topo.name, &topo.id)?,
    }

    // Failure sim always exits 0 (exploratory)
    Ok(())
}

fn print_failure_text(report: &FailureReport, placements: &[PlacementWithContext], verbose: bool) {
    println!(
        "Failure simulation: {} offline",
        report
            .failed_nodes
            .iter()
            .map(|n| style(n).red().bold().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!();

    // Volumes lost
    println!(
        "Volumes lost ({}):",
        style(report.volume_impact.len()).bold()
    );
    if report.volume_impact.is_empty() {
        println!("  (none)");
    } else {
        for vol in &report.volume_impact {
            let cap = Capacity::from_bytes(vol.capacity_bytes as u64);
            println!(
                "  {} on {} ({}, {} dataset(s))",
                vol.volume_name, vol.node_name, cap, vol.datasets_hosted
            );
        }
    }
    println!();

    // Dataset impact
    println!(
        "Dataset impact ({}):",
        style(report.dataset_impact.len()).bold()
    );
    if report.dataset_impact.is_empty() {
        println!("  (none)");
    } else {
        for di in &report.dataset_impact {
            let severity_styled = match di.severity {
                crate::domains::storage::analysis::FailureSeverity::Lost => {
                    style(format!("[{}]", di.severity)).red().bold()
                }
                crate::domains::storage::analysis::FailureSeverity::Degraded => {
                    style(format!("[{}]", di.severity)).yellow().bold()
                }
                crate::domains::storage::analysis::FailureSeverity::AtRisk => {
                    style(format!("[{}]", di.severity)).yellow()
                }
            };
            println!(
                "  {} {} [{}]: {}/{} copies, {}/{} locations",
                severity_styled,
                di.dataset_name,
                di.criticality,
                di.remaining_copies,
                di.total_copies,
                di.remaining_locations,
                di.total_locations,
            );

            if verbose {
                // Show remaining placements
                let remaining: Vec<&PlacementWithContext> = placements
                    .iter()
                    .filter(|p| {
                        p.dataset_name == di.dataset_name
                            && !report.failed_nodes.contains(&p.node_name)
                    })
                    .collect();
                if remaining.is_empty() {
                    println!("    No remaining copies");
                } else {
                    for rp in &remaining {
                        println!(
                            "    Remaining: {} on {} ({})",
                            rp.volume_name, rp.node_name, rp.node_location
                        );
                    }
                }
            }
        }
    }
    println!();

    // Summary
    println!(
        "Summary: {} lost, {} degraded, {} at risk",
        style(report.summary.datasets_lost).red().bold(),
        style(report.summary.datasets_degraded).yellow().bold(),
        style(report.summary.datasets_at_risk).yellow(),
    );
}

fn print_failure_json(report: &FailureReport, topo_name: &str, topo_id: &str) -> Result<()> {
    let json = serde_json::json!({
        "topology": topo_name,
        "topology_id": topo_id,
        "analysis": "failure",
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

    match format {
        OutputFormat::Text => print_capacity_text(&report, verbose),
        OutputFormat::Json => print_capacity_json(&report, &topo.name, &topo.id)?,
    }

    Ok(())
}

#[allow(clippy::print_literal)]
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
                issue.volume_name, issue.node_name, issue.months_until_full, used_str, ceiling_str
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
// Bandwidth (ANLZ-06)
// ---------------------------------------------------------------------------

fn run_bandwidth(
    db: &mut Database,
    topology_override: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    let topo = resolve_active_topology(db, topology_override)?;
    let sync_regimes = load_sync_regimes(db, &topo.id)?;
    let volumes = load_volumes(db, &topo.id)?;
    let datasets = load_datasets(db, &topo.id)?;
    let links = load_links(db, &topo.id)?;

    let report = analyze_bandwidth(&sync_regimes, &volumes, &links, &datasets);

    // Build node_id -> node_name map for display
    let nodes = load_nodes(db, &topo.id)?;
    let node_names: std::collections::HashMap<&str, &str> = nodes
        .iter()
        .map(|n| (n.id.as_str(), n.name.as_str()))
        .collect();

    match format {
        OutputFormat::Text => {
            print_bandwidth_text(&report, &node_names);
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "topology": topo.name,
                "topology_id": topo.id,
                "analysis": "bandwidth",
                "report": report,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

fn print_bandwidth_text(
    report: &BandwidthReport,
    node_names: &std::collections::HashMap<&str, &str>,
) {
    let score_pct = report.score * 100.0;
    let issue_count = report
        .results
        .iter()
        .filter(|r| {
            r.status == BandwidthStatus::Insufficient || r.status == BandwidthStatus::NoLink
        })
        .count();
    let detail = format!("{} issue(s)", issue_count);
    print_analysis_header("Bandwidth", score_pct, &detail);

    if report.results.is_empty() {
        println!("  No sync regimes found.");
        return;
    }

    for result in &report.results {
        let src_fallback = result.source_node.as_str();
        let tgt_fallback = result.target_node.as_str();
        let src_display = node_names
            .get(result.source_node.as_str())
            .unwrap_or(&src_fallback);
        let tgt_display = node_names
            .get(result.target_node.as_str())
            .unwrap_or(&tgt_fallback);

        let link_bw_str = match result.link_bandwidth_bytes_sec {
            Some(bw) => format_bandwidth(bw),
            None => "N/A".to_string(),
        };
        let req_bw_str = format_bandwidth(result.required_bandwidth_bytes_sec);

        let status_styled = match result.status {
            BandwidthStatus::Adequate => style(format!("[{}]", result.status)).green().bold(),
            BandwidthStatus::Tight => style(format!("[{}]", result.status)).yellow().bold(),
            BandwidthStatus::Insufficient => style(format!("[{}]", result.status)).red().bold(),
            BandwidthStatus::NoLink => style(format!("[{}]", result.status)).red().bold(),
        };

        println!(
            "  {} {} ({}): {} -> {} | need {} | have {}",
            status_styled,
            result.sync_regime_name,
            result.dataset_name,
            src_display,
            tgt_display,
            req_bw_str,
            link_bw_str,
        );
    }
}

/// Format bandwidth in human-readable units.
fn format_bandwidth(bytes_sec: i64) -> String {
    let bps = bytes_sec as f64;
    if bps >= 1_000_000_000.0 {
        format!("{:.1} GB/s", bps / 1_000_000_000.0)
    } else if bps >= 1_000_000.0 {
        format!("{:.1} MB/s", bps / 1_000_000.0)
    } else if bps >= 1_000.0 {
        format!("{:.1} KB/s", bps / 1_000.0)
    } else {
        format!("{} B/s", bytes_sec)
    }
}

// ---------------------------------------------------------------------------
// Cost (ANLZ-07)
// ---------------------------------------------------------------------------

fn run_cost(
    db: &mut Database,
    topology_override: Option<&str>,
    summary_mode: bool,
    tco_period: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    let topo = resolve_active_topology(db, topology_override)?;
    let nodes = load_nodes(db, &topo.id)?;
    let volumes = load_volumes(db, &topo.id)?;

    let entities = build_entity_costs(db, &nodes, &volumes)?;
    let report = analyze_cost(&entities);

    // Parse TCO period if provided
    let tco_years = if let Some(period) = tco_period {
        Some(parse_tco_period(period)?)
    } else {
        None
    };

    match format {
        OutputFormat::Text => {
            if summary_mode {
                print_cost_summary_text(&report, tco_years);
            } else {
                print_cost_detail_text(&report, tco_years);
            }
        }
        OutputFormat::Json => {
            let mut json = serde_json::to_value(&report)?;
            if let Some(years) = tco_years {
                json["tco_cents"] = serde_json::json!(compute_tco_cents(&report, years));
                json["tco_years"] = serde_json::json!(years);
            }
            let wrapper = serde_json::json!({
                "topology": topo.name,
                "topology_id": topo.id,
                "analysis": "cost",
                "report": json,
            });
            println!("{}", serde_json::to_string_pretty(&wrapper)?);
        }
    }

    Ok(())
}

/// Build EntityCost entries by looking up latest prices for entities with item_id.
fn build_entity_costs(
    db: &Database,
    nodes: &[Node],
    volumes: &[Volume],
) -> Result<Vec<EntityCost>> {
    let mut entities = Vec::new();

    // Nodes
    for node in nodes {
        let (item_name, one_time, monthly, annual) = if let Some(ref item_id) = node.item_id {
            load_latest_price_info(db, item_id)?
        } else {
            (None, 0, 0, 0)
        };

        entities.push(EntityCost {
            entity_type: "node".to_string(),
            entity_name: node.name.clone(),
            item_name,
            item_id: node.item_id.clone(),
            one_time_cents: one_time,
            monthly_cents: monthly,
            annual_cents: annual,
        });
    }

    // Volumes
    for volume in volumes {
        let (item_name, one_time, monthly, annual) = if let Some(ref item_id) = volume.item_id {
            load_latest_price_info(db, item_id)?
        } else {
            (None, 0, 0, 0)
        };

        entities.push(EntityCost {
            entity_type: "volume".to_string(),
            entity_name: volume.name.clone(),
            item_name,
            item_id: volume.item_id.clone(),
            one_time_cents: one_time,
            monthly_cents: monthly,
            annual_cents: annual,
        });
    }

    Ok(entities)
}

/// Load latest price for an item and return (item_name, one_time_cents, monthly_cents, annual_cents).
fn load_latest_price_info(db: &Database, item_id: &str) -> Result<(Option<String>, i64, i64, i64)> {
    // Get item name
    let item_name: Option<String> = {
        let mut stmt = db
            .conn()
            .prepare("SELECT name FROM catalog_items WHERE id = ?1")?;
        stmt.query_row(params![item_id], |row| row.get(0)).ok()
    };

    // Get latest price by observed_at
    let price: Option<Price> = {
        let mut stmt = db.conn().prepare(
            "SELECT id, item_id, amount_cents, currency, source, condition, price_type, observed_at \
             FROM prices WHERE item_id = ?1 ORDER BY observed_at DESC LIMIT 1",
        )?;
        stmt.query_row(params![item_id], Price::from_row).ok()
    };

    match price {
        Some(p) => {
            let (mut one_time, mut monthly, mut annual) = (0i64, 0i64, 0i64);
            match p.price_type.as_str() {
                "one-time" => one_time = p.amount_cents,
                "monthly" => monthly = p.amount_cents,
                "annual" => annual = p.amount_cents,
                _ => one_time = p.amount_cents, // default to one-time
            }
            Ok((item_name, one_time, monthly, annual))
        }
        None => Ok((item_name, 0, 0, 0)),
    }
}

/// Parse a TCO period string like "3yr" or "5yr" into years.
fn parse_tco_period(period: &str) -> Result<u32> {
    let trimmed = period.trim().to_lowercase();
    if let Some(num_str) = trimmed.strip_suffix("yr") {
        match num_str.parse::<u32>() {
            Ok(n) if n > 0 => Ok(n),
            _ => bail!(
                "Invalid TCO period '{}'. Use format like '3yr' or '5yr'.",
                period
            ),
        }
    } else if let Some(num_str) = trimmed.strip_suffix("year") {
        match num_str.parse::<u32>() {
            Ok(n) if n > 0 => Ok(n),
            _ => bail!(
                "Invalid TCO period '{}'. Use format like '3yr' or '5yr'.",
                period
            ),
        }
    } else {
        bail!(
            "Invalid TCO period '{}'. Use format like '3yr' or '5yr'.",
            period
        )
    }
}

/// Print per-entity cost breakdown (default view).
fn print_cost_detail_text(report: &CostReport, tco_years: Option<u32>) {
    println!("Cost Analysis");
    println!();

    // One-time costs
    if report.one_time_total_cents > 0 {
        println!("One-Time Costs:");
        for e in &report.entities {
            if e.one_time_cents > 0 {
                let item_str = e.item_name.as_deref().unwrap_or("(no catalog item)");
                println!(
                    "  {:<20} {:<30} ${:.2}",
                    format!("[{}]", e.entity_type),
                    format!("{} ({})", e.entity_name, item_str),
                    e.one_time_cents as f64 / 100.0,
                );
            }
        }
        println!(
            "  {:<50} ${:.2}",
            style("Total one-time:").bold(),
            report.one_time_total_cents as f64 / 100.0,
        );
        println!();
    }

    // Recurring costs
    let has_recurring = report
        .entities
        .iter()
        .any(|e| e.monthly_cents > 0 || e.annual_cents > 0);

    if has_recurring {
        println!("Recurring Costs:");
        for e in &report.entities {
            if e.monthly_cents > 0 {
                let item_str = e.item_name.as_deref().unwrap_or("(no catalog item)");
                println!(
                    "  {:<20} {:<30} ${:.2}/mo",
                    format!("[{}]", e.entity_type),
                    format!("{} ({})", e.entity_name, item_str),
                    e.monthly_cents as f64 / 100.0,
                );
            }
            if e.annual_cents > 0 {
                let item_str = e.item_name.as_deref().unwrap_or("(no catalog item)");
                println!(
                    "  {:<20} {:<30} ${:.2}/yr",
                    format!("[{}]", e.entity_type),
                    format!("{} ({})", e.entity_name, item_str),
                    e.annual_cents as f64 / 100.0,
                );
            }
        }
        println!(
            "  {:<50} ${:.2}/mo",
            style("Total monthly:").bold(),
            report.monthly_total_cents as f64 / 100.0,
        );
        if report.annual_total_cents > 0 {
            println!(
                "  {:<50} ${:.2}/yr",
                style("Total annual:").bold(),
                report.annual_total_cents as f64 / 100.0,
            );
        }
        println!();
    }

    // Entities with no catalog item linked
    let unlinked: Vec<&EntityCost> = report
        .entities
        .iter()
        .filter(|e| e.item_id.is_none())
        .collect();
    if !unlinked.is_empty() {
        println!(
            "  {} {} entit{} with no catalog item linked",
            style("Note:").dim(),
            unlinked.len(),
            if unlinked.len() == 1 { "y" } else { "ies" }
        );
        for e in &unlinked {
            println!(
                "    {} {}",
                style(&e.entity_type).dim(),
                style(&e.entity_name).dim(),
            );
        }
        println!();
    }

    // Totals section
    println!("Totals:");
    println!(
        "  One-time:  ${:.2}",
        report.one_time_total_cents as f64 / 100.0
    );
    println!(
        "  Monthly:   ${:.2}/mo",
        report.monthly_total_cents as f64 / 100.0
    );
    println!(
        "  Annual:    ${:.2}/yr",
        report.annual_total_cents as f64 / 100.0
    );

    if let Some(years) = tco_years {
        let tco = compute_tco_cents(report, years);
        println!(
            "  {} ${:.2}",
            style(format!("TCO ({}yr):", years)).bold(),
            tco as f64 / 100.0
        );
    }
}

/// Print category summary (--summary flag).
fn print_cost_summary_text(report: &CostReport, tco_years: Option<u32>) {
    println!("Cost Summary");
    println!();

    // Group by entity_type
    let mut node_count = 0usize;
    let mut node_one_time = 0i64;
    let mut node_monthly = 0i64;
    let mut node_annual = 0i64;
    let mut vol_count = 0usize;
    let mut vol_one_time = 0i64;
    let mut vol_monthly = 0i64;
    let mut vol_annual = 0i64;

    for e in &report.entities {
        match e.entity_type.as_str() {
            "node" => {
                node_count += 1;
                node_one_time += e.one_time_cents;
                node_monthly += e.monthly_cents;
                node_annual += e.annual_cents;
            }
            "volume" => {
                vol_count += 1;
                vol_one_time += e.one_time_cents;
                vol_monthly += e.monthly_cents;
                vol_annual += e.annual_cents;
            }
            _ => {}
        }
    }

    println!(
        "  {:<14}{:<8}{:<14}{:<14}Annual",
        "Category", "Count", "One-Time", "Monthly"
    );
    println!(
        "  {:<14}{:<8}{:<14}{:<14}------",
        "--------", "-----", "--------", "-------"
    );
    println!(
        "  {:<14}{:<8}${:<13.2}${:<13.2}${:.2}",
        "Nodes",
        node_count,
        node_one_time as f64 / 100.0,
        node_monthly as f64 / 100.0,
        node_annual as f64 / 100.0,
    );
    println!(
        "  {:<14}{:<8}${:<13.2}${:<13.2}${:.2}",
        "Volumes",
        vol_count,
        vol_one_time as f64 / 100.0,
        vol_monthly as f64 / 100.0,
        vol_annual as f64 / 100.0,
    );
    println!(
        "  {:<14}{:<8}{:<14}{:<14}{}",
        style("Total").bold(),
        node_count + vol_count,
        style(format!(
            "${:.2}",
            report.one_time_total_cents as f64 / 100.0
        ))
        .bold(),
        style(format!("${:.2}", report.monthly_total_cents as f64 / 100.0)).bold(),
        style(format!("${:.2}", report.annual_total_cents as f64 / 100.0)).bold(),
    );

    if let Some(years) = tco_years {
        let tco = compute_tco_cents(report, years);
        println!();
        println!(
            "  {} ${:.2}",
            style(format!("Total Cost of Ownership ({}yr):", years)).bold(),
            tco as f64 / 100.0
        );
    }
}

// ---------------------------------------------------------------------------
// Constraints (ANLZ-02)
// ---------------------------------------------------------------------------

fn run_constraints(
    db: &mut Database,
    decision_name: &str,
    topology_override: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    let decision = resolve_decision(db, decision_name)?;

    // Load constraints
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

    // Resolve topology
    let topo = resolve_active_topology(db, topology_override)?;
    let nodes = load_nodes(db, &topo.id)?;

    let report = check_constraints(&constraints, &nodes);

    match format {
        OutputFormat::Text => {
            print_constraints_text(&report, &topo.name);
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "topology": topo.name,
                "topology_id": topo.id,
                "decision": decision.title,
                "decision_id": decision.id,
                "analysis": "constraints",
                "report": report,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

fn print_constraints_text(report: &ConstraintReport, topo_name: &str) {
    let checked = report.results.len();
    let score_str = format!("{:.0}%", report.score);
    let colored_score = if report.score >= 100.0 {
        style(score_str).green().bold()
    } else if report.score >= 75.0 {
        style(score_str).yellow().bold()
    } else {
        style(score_str).red().bold()
    };

    println!(
        "Constraints: {} ({} checked) -- {}",
        colored_score, checked, topo_name
    );
    println!();

    for result in &report.results {
        let status_styled = match result.status {
            ConstraintStatus::Pass => style(format!("[{}]", result.status)).green().bold(),
            ConstraintStatus::Warn => style(format!("[{}]", result.status)).yellow().bold(),
            ConstraintStatus::Fail => style(format!("[{}]", result.status)).red().bold(),
        };

        let (actual_str, limit_str, _unit) =
            format_constraint_display(&result.constraint_type, result.actual, result.limit);

        let margin_word = if result.margin >= 0.0 {
            "headroom"
        } else {
            "over"
        };
        let margin_abs = result.margin.abs();
        let margin_str = format_constraint_margin(&result.constraint_type, margin_abs);

        println!(
            "  {} {:<12} {} / {} max    ({} {}, {:.1}%)",
            status_styled,
            format!("{}:", result.constraint_type),
            actual_str,
            limit_str,
            margin_str,
            margin_word,
            result.margin_pct.abs(),
        );
    }
}

/// Format constraint actual and limit values for display.
fn format_constraint_display(
    constraint_type: &str,
    actual: f64,
    limit: f64,
) -> (String, String, &'static str) {
    // Normalize negative zero to positive zero
    let actual = if actual == 0.0 { 0.0 } else { actual };
    let limit = if limit == 0.0 { 0.0 } else { limit };
    match constraint_type {
        "budget" => (format!("${:.2}", actual), format!("${:.2}", limit), ""),
        "noise" => (
            format!("{:.1} dB", actual),
            format!("{:.1} dB", limit),
            "dB",
        ),
        "power" => (format!("{:.1} W", actual), format!("{:.1} W", limit), "W"),
        "rack_units" => (format!("{:.1} U", actual), format!("{:.1} U", limit), "U"),
        _ => (format!("{:.1}", actual), format!("{:.1}", limit), ""),
    }
}

/// Format constraint margin value for display.
fn format_constraint_margin(constraint_type: &str, margin_abs: f64) -> String {
    match constraint_type {
        "budget" => format!("${:.2}", margin_abs),
        "noise" => format!("{:.1} dB", margin_abs),
        "power" => format!("{:.1} W", margin_abs),
        "rack_units" => format!("{:.1} U", margin_abs),
        _ => format!("{:.1}", margin_abs),
    }
}

// ---------------------------------------------------------------------------
// Compare (ANLZ-08)
// ---------------------------------------------------------------------------

fn run_compare(
    db: &mut Database,
    name_a: &str,
    name_b: &str,
    include_diff: bool,
    decision_name: Option<&str>,
    warn_months: i32,
    format: OutputFormat,
) -> Result<()> {
    let topo_a = resolve_topology(db, name_a)?;
    let topo_b = resolve_topology(db, name_b)?;

    // Load data for topology A
    let nodes_a = load_nodes(db, &topo_a.id)?;
    let volumes_a = load_volumes(db, &topo_a.id)?;
    let datasets_a = load_datasets(db, &topo_a.id)?;
    let placements_a = load_placements_with_context(db, &topo_a.id)?;
    let sync_regimes_a = load_sync_regimes(db, &topo_a.id)?;

    // Load data for topology B
    let nodes_b = load_nodes(db, &topo_b.id)?;
    let volumes_b = load_volumes(db, &topo_b.id)?;
    let datasets_b = load_datasets(db, &topo_b.id)?;
    let placements_b = load_placements_with_context(db, &topo_b.id)?;
    let sync_regimes_b = load_sync_regimes(db, &topo_b.id)?;

    let metrics_a = compute_topology_metrics(
        &topo_a.name,
        &topo_a.id,
        &nodes_a,
        &volumes_a,
        &datasets_a,
        &placements_a,
        &sync_regimes_a,
        warn_months,
    );
    let metrics_b = compute_topology_metrics(
        &topo_b.name,
        &topo_b.id,
        &nodes_b,
        &volumes_b,
        &datasets_b,
        &placements_b,
        &sync_regimes_b,
        warn_months,
    );

    // Optional constraint checking within decision context
    let (constraints_a, constraints_b) = if let Some(dec_name) = decision_name {
        let decision = resolve_decision(db, dec_name)?;
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

        if constraints.is_empty() {
            (None, None)
        } else {
            (
                Some(check_constraints(&constraints, &nodes_a)),
                Some(check_constraints(&constraints, &nodes_b)),
            )
        }
    } else {
        (None, None)
    };

    let report = compare_topologies(&metrics_a, &metrics_b, constraints_a, constraints_b);

    match format {
        OutputFormat::Text => {
            print_compare_text(&report);

            if include_diff {
                println!();
                println!("--- Structural Diff ---");
                // Use the existing topology diff engine pattern
                print_simple_diff(db, &topo_a.id, &topo_a.name, &topo_b.id, &topo_b.name)?;
            }
        }
        OutputFormat::Json => {
            let mut json = serde_json::to_value(&report)?;
            if include_diff {
                // Include diff data in JSON
                let diff_data = build_diff_json(db, &topo_a.id, &topo_b.id)?;
                json["diff"] = diff_data;
            }
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

fn print_compare_text(report: &crate::domains::storage::analysis::ComparisonReport) {
    let name_a = &report.topology_a.name;
    let name_b = &report.topology_b.name;

    println!(
        "Comparison: {} vs {}",
        style(name_a).bold(),
        style(name_b).bold()
    );
    println!();

    // Determine column widths
    let metric_width = 18;
    let val_width = 14;

    #[allow(clippy::print_literal)]
    {
        println!(
            "  {:<width_m$}{:<width_v$}{:<width_v$}{}",
            "Metric",
            name_a,
            name_b,
            "Better",
            width_m = metric_width,
            width_v = val_width,
        );
        println!(
            "  {:<width_m$}{:<width_v$}{:<width_v$}{}",
            "------",
            "---------",
            "---------",
            "------",
            width_m = metric_width,
            width_v = val_width,
        );
    }

    for mc in &report.metrics_comparison {
        let (a_str, b_str) = format_metric_values(&mc.metric, mc.a, mc.b, &mc.unit);
        let better_str = match mc.better.as_str() {
            "a" => format!("<- {}", name_a),
            "b" => format!("<- {}", name_b),
            _ => "tie".to_string(),
        };

        let label = match mc.metric.as_str() {
            "total_cost" => "Total cost",
            "total_noise" => "Total noise",
            "total_power" => "Total power",
            "total_rack_units" => "Rack units",
            "total_capacity" => "Total capacity",
            "total_usable" => "Total usable",
            "redundancy" => "Redundancy",
            "capacity" => "Capacity score",
            "rpo" => "RPO score",
            "nodes" => "Nodes",
            "volumes" => "Volumes",
            "datasets" => "Datasets",
            other => other,
        };

        println!(
            "  {:<width_m$}{:<width_v$}{:<width_v$}{}",
            label,
            a_str,
            b_str,
            better_str,
            width_m = metric_width,
            width_v = val_width,
        );
    }

    // Print constraint results if present
    if let Some(ref ca) = report.constraints_a {
        println!();
        println!("Constraints for {}:", style(&report.topology_a.name).bold());
        print_constraints_text(ca, &report.topology_a.name);
    }
    if let Some(ref cb) = report.constraints_b {
        println!();
        println!("Constraints for {}:", style(&report.topology_b.name).bold());
        print_constraints_text(cb, &report.topology_b.name);
    }
}

fn format_metric_values(metric: &str, a: f64, b: f64, unit: &str) -> (String, String) {
    match metric {
        "total_cost" => {
            // Normalize negative zero to positive zero
            let a = if a == 0.0 { 0.0 } else { a };
            let b = if b == 0.0 { 0.0 } else { b };
            (format!("${:.2}", a), format!("${:.2}", b))
        }
        "total_noise" => {
            let a = if a == 0.0 { 0.0 } else { a };
            let b = if b == 0.0 { 0.0 } else { b };
            (format!("{:.1} dB", a), format!("{:.1} dB", b))
        }
        "total_power" => {
            let a = if a == 0.0 { 0.0 } else { a };
            let b = if b == 0.0 { 0.0 } else { b };
            (format!("{:.1} W", a), format!("{:.1} W", b))
        }
        "total_rack_units" => {
            let a = if a == 0.0 { 0.0 } else { a };
            let b = if b == 0.0 { 0.0 } else { b };
            (format!("{:.1} U", a), format!("{:.1} U", b))
        }
        "total_capacity" | "total_usable" => {
            let a_cap = Capacity::from_bytes(a as u64);
            let b_cap = Capacity::from_bytes(b as u64);
            (format!("{}", a_cap), format!("{}", b_cap))
        }
        "redundancy" | "capacity" | "rpo" => (format!("{:.1}%", a), format!("{:.1}%", b)),
        _ => {
            let a = if a == 0.0 { 0.0 } else { a };
            let b = if b == 0.0 { 0.0 } else { b };
            if unit.is_empty() {
                (format!("{}", a as i64), format!("{}", b as i64))
            } else {
                (format!("{:.1} {}", a, unit), format!("{:.1} {}", b, unit))
            }
        }
    }
}

/// Print a simple structural diff between two topologies.
fn print_simple_diff(
    db: &mut Database,
    topo_a_id: &str,
    topo_a_name: &str,
    topo_b_id: &str,
    topo_b_name: &str,
) -> Result<()> {
    // Node comparison
    let nodes_a = load_nodes(db, topo_a_id)?;
    let nodes_b = load_nodes(db, topo_b_id)?;

    let names_a: std::collections::HashSet<String> =
        nodes_a.iter().map(|n| n.name.clone()).collect();
    let names_b: std::collections::HashSet<String> =
        nodes_b.iter().map(|n| n.name.clone()).collect();

    let only_a: Vec<&String> = names_a.difference(&names_b).collect();
    let only_b: Vec<&String> = names_b.difference(&names_a).collect();

    if !only_a.is_empty() || !only_b.is_empty() {
        println!("Nodes:");
        for name in &only_a {
            println!(
                "  {} only in {}",
                style(format!("- {}", name)).red(),
                topo_a_name
            );
        }
        for name in &only_b {
            println!(
                "  {} only in {}",
                style(format!("+ {}", name)).green(),
                topo_b_name
            );
        }
    }

    // Volume comparison
    let volumes_a = load_volumes(db, topo_a_id)?;
    let volumes_b = load_volumes(db, topo_b_id)?;

    let vol_names_a: std::collections::HashSet<String> =
        volumes_a.iter().map(|v| v.name.clone()).collect();
    let vol_names_b: std::collections::HashSet<String> =
        volumes_b.iter().map(|v| v.name.clone()).collect();

    let vol_only_a: Vec<&String> = vol_names_a.difference(&vol_names_b).collect();
    let vol_only_b: Vec<&String> = vol_names_b.difference(&vol_names_a).collect();

    if !vol_only_a.is_empty() || !vol_only_b.is_empty() {
        println!("Volumes:");
        for name in &vol_only_a {
            println!(
                "  {} only in {}",
                style(format!("- {}", name)).red(),
                topo_a_name
            );
        }
        for name in &vol_only_b {
            println!(
                "  {} only in {}",
                style(format!("+ {}", name)).green(),
                topo_b_name
            );
        }
    }

    if only_a.is_empty() && only_b.is_empty() && vol_only_a.is_empty() && vol_only_b.is_empty() {
        println!("  No structural differences in nodes or volumes.");
    }

    Ok(())
}

/// Build diff data as JSON for the compare --diff flag.
fn build_diff_json(
    db: &mut Database,
    topo_a_id: &str,
    topo_b_id: &str,
) -> Result<serde_json::Value> {
    let nodes_a = load_nodes(db, topo_a_id)?;
    let nodes_b = load_nodes(db, topo_b_id)?;

    let names_a: std::collections::HashSet<String> =
        nodes_a.iter().map(|n| n.name.clone()).collect();
    let names_b: std::collections::HashSet<String> =
        nodes_b.iter().map(|n| n.name.clone()).collect();

    let only_a: Vec<String> = names_a.difference(&names_b).cloned().collect();
    let only_b: Vec<String> = names_b.difference(&names_a).cloned().collect();
    let common: Vec<String> = names_a.intersection(&names_b).cloned().collect();

    let volumes_a = load_volumes(db, topo_a_id)?;
    let volumes_b = load_volumes(db, topo_b_id)?;

    let vol_names_a: std::collections::HashSet<String> =
        volumes_a.iter().map(|v| v.name.clone()).collect();
    let vol_names_b: std::collections::HashSet<String> =
        volumes_b.iter().map(|v| v.name.clone()).collect();

    let vol_only_a: Vec<String> = vol_names_a.difference(&vol_names_b).cloned().collect();
    let vol_only_b: Vec<String> = vol_names_b.difference(&vol_names_a).cloned().collect();

    Ok(serde_json::json!({
        "nodes": {
            "only_a": only_a,
            "only_b": only_b,
            "common": common,
        },
        "volumes": {
            "only_a": vol_only_a,
            "only_b": vol_only_b,
        }
    }))
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

/// Load all nodes for a topology.
fn load_nodes(db: &Database, topology_id: &str) -> Result<Vec<Node>> {
    let results = {
        let mut stmt = db.conn().prepare(
            "SELECT id, topology_id, name, role, location, available_bays, \
             interface_types, power_draw_watts, cost_estimate, noise_db, rack_units, item_id, created_at, updated_at \
             FROM nodes WHERE topology_id = ?1 ORDER BY name",
        )?;
        let rows = stmt.query_map(params![topology_id], Node::from_row)?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    Ok(results)
}

/// Load all links for a topology.
fn load_links(db: &Database, topology_id: &str) -> Result<Vec<Link>> {
    let results = {
        let mut stmt = db.conn().prepare(
            "SELECT id, topology_id, source_node_id, target_node_id, bandwidth_bytes_sec, \
             connection_type, latency_ms, is_metered, cost_per_gb_cents, created_at, updated_at \
             FROM links WHERE topology_id = ?1",
        )?;
        let rows = stmt.query_map(params![topology_id], Link::from_row)?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    Ok(results)
}

/// Load all sync regimes for a topology with context (via the JOINed loader).
fn load_sync_regimes(db: &Database, topology_id: &str) -> Result<Vec<SyncRegimeWithContext>> {
    load_sync_regimes_with_context(db, topology_id)
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

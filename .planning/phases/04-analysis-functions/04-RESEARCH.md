# Phase 4: Analysis Functions - Research

**Researched:** 2026-02-07
**Domain:** CLI analysis commands over existing topology data model (Rust/clap/rusqlite)
**Confidence:** HIGH

## Summary

Phase 4 adds read-only analysis commands to the existing `sp` CLI. Unlike Phases 1-3 which built entities and mutations, this phase queries existing data and produces computed output -- no schema changes, no event logging, no undo/redo. The core challenge is designing clean analysis logic that traverses the entity graph (topology -> nodes -> volumes -> placements -> datasets + sync_regimes) and produces scored reports.

The codebase already provides everything needed: entity models with `from_row` deserialization, `resolve_active_topology` for topology targeting, `OutputFormat` enum for text/JSON branching, and the `console` crate for styled terminal output. The only new dependency consideration is cron expression parsing for RPO analysis, where the `croner` crate (v3.0.1) provides exactly the needed `find_next_occurrence` API with chrono integration already in the dependency tree.

**Primary recommendation:** Implement analysis as pure functions in `src/domains/storage/` that take loaded entity data and return analysis result structs, with thin CLI wrappers in `src/cli/analyze.rs` handling argument parsing and output formatting. This separates testable logic from I/O.

<user_constraints>

## User Constraints (from CONTEXT.md)

### Locked Decisions
- Scored summary per analysis type (separate scores: Redundancy: X%, RPO: Y%, Capacity: Z%)
- Default output shows score + problems only; --verbose flag shows full per-dataset/volume breakdown
- Datasets/volumes without issues are hidden in default mode, shown in verbose
- 'sp analyze' with no subcommand runs ALL analyses and gives combined report (score per type)
- Individual analyses available as subcommands (e.g., sp analyze redundancy, sp analyze rpo, etc.)
- Failure simulation: required argument specifying which node(s) to simulate failing
- Multi-node failure supported: accept multiple node names
- Failure report shows BOTH volume impact AND dataset impact
- Capacity projection headline metric: months-until-full per volume
- --verbose adds a timeline table showing projected usage at intervals (3, 6, 12 months)
- Datasets without growth_rate set are skipped and noted (no guessing)
- Ceiling uses usable_bytes if set, falls back to capacity_bytes
- Warning threshold: default 12 months, configurable via --warn-months=N flag
- Volumes approaching threshold are highlighted

### Claude's Discretion
- Exact verb for analysis commands (analyze vs check) based on CLI conventions
- Fix suggestion inclusion per analysis type
- JSON output structure and score inclusion
- Exit code behavior
- Color/symbol approach for terminal output
- Whether --compare belongs in Phase 4 or Phase 5
- Severity tier design for failure simulation
- Timeline table intervals for capacity projection
- All-clear output verbosity

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope

</user_constraints>

## Standard Stack

### Core (already in Cargo.toml)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| clap | 4 (derive) | CLI argument parsing, subcommands | Already used for all commands |
| rusqlite | 0.31 (bundled) | Database queries for analysis input | Already used for all data access |
| serde / serde_json | 1 | JSON output formatting | Already used for all JSON output |
| console | 0.15 | Terminal colors and styling | Already used in diff output |
| chrono | 0.4 | DateTime handling for RPO calculations | Already used for all timestamps |
| anyhow | 1 | Error handling | Already used throughout |

### New Dependency
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| croner | 3.0.1 | Cron expression parsing for RPO analysis | Converting schedule strings to interval hours |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| croner | cron (0.12) | cron is older, less maintained; croner is actively maintained, simpler API |
| croner | Manual parsing | Schedule field is freeform cron -- hand-rolling is error-prone |
| croner | Skip cron parsing | Could just check if schedule exists (boolean) but loses actual RPO gap calculation |

**Installation:**
```bash
cargo add croner@3.0.1
```

## Architecture Patterns

### Recommended Module Structure
```
src/
├── cli/
│   ├── mod.rs             # Add Analyze variant to Commands enum
│   └── analyze.rs         # NEW: CLI layer -- args, output formatting
├── domains/
│   └── storage/
│       ├── mod.rs          # Add analysis module
│       ├── models.rs       # Existing (empty, placeholder for Phase 4)
│       └── analysis.rs     # NEW: Pure analysis functions + result types
```

### Pattern 1: Separation of Analysis Logic from CLI
**What:** Analysis functions live in `domains/storage/analysis.rs`, taking loaded data as input and returning result structs. CLI module handles argument parsing, data loading, and output formatting.
**When to use:** Always -- this is the core architectural pattern for this phase.
**Why:** Enables unit testing analysis logic without needing database fixtures for every test. Analysis functions can be tested with constructed entity structs directly.

```rust
// src/domains/storage/analysis.rs
pub struct RedundancyReport {
    pub score: f64,  // 0.0 to 100.0
    pub issues: Vec<RedundancyIssue>,
    pub dataset_count: usize,
    pub ok_count: usize,
}

pub struct RedundancyIssue {
    pub dataset_name: String,
    pub criticality: String,
    pub required_copies: i32,
    pub actual_copies: i32,
    pub required_locations: i32,
    pub actual_locations: i32,
    pub problems: Vec<String>,
}

pub fn analyze_redundancy(
    datasets: &[Dataset],
    placements: &[PlacementWithContext],  // placement + volume + node info
) -> RedundancyReport {
    // Pure function: no DB access, no I/O
}
```

```rust
// src/cli/analyze.rs -- thin wrapper
fn redundancy(db: &mut Database, topology_override: Option<&str>, ...) -> Result<()> {
    let topo = resolve_active_topology(db, topology_override)?;
    let datasets = load_datasets(db, &topo.id)?;
    let placements = load_placements_with_context(db, &topo.id)?;

    let report = analysis::analyze_redundancy(&datasets, &placements);

    // Format and print based on OutputFormat + verbose flag
}
```

### Pattern 2: Enriched Query Types for Analysis
**What:** Define intermediate structs that join across tables for analysis input, rather than passing raw entity vectors with separate lookups.
**When to use:** When analysis needs data from multiple joined tables (placement + volume + node).

```rust
// Enriched placement data for analysis functions
pub struct PlacementWithContext {
    pub placement_id: String,
    pub dataset_id: String,
    pub dataset_name: String,
    pub volume_id: String,
    pub volume_name: String,
    pub node_id: String,
    pub node_name: String,
    pub node_location: String,
    pub role: String,
    pub capacity_bytes: i64,
    pub usable_bytes: Option<i64>,
}
```

This pattern is already established in the codebase -- see `placement.rs` list command which uses a custom `PlacementRow` struct, and `sync_regime.rs` which uses `SyncRow`.

### Pattern 3: Command Structure with Shared Flags
**What:** The `Analyze` command uses subcommands for individual analyses plus a default "all" behavior.
**When to use:** This is the locked decision from CONTEXT.md.

```rust
#[derive(Subcommand)]
pub enum AnalyzeCommands {
    /// Analyze redundancy coverage for all datasets
    Redundancy { /* shared flags */ },
    /// Simulate node failure and show impact
    Failure {
        /// Node(s) to simulate failing
        #[arg(required = true)]
        nodes: Vec<String>,
        /* shared flags */
    },
    /// Check RPO compliance against sync schedules
    Rpo { /* shared flags */ },
    /// Project capacity usage and time-to-full
    Capacity { /* shared flags */ },
}
```

For the "run all" behavior when `sp analyze` is invoked with no subcommand, use clap's `#[command(subcommand_required = false)]` pattern. When no subcommand is provided, run all four analyses and produce combined output.

### Pattern 4: Score Calculation
**What:** Each analysis type produces a 0-100 score independently.
**When to use:** All analysis types.

Score formula recommendation:
- **Redundancy:** `(datasets_meeting_all_requirements / total_datasets) * 100`
- **RPO:** `(datasets_with_rpo_set_and_met / datasets_with_rpo_set) * 100` (skip datasets without max_rpo)
- **Capacity:** `(volumes_above_threshold / total_volumes_with_growth_data) * 100` (inverted: 100% = all good)

### Anti-Patterns to Avoid
- **Querying inside loops:** Load all needed data upfront with JOINs, not one query per dataset/volume.
- **Mixed logic and formatting:** Don't format strings inside analysis functions. Return data, format in CLI layer.
- **Hardcoded thresholds without configuration:** The capacity warning threshold is configurable via `--warn-months`. Follow this pattern.
- **Mutating the database:** Analysis commands are read-only. No events, no transactions, no undo/redo.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Cron interval calculation | Regex-based cron parser | `croner` crate | Cron syntax has edge cases (day-of-week, month names, L/W modifiers) that are deceptively complex |
| Capacity formatting | Custom byte formatting | Existing `Capacity::from_bytes().to_string()` | Already in `src/core/specs.rs`, handles TB/GB/MB display |
| Topology resolution | Ad-hoc SQL for finding current topology | `resolve_active_topology()` | Already handles --topology override vs current tag |
| Entity resolution | Custom lookup code | `resolve_node()`, `resolve_dataset()` | Already handles name-or-UUID, disambiguation |

**Key insight:** This phase has zero new infrastructure to build. All entity access, formatting, and resolution patterns exist. The work is purely analysis logic + output formatting.

## Common Pitfalls

### Pitfall 1: Division by Zero in Score Calculations
**What goes wrong:** Score calculation divides by total count, which may be zero (no datasets, no volumes with growth data, no datasets with RPO set).
**Why it happens:** Empty topologies or topologies where optional fields aren't set.
**How to avoid:** Always check denominator. Return 100% score (all clear) when there's nothing to analyze, with a note.
**Warning signs:** Panic on `NaN` or `Infinity` in score output.

### Pitfall 2: RPO Analysis with No Schedule Set
**What goes wrong:** SyncRegime.schedule is `Option<String>`. If no schedule is set, we can't calculate sync interval.
**Why it happens:** Users may define sync regimes without schedules (manual syncs).
**How to avoid:** Treat sync regimes without schedules as "unknown RPO" and flag them as issues when the dataset has a max_rpo_hours set. Report "no scheduled sync" rather than guessing.
**Warning signs:** Silently marking RPO as compliant when no schedule exists.

### Pitfall 3: Capacity Analysis Ignoring usable_bytes
**What goes wrong:** Using `capacity_bytes` for the ceiling when `usable_bytes` is set.
**Why it happens:** Forgetting the locked decision that usable_bytes takes precedence.
**How to avoid:** `let ceiling = volume.usable_bytes.unwrap_or(volume.capacity_bytes);` -- this is the locked decision from CONTEXT.md.
**Warning signs:** Capacity projections are optimistic because they use raw capacity instead of usable space.

### Pitfall 4: Failure Simulation Missing Transitive Impact
**What goes wrong:** Reporting only direct volume loss, missing that a dataset's placement on the failed node means the dataset loses a copy.
**Why it happens:** Only looking at volumes on the node, not following volume -> placement -> dataset chain.
**How to avoid:** The locked decision requires BOTH volume impact AND dataset impact. Query placements that reference volumes on the failed node, then aggregate by dataset.
**Warning signs:** Failure sim shows "2 volumes lost" but doesn't mention which datasets are affected.

### Pitfall 5: Location Counting for min_locations
**What goes wrong:** Counting nodes instead of distinct locations. Two nodes in the same location count as one location.
**Why it happens:** Confusing node count with location count.
**How to avoid:** Use `node.location` field and count distinct values. Note: empty string location should be treated as a unique unknown location (or flagged).
**Warning signs:** Redundancy analysis says "2 locations" when both nodes have location="office".

### Pitfall 6: Borrow Checker with Prepared Statements
**What goes wrong:** Trying to use `db.conn()` to prepare a statement while another prepared statement is still borrowing `conn`.
**Why it happens:** Multiple queries in the same scope.
**How to avoid:** Use the D023 pattern (block-scoped prepared statements) already established in this codebase. Load all data into owned Vecs before processing.
**Warning signs:** Compiler error "cannot borrow `*db` as immutable because it is also borrowed as mutable".

## Code Examples

### Loading Enriched Placement Data
```rust
// Pattern matching existing codebase JOINs (see placement.rs list, sync_regime.rs list)
fn load_placements_with_context(
    db: &Database,
    topology_id: &str,
) -> Result<Vec<PlacementWithContext>> {
    let mut stmt = db.conn().prepare(
        "SELECT p.id AS placement_id, p.dataset_id, p.volume_id, p.role, p.priority,
                d.name AS dataset_name, d.min_copies, d.min_locations, d.max_rpo_hours,
                d.criticality, d.size_bytes, d.growth_rate_bytes_month,
                v.name AS volume_name, v.capacity_bytes, v.usable_bytes,
                n.id AS node_id, n.name AS node_name, n.location AS node_location
         FROM placements p
         JOIN datasets d ON p.dataset_id = d.id
         JOIN volumes v ON p.volume_id = v.id
         JOIN nodes n ON v.node_id = n.id
         WHERE p.topology_id = ?1
         ORDER BY d.name, n.name",
    )?;
    // ... map rows to PlacementWithContext
}
```

### Cron Interval Calculation for RPO
```rust
use croner::Cron;
use chrono::Utc;
use std::str::FromStr;

/// Calculate the maximum gap in hours between two successive cron occurrences.
/// Returns None if the cron expression is invalid or has no next occurrence.
fn cron_interval_hours(schedule: &str) -> Option<f64> {
    let cron = Cron::from_str(schedule).ok()?;
    let now = Utc::now();
    let first = cron.find_next_occurrence(&now, false).ok()?;
    let second = cron.find_next_occurrence(&first, false).ok()?;
    let gap = second - first;
    Some(gap.num_minutes() as f64 / 60.0)
}
```

### Score + Problems Output Pattern (Text Mode)
```rust
use console::style;

fn print_analysis_header(name: &str, score: f64, issue_count: usize) {
    let score_color = if score >= 100.0 {
        style(format!("{:.0}%", score)).green()
    } else if score >= 75.0 {
        style(format!("{:.0}%", score)).yellow()
    } else {
        style(format!("{:.0}%", score)).red()
    };

    let status = if issue_count == 0 {
        style("OK").green().to_string()
    } else {
        format!("{} issue{}", issue_count, if issue_count == 1 { "" } else { "s" })
    };

    println!("{}: {} ({})", name, score_color, status);
}
```

### Default "All Analyses" Combined Report
```rust
// sp analyze (no subcommand) -- dashboard view
fn run_all(db: &mut Database, verbose: bool, ...) -> Result<()> {
    let topo = resolve_active_topology(db, topology_override)?;
    println!("Analysis: {} [{}]", topo.name, topo.tag.as_deref().unwrap_or("untagged"));
    println!();

    let redundancy = analyze_redundancy(&datasets, &placements);
    print_redundancy_report(&redundancy, verbose, format);

    let rpo = analyze_rpo(&datasets, &placements, &sync_regimes);
    print_rpo_report(&rpo, verbose, format);

    let capacity = analyze_capacity(&datasets, &volumes, &placements, warn_months);
    print_capacity_report(&capacity, verbose, format);

    // Failure sim is NOT included in "all" -- it requires explicit node argument
}
```

## Discretion Recommendations

### Command Verb: `analyze` (not `check`)
**Recommendation:** Use `sp analyze` as the top-level command.
**Rationale:** "analyze" implies deeper evaluation with scoring, while "check" implies pass/fail. The scored-report design is more analytical. Also, `check` conflicts with the `just check` build command.

### Fix Suggestions: Include for redundancy and RPO, skip for capacity
**Recommendation:**
- Redundancy issues: suggest "add a placement on a volume in a different location"
- RPO issues: suggest "add a sync regime with schedule meeting Xh RPO requirement"
- Capacity: the warning itself is the actionable information (months-until-full)
- Failure: no fix suggestions -- this is exploratory "what if" analysis

### JSON Output Structure: Include scores and computed fields
**Recommendation:** JSON output should include both raw data and computed scores.
```json
{
  "topology": "my-setup",
  "redundancy": {
    "score": 66.7,
    "datasets_analyzed": 3,
    "datasets_ok": 2,
    "issues": [{ "dataset": "photos", "required_copies": 3, "actual_copies": 1 }]
  }
}
```

### Exit Codes: Non-zero on issues found
**Recommendation:** Exit code 0 when all analyses pass (100% scores). Exit code 1 when any analysis finds issues. This enables CI/scripting use: `sp analyze && echo "all clear"`.
**Note:** Failure sim always exits 0 since it's exploratory, not pass/fail.

### Color/Symbol Approach
**Recommendation:** Use the `console` crate's `style()` function (already in use for diff):
- Green for OK/passing items
- Yellow for warnings (capacity approaching threshold)
- Red for issues/failures
- No emoji -- the codebase doesn't use emoji anywhere; use text indicators like `[OK]`, `[WARN]`, `[FAIL]`

### --compare Flag: Defer to Phase 5
**Recommendation:** `--compare` (side-by-side analysis of two topologies) naturally belongs in Phase 5 where topology comparison features are planned. Phase 4 should focus on single-topology analysis.

### Severity Tiers for Failure Simulation: Three tiers
**Recommendation:**
- **LOST:** Volume/dataset has zero remaining copies after failure
- **DEGRADED:** Dataset loses copies but still has at least one remaining
- **AT RISK:** Dataset still meets min_copies but no longer meets min_locations

### Timeline Table Intervals: 3, 6, 12 months (as specified)
**Recommendation:** Use the intervals specified in CONTEXT.md. Format as a simple text table:
```
  Volume     Now        3mo        6mo        12mo       Full
  pool-1     2.1/4.0TB  2.4/4.0TB  2.7/4.0TB  3.3/4.0TB  18 months
```

### All-Clear Output: Concise single line per analysis
**Recommendation:** When no issues:
```
Redundancy: 100% (3 datasets, all requirements met)
RPO: 100% (2 datasets with RPO, all compliant)
Capacity: 100% (4 volumes, none approaching threshold)
```

### Default to Current-Tagged Topology: Yes
**Recommendation:** Use `resolve_active_topology(db, topology_override)` consistently with `--topology` flag, matching all existing commands. This is the established pattern.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `is_active` boolean | `tag` column (current/exploring/archived/NULL) | Phase 3 (D019) | Analysis must use `resolve_active_topology` which checks `tag='current'` |
| Global entity names | Topology-scoped names | Phase 2 (D017) | Analysis queries must scope to topology_id |
| None | Block-scoped prepared stmts | Phase 3 (D023) | Must scope DB borrows carefully |

**Deprecated/outdated:**
- `set-active` command is deprecated (use `tag` instead) but analysis doesn't use it.
- `domains/storage/models.rs` is currently empty with "Phase 4" placeholder.

## Open Questions

1. **How to handle datasets with no placements at all?**
   - What we know: A dataset can exist without any placements (added but not yet placed)
   - What's unclear: Should this be scored as 0 copies (failing redundancy) or flagged as "unplaced, skipped"?
   - Recommendation: Score as 0 copies -- it IS a redundancy issue. Flag prominently: "dataset 'X' has no placements"

2. **Should `sp analyze` (no subcommand) include failure sim?**
   - What we know: Failure sim requires explicit node arguments, which aren't present in the "all" invocation
   - What's unclear: Should we run a "what if every node fails individually" simulation?
   - Recommendation: Exclude failure sim from "all" run. It's exploratory and requires user input. The dashboard should show redundancy + RPO + capacity only.

3. **What about datasets placed on volumes with no growth_rate for capacity analysis?**
   - What we know: Only volumes that host datasets with growth_rate_bytes_month are relevant for capacity projection
   - What's unclear: Should we project capacity for volumes that host SOME datasets with growth rates and SOME without?
   - Recommendation: Sum only the growth rates that are set. Note which datasets on the volume lack growth data. This gives a lower-bound projection.

## Sources

### Primary (HIGH confidence)
- Codebase inspection: `src/core/models.rs` -- complete entity model with all fields used for analysis
- Codebase inspection: `src/cli/topology.rs`, `dataset.rs`, `placement.rs`, `sync_regime.rs` -- established CLI patterns
- Codebase inspection: `src/core/resolve.rs` -- topology/entity resolution patterns
- Codebase inspection: `src/core/specs.rs` -- Capacity formatting utility
- Codebase inspection: `Cargo.toml` -- current dependency versions

### Secondary (MEDIUM confidence)
- [croner crate docs](https://docs.rs/croner/latest/croner/) - v3.0.1 API for cron parsing
- [croner GitHub](https://github.com/Hexagon/croner-rust) - Usage examples and version info
- [Rust CLI exit codes](https://rust-cli.github.io/book/in-depth/exit-code.html) - Exit code conventions

### Tertiary (LOW confidence)
- None -- all findings verified against codebase or official docs

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- no new dependencies except croner, everything else is already in Cargo.toml
- Architecture: HIGH -- patterns directly observed in existing codebase, extending established conventions
- Pitfalls: HIGH -- identified from actual field relationships and data model inspection
- Analysis logic: HIGH -- straightforward graph traversal over well-understood entity model
- croner API: MEDIUM -- verified via docs.rs and GitHub README but not hands-on tested

**Research date:** 2026-02-07
**Valid until:** 2026-03-07 (stable -- no external API dependencies, internal codebase patterns)

# Phase 5: Decision Integration - Research

**Researched:** 2026-02-07
**Domain:** Decision lifecycle management, constraint checking, topology comparison (Rust/clap/rusqlite)
**Confidence:** HIGH

## Summary

Phase 5 introduces a new top-level entity domain -- decisions -- with lifecycle management, constraint checking against topologies, and side-by-side topology comparison. This is the first phase that adds entirely new database tables (decisions, decision_constraints, decision_topologies) plus new columns on the existing nodes table (cost_estimate, noise_db, power_watts, rack_units). The schema migration will be version 3.

The codebase patterns are extremely well-established across 4 prior phases. Every CLI command follows the same flow: clap derive for arg parsing, entity resolver for name/ID lookup, transaction-wrapped mutations with event recording, OutputFormat branching for text/JSON output. The constraint checking system should follow the exact same pass/warn/fail pattern already used by the analysis engine (redundancy, RPO, capacity, failure sim). The topology comparison command extends the existing diff infrastructure already built in Phase 3.

The main engineering challenge is the junction table pattern for decision-topology relationships (decision_topologies) and ensuring the decision lifecycle state machine is enforced at the database level. The constraint system is straightforward -- typed constraints on decisions, node-level numeric fields summed per topology, and pass/warn/fail evaluation.

**Primary recommendation:** Implement decisions as a new entity type in core/models.rs with its own CLI module (cli/decision.rs), reuse the analysis pattern for constraint checking (pure functions in domains/storage/analysis.rs), and extend the existing diff/comparison infrastructure for topology comparison. Schema migration v3 adds all new tables and node columns in one migration.

<user_constraints>

## User Constraints (from CONTEXT.md)

### Locked Decisions
- Four-state lifecycle: draft -> open -> decided -> abandoned
- Draft state for decisions still being set up (adding constraints, considering topologies) before formally opening
- Decided = chose a topology; Abandoned = gave up / became moot
- DEC-11 reopen moves back to "open" status
- Supported constraint types: budget (max $), noise (max dB), power (max watts), rack units (max U)
- These are typed constraints, not arbitrary key-value pairs
- Constraints are attached to decisions and checked against considered topologies
- Add cost_estimate, noise_db, power_watts, rack_units fields to nodes (schema migration)
- Sum across topology nodes for totals when checking constraints
- Phase 6 will enrich these via catalog links, but Phase 5 uses direct node-level values
- User manually sets these values per node for now
- Pass/warn/fail with margin for each constraint
- PASS = within limit, WARN = within 10% of limit, FAIL = over limit
- Show actual value vs limit and how much headroom or overage
- Default comparison shows analysis-only comparison (metrics side-by-side)
- --diff flag adds structural changes (builds on existing diff command)
- Comparison works on any two topologies -- not scoped to a decision
- When run within a decision context, constraints are included in the comparison
- Closing a decision with a chosen topology records the choice but does NOT auto-tag the topology as "current"
- User may not want to switch current topology immediately after deciding

### Claude's Discretion
- Decision hierarchy: Flat with optional parent_id, no enforced tree behavior -- just a foreign key for optional grouping
- Reopen behavior: Pick the simpler approach for handling the previously chosen topology reference on reopen
- Decision show command: Follow existing patterns in the codebase (e.g., how topology show works with inline details)
- Comparison indicators: Per-metric advantage indicators or neutral data -- pick what works best for CLI output
- Comparison JSON format: Pick the format that best serves AI agent consumption
- Rationale capture: Requiring rationale for decided (but optional for abandoned) makes sense
- Abandon reasons: Pick the simpler option (likely freeform only)
- Decision snapshot: Snapshotting comparison data at close time preserves the historical record. Claude can decide whether the complexity is worth it.

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope

</user_constraints>

## Standard Stack

### Core (already in Cargo.toml -- no new dependencies needed)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| clap | 4 (derive) | CLI argument parsing, subcommands | Already used for all commands |
| rusqlite | 0.31 (bundled) | Database tables, queries, transactions | Already used for all data access |
| serde / serde_json | 1 | JSON output, before/after state serialization | Already used for all entities |
| console | 0.15 | Terminal colors for pass/warn/fail output | Already used in analysis + diff output |
| chrono | 0.4 | DateTime handling for timestamps | Already used for all entities |
| anyhow | 1 | Error handling | Already used throughout |
| uuid | 1 (v4, serde) | ID generation for new entities | Already used for all entities |

### New Dependencies
None required. All needed functionality is covered by existing dependencies.

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Typed constraint enum | Arbitrary key-value pairs | User decided: typed constraints only (budget, noise, power, rack_units) |
| Node-level fields | Separate metadata table | User decided: add fields directly to nodes table for simplicity |
| Snapshot at close | No snapshot | Claude's discretion -- recommend YES, JSON blob is cheap and preserves historical context |

## Architecture Patterns

### Recommended Module Structure
```
src/
├── cli/
│   ├── mod.rs             # Add Decision variant to Commands enum, add Compare to Analyze
│   ├── decision.rs        # NEW: Decision lifecycle CLI commands
│   └── analyze.rs         # EXTEND: Add constraint check + compare subcommands
├── core/
│   ├── db.rs              # EXTEND: Migration v3 (new tables + node columns)
│   ├── models.rs          # EXTEND: Decision, DecisionConstraint, DecisionTopology structs
│   ├── resolve.rs         # EXTEND: resolve_decision() function
│   └── events.rs          # EXTEND: entity_table_name + restore for new entity types
├── domains/
│   └── storage/
│       └── analysis.rs    # EXTEND: constraint checking + comparison functions
```

### Pattern 1: Decision Entity Model
**What:** Three new model structs: Decision, DecisionConstraint, DecisionTopology. Follow the exact same pattern as Topology/Node/Volume: new(), insert(), from_row(), to_json().
**When to use:** For all decision-related data.

```rust
// Decision struct follows the established model pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: String,           // draft, open, decided, abandoned
    pub parent_id: Option<String>,
    pub chosen_topology_id: Option<String>,
    pub rationale: Option<String>,
    pub snapshot: Option<String>,  // JSON blob of comparison at close time
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
}

// DecisionConstraint -- typed constraints on a decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionConstraint {
    pub id: String,
    pub decision_id: String,
    pub constraint_type: String,  // budget, noise, power, rack_units
    pub max_value: f64,           // max dollars, dB, watts, or U
    pub created_at: DateTime<Utc>,
}

// DecisionTopology -- junction table linking decisions to considered topologies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionTopology {
    pub id: String,
    pub decision_id: String,
    pub topology_id: String,
    pub added_at: DateTime<Utc>,
}
```

### Pattern 2: Decision Lifecycle State Machine
**What:** Enforce valid state transitions at the application layer.
**When to use:** For all status changes (open, decide, abandon, reopen).

```
Valid transitions:
  draft -> open        (DEC-04 update or implicit on first constraint/topology add)
  draft -> decided     (ERROR: must be open first)
  draft -> abandoned   (allowed: user gave up before opening)
  open  -> decided     (DEC-09: close with chosen topology)
  open  -> abandoned   (DEC-10: close without choice)
  decided  -> open     (DEC-11: reopen)
  abandoned -> open    (DEC-11: reopen)
```

Reopen behavior (Claude's discretion recommendation): On reopen, clear chosen_topology_id and rationale. The decision is back to "open" with its existing constraints and considered topologies intact. The old choice is visible in the event log and in the snapshot if one was taken.

### Pattern 3: Constraint Checking (pass/warn/fail)
**What:** Sum node-level values per topology, compare against decision constraints. Follow the exact same scored report pattern as redundancy/capacity/RPO analysis.
**When to use:** For ANLZ-02 (analyze with constraints) and comparison output.

```rust
// Constraint check result, one per constraint type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintResult {
    pub constraint_type: String,  // "budget", "noise", "power", "rack_units"
    pub limit: f64,
    pub actual: f64,
    pub status: ConstraintStatus, // Pass, Warn, Fail
    pub margin: f64,              // positive = headroom, negative = overage
    pub margin_pct: f64,          // percentage headroom/overage
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConstraintStatus {
    Pass,  // actual <= limit * 0.9
    Warn,  // limit * 0.9 < actual <= limit
    Fail,  // actual > limit
}

// Pure function: no database access
pub fn check_constraints(
    constraints: &[DecisionConstraint],
    nodes: &[Node],  // nodes with cost_estimate, noise_db, etc.
) -> Vec<ConstraintResult> {
    // Sum node fields, compare against each constraint
}
```

### Pattern 4: Topology Comparison
**What:** Side-by-side metrics comparison of two topologies. Optionally includes structural diff (reusing existing diff engine) and constraint evaluation when a decision context is provided.
**When to use:** For ANLZ-08 and decision comparison.

```rust
// Comparison report for two topologies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonReport {
    pub topology_a: TopologyMetrics,
    pub topology_b: TopologyMetrics,
    pub constraints: Option<Vec<ConstraintComparison>>, // when decision context
    pub diff: Option<serde_json::Value>,                // when --diff flag
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologyMetrics {
    pub name: String,
    pub node_count: usize,
    pub volume_count: usize,
    pub total_capacity_bytes: i64,
    pub total_usable_bytes: i64,
    pub dataset_count: usize,
    pub total_cost_estimate: f64,
    pub total_noise_db: f64,
    pub total_power_watts: f64,
    pub total_rack_units: f64,
    pub redundancy_score: f64,
    pub capacity_score: f64,
    pub rpo_score: f64,
}
```

### Pattern 5: Schema Migration v3
**What:** Single migration adding all new tables and columns. Bumps CURRENT_VERSION to 3.
**When to use:** Applied on db.open() via the existing migration system.

```sql
-- New columns on nodes table
ALTER TABLE nodes ADD COLUMN cost_estimate REAL;
ALTER TABLE nodes ADD COLUMN noise_db REAL;
ALTER TABLE nodes ADD COLUMN power_watts REAL;
ALTER TABLE nodes ADD COLUMN rack_units REAL;

-- Decisions table
CREATE TABLE decisions (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'draft',
    parent_id TEXT REFERENCES decisions(id),
    chosen_topology_id TEXT REFERENCES topologies(id),
    rationale TEXT,
    snapshot TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    closed_at TEXT
);

-- Decision constraints
CREATE TABLE decision_constraints (
    id TEXT PRIMARY KEY,
    decision_id TEXT NOT NULL REFERENCES decisions(id) ON DELETE CASCADE,
    constraint_type TEXT NOT NULL,
    max_value REAL NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(decision_id, constraint_type)
);

-- Decision-topology junction (considered topologies)
CREATE TABLE decision_topologies (
    id TEXT PRIMARY KEY,
    decision_id TEXT NOT NULL REFERENCES decisions(id) ON DELETE CASCADE,
    topology_id TEXT NOT NULL REFERENCES topologies(id) ON DELETE CASCADE,
    added_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(decision_id, topology_id)
);

-- Indexes
CREATE INDEX idx_decisions_status ON decisions(status);
CREATE INDEX idx_decisions_parent ON decisions(parent_id);
CREATE INDEX idx_decision_constraints_decision ON decision_constraints(decision_id);
CREATE INDEX idx_decision_topologies_decision ON decision_topologies(decision_id);
CREATE INDEX idx_decision_topologies_topology ON decision_topologies(topology_id);

PRAGMA user_version = 3;
```

### Anti-Patterns to Avoid
- **Do not enforce state machine in SQL CHECK constraints:** The valid transitions depend on the current state and the target state together, which is best validated in application code. Use application-level validation.
- **Do not create a separate "decision_snapshots" table:** A JSON blob in the decisions table is sufficient for the snapshot requirement.
- **Do not use ON DELETE CASCADE from decisions to topologies:** Deleting a decision should NOT delete considered topologies. The cascade goes the other direction: deleting a topology should remove it from decision_topologies.
- **Do not sum noise_db values naively for multiple nodes:** For noise, the context discussion specified "max dB" constraint. However, summing is what was decided ("Sum across topology nodes for totals"). Follow the user's decision: sum all node noise_db values.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Entity resolution | Custom decision resolver | Extend existing resolve.rs pattern | resolve_decision() follows same exact pattern as resolve_topology() |
| Event recording | Custom event tracking | Use existing record_event() | Same entity_type/event_type pattern, add "decision", "decision_constraint", "decision_topology" |
| Diff engine | New comparison infrastructure | Reuse existing diff_entities_by_name() | The diff engine in topology.rs already handles entity-level + field-level diffs |
| Pass/warn/fail output | Custom formatting | Follow analyze.rs print_analysis_header() pattern | Consistent user experience with existing analysis commands |
| UUID generation | Custom IDs | uuid::Uuid::new_v4() | Already used everywhere |
| State serialization | Custom serialization | serde_json to_value/to_string | Already used for all before/after event states |

**Key insight:** This phase is largely "more of the same" -- every pattern needed already exists in the codebase. The decision entity follows the topology entity pattern. Constraint checking follows the analysis pattern. The comparison extends the diff pattern.

## Common Pitfalls

### Pitfall 1: Schema Migration Ordering
**What goes wrong:** ALTER TABLE statements adding columns to nodes must come before any queries that reference those columns. If the migration SQL references new columns in DEFAULT expressions or triggers, it can fail.
**Why it happens:** SQLite ALTER TABLE ADD COLUMN is limited -- it cannot add NOT NULL columns without defaults, or columns with complex defaults.
**How to avoid:** All four new node columns should be nullable REAL with no default (will be NULL for existing nodes). Test migration from v2 -> v3 explicitly.
**Warning signs:** "no such column" errors when querying after migration.

### Pitfall 2: Decision Topology Foreign Key Direction
**What goes wrong:** CASCADE DELETE in the wrong direction. If deleting a decision cascades to delete the TOPOLOGIES themselves, that would be catastrophic.
**Why it happens:** Confusion about which direction the relationship works.
**How to avoid:** decision_topologies has ON DELETE CASCADE on decision_id (deleting a decision cleans up the junction rows). It has ON DELETE CASCADE on topology_id too (deleting a topology removes it from consideration). Neither deletes the other end.
**Warning signs:** Topologies disappearing when decisions are deleted.

### Pitfall 3: Node Field Updates and the Event System
**What goes wrong:** Adding cost_estimate, noise_db, power_watts, rack_units to nodes requires updating the Node struct, its from_row(), insert(), and to_json() methods. If any are missed, undo/redo breaks because the serialized before/after state won't round-trip correctly.
**Why it happens:** The event system uses serde_json serialization of the full entity. Missing fields means deserialization produces wrong values on undo.
**How to avoid:** Update ALL four model methods (new, insert, from_row, to_json) and add the fields to the struct. The node update CLI command also needs new flags. Test undo/redo with the new fields.
**Warning signs:** Undo restores a node with NULL values for the new fields even though they were set.

### Pitfall 4: Constraint Type Validation
**What goes wrong:** Accepting arbitrary strings for constraint_type allows typos ("budgett", "noice") that silently fail constraint checking.
**Why it happens:** No enum validation on the constraint_type column.
**How to avoid:** Validate constraint_type against an exhaustive match ("budget", "noise", "power", "rack_units") at the CLI layer before inserting. Return clear error for unknown types.
**Warning signs:** Constraints that never trigger pass/warn/fail because the type string doesn't match.

### Pitfall 5: Entity Resolver Registration
**What goes wrong:** New entity types (decision, decision_constraint, decision_topology) need to be registered in events.rs entity_table_name() and restore_entity_from_json() for undo/redo to work.
**Why it happens:** Easy to forget when adding new entity types.
**How to avoid:** After adding model structs, immediately update entity_table_name() and restore_entity_from_json() in events.rs. Write a test that creates and undoes each new entity type.
**Warning signs:** "Unknown entity type" errors on undo.

### Pitfall 6: Block-Scoped Prepared Statements (D023)
**What goes wrong:** Prepared statements borrow the database connection. If a prepared statement is alive when a transaction is started, the borrow checker rejects it.
**Why it happens:** Rust borrow rules.
**How to avoid:** Use the block-scoping pattern already established throughout the codebase: wrap stmt.query_map() in a { } block that ends before the transaction starts. See fork() in topology.rs for the canonical example.
**Warning signs:** Compile errors about borrowing `db` as mutable while immutable borrow exists.

### Pitfall 7: Decision Title vs Name
**What goes wrong:** Decisions use "title" not "name" -- they allow spaces and don't need to be slugs. Entity resolution uses title matching, not slug matching.
**Why it happens:** Decisions are user-facing descriptions, not programmatic identifiers.
**How to avoid:** Decision resolution should match by exact title OR UUID prefix. No slug validation for decision titles. Enforce uniqueness of title in the database (UNIQUE constraint).
**Warning signs:** Users can't create decisions with readable titles like "NAS upgrade 2026".

## Code Examples

### Entity Model Pattern (following existing codebase)
```rust
// Source: Existing pattern from src/core/models.rs
impl Decision {
    pub fn new(title: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title: title.into(),
            description: String::new(),
            status: "draft".to_string(),
            parent_id: None,
            chosen_topology_id: None,
            rationale: None,
            snapshot: None,
            created_at: now,
            updated_at: now,
            closed_at: None,
        }
    }

    pub fn insert(&self, tx: &Transaction) -> rusqlite::Result<()> {
        tx.execute(
            "INSERT INTO decisions (id, title, description, status, parent_id, \
             chosen_topology_id, rationale, snapshot, created_at, updated_at, closed_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                self.id, self.title, self.description, self.status,
                self.parent_id, self.chosen_topology_id, self.rationale,
                self.snapshot,
                self.created_at.to_rfc3339(), self.updated_at.to_rfc3339(),
                self.closed_at.map(|dt| dt.to_rfc3339()),
            ],
        )?;
        Ok(())
    }

    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let created_str: String = row.get("created_at")?;
        let updated_str: String = row.get("updated_at")?;
        let closed_str: Option<String> = row.get("closed_at")?;
        Ok(Self {
            id: row.get("id")?,
            title: row.get("title")?,
            description: row.get("description")?,
            status: row.get("status")?,
            parent_id: row.get("parent_id")?,
            chosen_topology_id: row.get("chosen_topology_id")?,
            rationale: row.get("rationale")?,
            snapshot: row.get("snapshot")?,
            created_at: DateTime::parse_from_rfc3339(&created_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_at: DateTime::parse_from_rfc3339(&updated_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            closed_at: closed_str.and_then(|s|
                DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .ok()
            ),
        })
    }

    pub fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string(self)?)
    }
}
```

### CLI Command Pattern (following existing codebase)
```rust
// Source: Existing pattern from src/cli/topology.rs, src/cli/node.rs
#[derive(Subcommand)]
pub enum DecisionCommands {
    /// Create a new decision
    Create {
        /// Decision title (can include spaces)
        title: String,
        /// Optional description
        #[arg(long, default_value = "")]
        description: String,
        /// Optional parent decision for grouping
        #[arg(long)]
        parent: Option<String>,
    },
    /// Show decision details
    Show {
        /// Decision title or ID prefix
        decision: String,
    },
    /// List decisions
    List {
        /// Filter by status (draft, open, decided, abandoned)
        #[arg(long)]
        status: Option<String>,
    },
    /// Update decision title or description
    Update {
        decision: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        description: Option<String>,
        /// Change status to "open"
        #[arg(long)]
        open: bool,
    },
    /// Add a constraint to a decision
    Constrain {
        decision: String,
        /// Constraint type: budget, noise, power, rack_units
        #[arg(long)]
        r#type: String,
        /// Maximum value (dollars, dB, watts, or U)
        #[arg(long)]
        max: f64,
    },
    /// Remove a constraint from a decision
    Unconstrain {
        decision: String,
        /// Constraint type to remove
        #[arg(long)]
        r#type: String,
    },
    /// Consider a topology for this decision
    Consider {
        decision: String,
        /// Topology name or ID to consider
        topology: String,
    },
    /// Remove a topology from consideration
    Unconsider {
        decision: String,
        /// Topology name or ID to remove
        topology: String,
    },
    /// Close a decision with a chosen topology
    Choose {
        decision: String,
        /// Chosen topology name or ID
        topology: String,
        /// Rationale for the choice (required)
        #[arg(long)]
        rationale: String,
    },
    /// Abandon a decision without choosing
    Abandon {
        decision: String,
        /// Optional reason for abandonment
        #[arg(long)]
        reason: Option<String>,
    },
    /// Reopen a closed/abandoned decision
    Reopen {
        decision: String,
    },
}
```

### Constraint Check Pattern (following analysis pattern)
```rust
// Source: Existing pattern from src/domains/storage/analysis.rs
pub fn check_constraints(
    constraints: &[DecisionConstraint],
    nodes: &[Node],  // Node struct with new fields
) -> ConstraintReport {
    let mut results = Vec::new();

    for constraint in constraints {
        let actual: f64 = match constraint.constraint_type.as_str() {
            "budget" => nodes.iter().filter_map(|n| n.cost_estimate).sum(),
            "noise" => nodes.iter().filter_map(|n| n.noise_db).sum(),
            "power" => nodes.iter().filter_map(|n| n.power_watts).sum(),
            "rack_units" => nodes.iter().filter_map(|n| n.rack_units).sum(),
            _ => continue,
        };

        let limit = constraint.max_value;
        let margin = limit - actual;
        let margin_pct = if limit > 0.0 { (margin / limit) * 100.0 } else { 0.0 };

        let status = if actual > limit {
            ConstraintStatus::Fail
        } else if actual > limit * 0.9 {
            ConstraintStatus::Warn
        } else {
            ConstraintStatus::Pass
        };

        results.push(ConstraintResult {
            constraint_type: constraint.constraint_type.clone(),
            limit,
            actual,
            status,
            margin,
            margin_pct,
        });
    }

    let has_failures = results.iter().any(|r| matches!(r.status, ConstraintStatus::Fail));
    let score = if results.is_empty() {
        100.0
    } else {
        let passing = results.iter().filter(|r| matches!(r.status, ConstraintStatus::Pass)).count();
        (passing as f64 / results.len() as f64) * 100.0
    };

    ConstraintReport { score, results, has_failures }
}
```

### Node Update Extension Pattern
```rust
// Source: Existing pattern from src/cli/node.rs update function
// Add new CLI flags for node update:
Update {
    // ... existing flags ...
    /// Estimated cost in dollars
    #[arg(long)]
    cost: Option<f64>,
    /// Noise level in dB
    #[arg(long)]
    noise: Option<f64>,
    /// Power consumption in watts (separate from existing power_draw)
    #[arg(long)]
    power: Option<f64>,
    /// Rack units consumed
    #[arg(long)]
    rack_units: Option<f64>,
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| is_active boolean | tag-based lifecycle (Phase 3) | Schema v2 | Decisions use same lifecycle pattern but with different states |
| No analysis | Scored reports (Phase 4) | Schema v2 era | Constraint checking follows same scored report pattern |
| Manual comparison | Diff engine (Phase 3) | Phase 3 | Topology comparison extends diff with metrics |

**No deprecated patterns to worry about.** The codebase is clean and consistent.

## Discretion Recommendations

These are areas marked as "Claude's Discretion" where I provide specific recommendations.

### Decision Hierarchy
**Recommendation:** Flat with optional parent_id. Add a `parent_id TEXT REFERENCES decisions(id)` column. No tree enforcement, no recursive queries. If a user groups sub-decisions under a parent, `sp decision list` can optionally show the tree structure (like `sp topology tree`). Deleting a parent does NOT cascade to children -- children become orphans (parent_id points to deleted row, but no FK enforcement on parent_id -- or use SET NULL).

### Reopen Behavior
**Recommendation:** On reopen, set status back to "open", clear chosen_topology_id and rationale, keep constraints and considered topologies intact. The snapshot (if taken) remains as historical record. Events log captures the full history. This is simplest and avoids complex "restore previous state" logic.

### Decision Show Command
**Recommendation:** Follow topology show pattern. Show decision metadata (title, status, description, created_at, parent), then inline: constraints listed, considered topologies listed, chosen topology if decided. JSON mode returns the full decision object with nested constraints and topologies arrays.

### Comparison Indicators
**Recommendation:** Use advantage indicators in text mode. For each metric, append an arrow showing which topology is better. For budget: lower is better. For capacity: higher is better. For analysis scores: higher is better. Example:
```
  Total cost:     $850 vs $1,200   <- Topology A
  Total capacity: 16TB vs 24TB     <- Topology B
  Redundancy:     100% vs 75%      <- Topology A
```
In JSON mode, include a `"better"` field per metric: `"a"`, `"b"`, or `"tie"`.

### Comparison JSON Format
**Recommendation:** Object with `topology_a` and `topology_b` as top-level keys, each containing the full metrics object. Include a `metrics_comparison` array for agent consumption:
```json
{
  "topology_a": { "name": "...", "metrics": { ... } },
  "topology_b": { "name": "...", "metrics": { ... } },
  "metrics_comparison": [
    { "metric": "total_cost", "a": 850, "b": 1200, "better": "a", "unit": "$" },
    ...
  ],
  "constraints": [ ... ]  // if decision context
}
```

### Rationale Capture
**Recommendation:** Required for `decide` (--rationale flag, no default). Optional for `abandon` (--reason flag). This matches the user's "session continuity" principle -- the rationale explains WHY this topology was chosen and is critical for future reference. Abandonment is inherently less important to document.

### Abandon Reasons
**Recommendation:** Freeform string via optional --reason flag. No predefined categories. Keep it simple.

### Decision Snapshot
**Recommendation:** YES, implement snapshot. At close time (decide or abandon), serialize the current comparison data as a JSON blob in the `snapshot` column. This is cheap (one JSON string), preserves the historical record, and follows the "session continuity" principle. Without it, if topologies are modified after a decision is made, the historical context is lost. The snapshot should include: constraint check results for all considered topologies, and basic metrics for each topology.

## Open Questions

1. **Node power_draw_watts vs power_watts**
   - What we know: Nodes already have `power_draw_watts` field. The context says to add `power_watts` for constraint checking.
   - What's unclear: Should these be the same field, or separate? `power_draw_watts` is the existing field from Phase 1. The context explicitly says "Add cost_estimate, noise_db, power_watts, rack_units fields to nodes."
   - Recommendation: The existing `power_draw_watts` and the new `power_watts` likely represent the same concept. However, the user explicitly said to add `power_watts`. I recommend using the existing `power_draw_watts` field for constraint checking instead of adding a redundant field, but the planner should note this and may want to confirm with the user. If they are different (e.g., power_draw_watts is idle draw, power_watts is max draw), keep both. If the same, skip adding power_watts and use power_draw_watts in constraint checks.

2. **Decision Title Uniqueness**
   - What we know: Topologies enforce unique names. Decisions use "title" instead.
   - What's unclear: Should titles be unique? Unlike topology names which are used for resolution, decision titles are more like display labels.
   - Recommendation: Enforce UNIQUE on title for consistency with entity resolver pattern. This allows `sp decision show "NAS upgrade"` to work unambiguously by title.

## Sources

### Primary (HIGH confidence)
- Source code analysis of all files in src/core/ (db.rs, models.rs, resolve.rs, events.rs, specs.rs)
- Source code analysis of all files in src/cli/ (mod.rs, topology.rs, node.rs, analyze.rs)
- Source code analysis of src/domains/storage/analysis.rs
- Schema inspection: 9 existing tables, migration v1 + v2, PRAGMA user_version tracking
- Cargo.toml dependency list

### Secondary (MEDIUM confidence)
- Phase 4 research document (.planning/phases/04-analysis-functions/04-RESEARCH.md) for established patterns

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - no new dependencies needed, everything already in Cargo.toml
- Architecture: HIGH - every pattern is directly observable in existing codebase
- Schema migration: HIGH - migration system is well-understood, v1 and v2 provide templates
- Pitfalls: HIGH - based on direct observation of borrow checker patterns, event system requirements
- Discretion recommendations: MEDIUM - these are judgment calls based on understanding the codebase patterns and user intent

**Research date:** 2026-02-07
**Valid until:** 2026-03-07 (stable -- no external dependencies changing)

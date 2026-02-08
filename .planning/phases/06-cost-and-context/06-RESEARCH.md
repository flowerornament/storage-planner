# Phase 6: Cost and Context - Research

**Researched:** 2026-02-07
**Domain:** Catalog/pricing data model, cost analysis, CLI context commands, YAML import/export, ASCII diagrams
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Both per-entity breakdown AND category summary views available -- user picks with a flag
- Default shows separate one-time and recurring sections; `--tco=3yr` flag adds total cost of ownership projection
- Price selection uses latest observation (most recent price recorded)
- `sp prime` is an agent bootstrap document (like `bd prime`), NOT a data dump
- Static instructional content (how to use sp, workflow guide, example commands) with dynamically appended state summary
- Instructions only -- no inline topology data. Agent runs `sp status` or specific commands for state
- Stdout only, no file output flag
- Complements CLAUDE.md -- CLAUDE.md has project-level info, `sp prime` has runtime command guide and usage patterns
- Include concrete example commands showing typical usage patterns
- `sp status` is a full health report: current topology + analysis summary, open decisions with status, catalog stats, recent activity
- Problems highlighted prominently at the top -- "2 datasets at risk, 1 decision open 30+ days" -- action-oriented alerts
- Supports `--format=json` consistent with all other commands
- YAML export: default preserves identity for backup (round-trip fidelity), `--template` flag strips IDs for reuse
- Export scope: default full graph, `--only=nodes,volumes` for partial export of large topologies
- ASCII diagram: standalone `sp diagram` command (not a flag on show)
- Two diagram perspectives: `sp diagram --tree` for node-volume-dataset hierarchy, `sp diagram --network` for link topology between nodes

### Claude's Discretion
- Catalog item linking model (direct vs bill of materials)
- Whether `sp status` runs inline mini-analysis or references last analysis results
- Diagram rendering implementation
- `sp prime` workflow guide structure (full workflow vs action-oriented -- pick what works best for agent bootstrap)

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

## Summary

Phase 6 is the final phase of the storage planner. It adds four major capability areas: (1) a catalog system for tracking items and their prices, (2) cost analysis that connects catalog prices to topology entities, (3) context commands (`sp prime`, `sp status`) for AI agent bootstrap and health monitoring, and (4) topology import/export with ASCII diagrams.

The codebase is mature with well-established patterns across 5 completed phases. The schema is at version 3 with a clean migration system. All CLI commands follow the same patterns: clap derive API, `OutputFormat` text/json switching, entity resolution via `resolve_*` functions, event logging for undo/redo, and block-scoped prepared statements (D023). There are currently no catalog tables in the schema -- `volumes.item_id` exists as a TEXT field with no FK constraint (noted as D003: "deferred to Phase 6"). No `src/pricing/` directory exists.

The phase touches every layer of the stack: new database tables (catalog items, prices), new CLI modules (item, price, status, prime, diagram, import/export), new analysis functions (cost, bandwidth), and YAML serialization. The YAML ecosystem in Rust needs care: `serde_yaml` is deprecated, and the recommended replacement is `serde_yaml_ng`.

**Primary recommendation:** Build catalog tables in migration v4, use `serde_yaml_ng` for YAML import/export, hand-roll ASCII diagrams using Unicode box-drawing characters and the existing `console` crate (no external diagram library needed), and model `sp prime` after `bd prime`'s action-oriented agent bootstrap pattern.

## Standard Stack

### Core (already in Cargo.toml)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| clap | 4.x | CLI argument parsing with derive | Already used for all commands |
| rusqlite | 0.31 (bundled) | SQLite database access | All entity CRUD, migrations |
| serde + serde_json | 1.x | Serialization | All JSON output, event state |
| console | 0.15 | Terminal styling | All colored output, status display |
| chrono | 0.4 | Timestamps | All entity timestamps |
| anyhow | 1.x | Error handling | All Result types |
| uuid | 1.x | Entity IDs | All UUID generation |

### New Dependencies Required
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| serde_yaml_ng | 0.10.x | YAML serialization/deserialization | Topology import/export (TOPO-10, TOPO-11) |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| serde_yaml_ng | serde_yml | serde_yml has known unsoundness issues (RUSTSEC-2025-0068); serde_yaml_ng is a cleaner fork |
| serde_yaml_ng | serde_yaml 0.9.34 | Deprecated, no longer maintained |
| External diagram crate | ascii-dag, termtree | Overkill; diagrams are simple tree/network layouts easily built with string formatting + box-drawing chars |

**Installation:**
```bash
cargo add serde_yaml_ng
```

## Architecture Patterns

### New CLI Module Structure
```
src/
├── cli/
│   ├── mod.rs              # Add: item, price, status, prime, diagram, import/export commands
│   ├── item.rs             # NEW: sp item add/show/list/search (CAT-01 through CAT-04)
│   ├── price.rs            # NEW: sp price add/list (CAT-05, CAT-06, CAT-07)
│   ├── status.rs           # NEW: sp status (CTX-01)
│   ├── prime.rs            # NEW: sp prime (CTX-02)
│   ├── diagram.rs          # NEW: sp diagram --tree/--network (TOPO-09)
│   ├── topology.rs         # MODIFY: add import/export subcommands (TOPO-10, TOPO-11)
│   ├── analyze.rs          # MODIFY: add bandwidth/cost subcommands (ANLZ-06, ANLZ-07)
│   └── ... (existing)
├── core/
│   ├── db.rs               # MODIFY: migration v4 (catalog tables, item FK)
│   ├── models.rs           # MODIFY: add Item, Price models
│   ├── resolve.rs          # MODIFY: add resolve_item
│   ├── events.rs           # MODIFY: add item/price entity types
│   └── ...
└── domains/
    └── storage/
        └── analysis.rs     # MODIFY: add analyze_cost, analyze_bandwidth
```

### Pattern 1: Catalog Item Linking (Discretion Decision: Direct Association)
**What:** Direct `item_id` foreign key on volumes (and optionally nodes) linking to catalog items
**When to use:** When each volume or node corresponds to a single purchasable product
**Why chosen over bill-of-materials:** The existing `volumes.item_id` field (D003) already establishes this pattern. A bill-of-materials model (many items per entity) adds complexity without clear benefit -- a volume IS a drive, a node IS a device. Multiple items per entity can be handled by multiple volumes per node (already supported).

```rust
// volumes.item_id already exists as TEXT
// Migration v4 adds FK constraint: REFERENCES catalog_items(id) ON DELETE SET NULL
// nodes.item_id added as optional field for node-level product tracking

// Cost analysis sums prices across all items referenced by entities in the topology
```

### Pattern 2: Price Observation Append-Only
**What:** Price observations are append-only (never updated/deleted), with the latest observation used for analysis
**When to use:** Always -- price history is valuable data
**Why:** Matches CLAUDE.md principle "Append-only prices -- Price history is preserved, never overwritten"

```rust
// Price model follows existing entity pattern: new/insert/from_row/to_json
pub struct Price {
    pub id: String,
    pub item_id: String,
    pub amount_cents: i64,          // Store in cents to avoid float issues
    pub currency: String,           // Default "USD"
    pub source: String,             // "bestbuy", "ebay", "amazon", "manual"
    pub condition: String,          // "new", "used", "refurbished", "open-box"
    pub price_type: String,         // "one-time", "monthly", "annual"
    pub observed_at: DateTime<Utc>,
}
```

### Pattern 3: Status Dashboard with Alerts
**What:** Problems at the top, then topology summary, decisions, catalog stats
**When to use:** `sp status` command
**Why:** User decision: "Problems highlighted prominently at the top"

**Recommendation for discretion area:** Run inline mini-analysis rather than referencing cached results. Rationale: The analysis functions are pure and fast (in-memory computation), there is no analysis caching mechanism in the codebase, and the user expects fresh data. The existing `sp analyze` dashboard already runs all analyses inline.

### Pattern 4: Prime as Agent Bootstrap
**What:** Static instructional markdown with dynamic state summary appended
**When to use:** `sp prime` command, called by AI agents at session start
**Why:** Models `bd prime` pattern -- provides workflow context agents need to operate

**Recommendation for discretion area:** Use action-oriented structure (not full workflow narrative). Group by what the agent needs to DO, not the full conceptual model. Pattern from `bd prime`:
1. Essential commands (grouped by task: finding data, modifying data, analyzing)
2. Common workflows (2-3 step sequences)
3. Dynamic state (current topology, open decisions, recent activity)

### Pattern 5: YAML Export with Identity Preservation
**What:** Default export includes UUIDs for round-trip fidelity; `--template` strips them
**When to use:** `sp topology export` and `sp topology import`

```yaml
# Default export (with IDs for backup/restore)
topology:
  id: "abc-123-..."
  name: "home-setup"
  nodes:
    - id: "def-456-..."
      name: "mac-mini"
      role: "desktop"
      volumes:
        - id: "ghi-789-..."
          name: "ssd-1"
          capacity_bytes: 1000000000000

# Template export (no IDs, for reuse)
topology:
  name: "home-setup"
  nodes:
    - name: "mac-mini"
      role: "desktop"
      volumes:
        - name: "ssd-1"
          capacity_bytes: 1000000000000
```

### Anti-Patterns to Avoid
- **Storing prices as floats:** Use integer cents (i64). Float arithmetic causes rounding errors in financial calculations. Display formatting converts cents to dollars at the presentation layer.
- **Coupling catalog to topology:** Items are global entities, not topology-scoped. An item (e.g., "Samsung 870 EVO 4TB") can be referenced by volumes across many topologies.
- **Caching analysis results:** The analysis functions are pure and fast. Adding a cache layer creates staleness bugs and complexity for no real performance gain at this data scale.
- **Inline topology data in prime output:** User explicitly decided against this. Prime provides instructions; agent uses `sp status` and specific commands for data.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| YAML serialization | Custom YAML parser/writer | `serde_yaml_ng` | YAML spec is complex (anchors, tags, multi-doc); serde derives handle round-tripping |
| Price formatting | Custom float-to-currency | `format!("${:.2}", cents as f64 / 100.0)` | Standard pattern, keep it simple; consistent with existing `format_constraint_display` |
| Cron parsing (for bandwidth analysis) | Custom schedule parser | `croner` (already in deps) | Already used for RPO analysis |
| Entity resolution | Custom name/ID lookup | `resolve_*` functions (existing) | Established pattern with UUID prefix disambiguation |

**Key insight:** Most infrastructure for this phase already exists. The migration system, event logging, entity resolution, output formatting, and analysis patterns are all established. New code follows existing patterns.

## Common Pitfalls

### Pitfall 1: Migration v4 Must Handle Existing item_id Data
**What goes wrong:** Migration adds FK constraint on `volumes.item_id` but existing rows may have non-null `item_id` values that don't reference any catalog_items row (since the table doesn't exist yet).
**Why it happens:** D003 established `item_id` as TEXT with no FK. Users may have set arbitrary strings.
**How to avoid:** Migration v4 must (1) CREATE the catalog_items table first, (2) only THEN add the FK. Since SQLite doesn't support `ALTER TABLE ADD CONSTRAINT`, the FK is best enforced at the application level (check on insert/update) rather than via schema. The existing pattern already works this way -- `item_id` has no FK in the schema and the test `test_volumes_item_id_no_fk` explicitly verifies this.
**Warning signs:** Tests with existing `item_id` values failing after migration.

### Pitfall 2: Price Type Confusion in Cost Analysis
**What goes wrong:** Mixing one-time and recurring costs in a single sum without clear separation.
**Why it happens:** "Add price observation" stores the type but analysis may sum everything together.
**How to avoid:** Cost analysis MUST separate one-time costs from recurring costs. The user explicitly decided: "Default shows separate one-time and recurring sections; `--tco=3yr` flag adds total cost of ownership projection." TCO = one_time + (monthly * 12 * years) + (annual * years).
**Warning signs:** A single "total cost" number without breakdown.

### Pitfall 3: Event System Entity Type Registration
**What goes wrong:** Undo/redo fails for new entity types because `entity_table_name()` and `restore_entity_from_json()` don't know about them.
**Why it happens:** Both functions in `events.rs` use exhaustive match statements that must be extended.
**How to avoid:** When adding `CatalogItem` and `Price` models, also update: (1) `entity_table_name()` to map "catalog_item" -> "catalog_items" and "price" -> "prices", (2) `restore_entity_from_json()` to handle deserialization for both types.
**Warning signs:** "Unknown entity type" errors on undo after item/price operations.

### Pitfall 4: Import ID Collision
**What goes wrong:** Importing a YAML file with existing IDs causes UNIQUE constraint violations.
**Why it happens:** Default export preserves IDs for backup fidelity. Re-importing into the same database collides.
**How to avoid:** Import should generate new UUIDs for all entities (like `topology fork` does), with an ID remapping table to preserve internal references. The fork code in `topology.rs` already implements this exact pattern with `node_map`, `volume_map`, `dataset_map`.
**Warning signs:** SQL UNIQUE constraint errors during import.

### Pitfall 5: Bandwidth Analysis Scope
**What goes wrong:** Building a complex network flow analysis when a simpler check suffices.
**Why it happens:** ANLZ-06 "can links support sync regimes?" sounds like it needs path-finding.
**How to avoid:** For v1, bandwidth analysis should check: for each sync regime, does the link between source and target nodes have sufficient bandwidth for the data volume? This is a direct lookup (source_node -> target_node link), not a multi-hop path-finding problem. The REQUIREMENTS.md explicitly puts "bandwidth analysis with path finding" in v2 (ANLZ-10).
**Warning signs:** Implementing Dijkstra's algorithm or graph traversal for a v1 feature.

### Pitfall 6: YAML Crate Selection
**What goes wrong:** Using deprecated `serde_yaml` or unsound `serde_yml`.
**Why it happens:** `serde_yaml` appears in many examples online. `serde_yml` has the closest name.
**How to avoid:** Use `serde_yaml_ng` specifically. It is a conservative fork of dtolnay's original, maintained, and does not have the soundness issues of `serde_yml` (RUSTSEC-2025-0068).
**Warning signs:** Cargo audit warnings, deprecation notices.

## Code Examples

### Schema Migration v4 (catalog tables)
```sql
-- Catalog items: products the user is considering
CREATE TABLE catalog_items (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    category TEXT NOT NULL DEFAULT '',
    specs TEXT NOT NULL DEFAULT '{}',  -- JSON blob for flexible specs
    url TEXT,
    notes TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Price observations: append-only price history
CREATE TABLE prices (
    id TEXT PRIMARY KEY,
    item_id TEXT NOT NULL REFERENCES catalog_items(id) ON DELETE CASCADE,
    amount_cents INTEGER NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',
    source TEXT NOT NULL DEFAULT 'manual',
    condition TEXT NOT NULL DEFAULT 'new',
    price_type TEXT NOT NULL DEFAULT 'one-time',
    observed_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_prices_item ON prices(item_id);
CREATE INDEX idx_prices_observed ON prices(observed_at);
CREATE INDEX idx_catalog_items_category ON catalog_items(category);

-- Add item_id to nodes (optional product tracking)
ALTER TABLE nodes ADD COLUMN item_id TEXT;

PRAGMA user_version = 4;
```

### CatalogItem Model (following existing entity pattern)
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogItem {
    pub id: String,
    pub name: String,
    pub category: String,
    pub specs: String,       // JSON string
    pub url: Option<String>,
    pub notes: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl CatalogItem {
    pub fn new(name: impl Into<String>, category: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            category: category.into(),
            specs: "{}".to_string(),
            url: None,
            notes: String::new(),
            created_at: now,
            updated_at: now,
        }
    }
    // insert/from_row/to_json follow exact same pattern as Topology, Node, etc.
}
```

### Cost Analysis Function (pure function pattern)
```rust
pub struct CostReport {
    pub one_time_total_cents: i64,
    pub monthly_total_cents: i64,
    pub annual_total_cents: i64,
    pub items: Vec<CostLineItem>,
    pub tco_cents: Option<i64>,       // Set when --tco=Nyr is used
    pub tco_years: Option<i32>,
}

pub struct CostLineItem {
    pub item_name: String,
    pub entity_type: String,    // "volume" or "node"
    pub entity_name: String,
    pub amount_cents: i64,
    pub price_type: String,
    pub source: String,
    pub condition: String,
}

// Pure function: takes pre-loaded data, returns report
pub fn analyze_cost(
    nodes: &[Node],
    volumes: &[Volume],
    items: &[CatalogItem],
    prices: &[Price],
    tco_years: Option<i32>,
) -> CostReport {
    // For each node/volume with item_id:
    //   Find the item in items
    //   Find the latest price for that item (max observed_at)
    //   Categorize by price_type
    // Sum one-time, monthly, annual separately
    // If tco_years set: tco = one_time + (monthly * 12 * years) + (annual * years)
}
```

### ASCII Tree Diagram (using box-drawing characters)
```rust
// sp diagram --tree output example:
//
// home-setup [current]
// +-- mac-mini [desktop] (office)
// |   +-- ssd-1: 1.0TB apfs
// |   |   +-- photos (500.0GB) [primary]
// |   |   +-- documents (100.0GB) [primary]
// |   +-- external-1: 4.0TB hfs+
// |       +-- photos (500.0GB) [backup]
// +-- nas-01 [nas] (closet)
//     +-- pool-1: 8.0TB zfs/raidz1
//         +-- photos (500.0GB) [replica]
//         +-- documents (100.0GB) [replica]
//         +-- media (2.0TB) [primary]

// sp diagram --network output example:
//
// mac-mini ----[lan/1Gbps]---- nas-01
//     |                           |
//     +---[wan/100Mbps]--- cloud-backup
//
// Links:
//   mac-mini -> nas-01: lan, 1.0GB/s
//   mac-mini -> cloud-backup: wan, 100.0MB/s, metered ($0.09/GB)
```

### YAML Export Structure
```rust
use serde_yaml_ng;

#[derive(Serialize, Deserialize)]
struct TopologyExport {
    topology: TopologyData,
}

#[derive(Serialize, Deserialize)]
struct TopologyData {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    name: String,
    description: String,
    nodes: Vec<NodeExport>,
    datasets: Vec<DatasetExport>,
    links: Vec<LinkExport>,
    sync_regimes: Vec<SyncRegimeExport>,
}

// For --template mode: skip_serializing_if on all ID fields
// For default mode: include all IDs for round-trip fidelity

fn export_topology(db: &Database, topo_id: &str, template: bool) -> Result<String> {
    // Load all entities (same pattern as topology fork)
    // Build export struct
    // If template: set all ID fields to None
    // Serialize with serde_yaml_ng::to_string()
}

fn import_topology(db: &mut Database, yaml_content: &str) -> Result<String> {
    // Deserialize from YAML
    // Generate new IDs for all entities (reuse fork's remapping pattern)
    // Insert in transaction with event logging
}
```

### Status Dashboard Output
```rust
// sp status output example:
//
// Problems:
//   2 datasets at risk (run: sp analyze redundancy)
//   1 decision open 30+ days: "NAS Upgrade" (run: sp decision show "NAS Upgrade")
//
// Topology: home-setup [current]
//   Nodes: 3 | Volumes: 5 | Datasets: 4
//   Redundancy: 75% | Capacity: 100% | RPO: 100%
//
// Decisions:
//   [open]    "NAS Upgrade 2026" (2 options, budget: $1,500)
//   [decided] "SSD Choice" -> nvme-option (closed 2026-01-15)
//
// Catalog: 8 items, 23 price observations
//   Latest: Samsung 870 EVO 4TB @ $229.99 (Best Buy, 2d ago)
```

### Prime Output Structure
```rust
// sp prime output (static instructions + dynamic state):
//
// # Storage Planner Context
//
// ## Quick Reference
//
// ### Viewing State
// ```
// sp status                    # Health report with alerts
// sp topology show <name>      # Topology details
// sp analyze                   # Full analysis dashboard
// sp item list                 # Catalog items
// ```
//
// ### Building Topologies
// ```
// sp topology create <name>    # New topology
// sp node add <name> --role=<role> --location=<loc>
// sp volume add <name> --node=<node> --capacity=4TB
// sp dataset add <name> --size=500GB --criticality=critical --min-copies=2
// sp placement add <dataset> <volume>
// ```
//
// ### Making Decisions
// ```
// sp decision create "title"   # New decision
// sp decision constrain "title" --type=budget --max=1500
// sp decision consider "title" --topology=<name>
// sp analyze compare <a> <b> --decision="title"
// sp decision choose "title" --topology=<name> --rationale="..."
// ```
//
// ## Current State
//
// Topology: home-setup [current] (3 nodes, 5 volumes, 4 datasets)
// Decisions: 1 open, 2 decided
// Catalog: 8 items
// Last activity: 2h ago
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `serde_yaml` (dtolnay) | `serde_yaml_ng` (acatton fork) | 2024-04 (deprecation) | Must use ng for new YAML features |
| `volumes.item_id` no FK | Application-level validation | Phase 6 | Keep no-FK pattern per D003, validate in CLI code |
| Node cost in `cost_estimate` only | Node cost via catalog item link | Phase 6 | Richer cost data with price history |

**Deprecated/outdated:**
- `serde_yaml 0.9.34+deprecated`: Archived by dtolnay, use `serde_yaml_ng` instead
- `serde_yml`: Has RUSTSEC-2025-0068 advisory for unsoundness, avoid

## Open Questions

1. **Should nodes.item_id have a FK constraint?**
   - What we know: `volumes.item_id` explicitly has no FK (D003, with a test verifying this). Adding `item_id` to nodes follows the same pattern.
   - What's unclear: Whether the user wants application-level validation only (current pattern) or a proper FK constraint for the new catalog_items table.
   - Recommendation: Keep application-level validation consistent with D003. The test `test_volumes_item_id_no_fk` establishes this as an intentional pattern. Migration v4 does NOT add FK constraints to `item_id` columns.

2. **Bandwidth analysis data requirements**
   - What we know: ANLZ-06 asks "can links support sync regimes?" Links have `bandwidth_bytes_sec`. Sync regimes have `schedule` (cron) and reference datasets with `size_bytes`.
   - What's unclear: How to calculate required bandwidth. Is it dataset_size / sync_interval? Or should it consider incremental sync (only changed data)?
   - Recommendation: Use conservative estimate: `dataset.size_bytes / cron_interval_seconds`. Flag links where required bandwidth exceeds available. Note this is a worst-case (full sync) estimate; real incremental syncs would be much smaller.

3. **CTX-03: Show/set current topology shortcut**
   - What we know: `sp topology tag <name> current` already sets the current topology. `sp topology list` shows which is current.
   - What's unclear: Whether CTX-03 needs a dedicated shortcut command (e.g., `sp use <topology>`) or if existing commands suffice.
   - Recommendation: Add a shortcut `sp use <topology>` that delegates to `sp topology tag <name> current` for ergonomics. Also support `sp use` (no args) to display current topology name.

## Sources

### Primary (HIGH confidence)
- Codebase analysis: `src/core/db.rs` (schema v1-v3, migration pattern)
- Codebase analysis: `src/core/models.rs` (entity model pattern: new/insert/from_row/to_json)
- Codebase analysis: `src/core/events.rs` (undo/redo, entity_table_name, restore_entity_from_json)
- Codebase analysis: `src/cli/analyze.rs` (analysis function integration pattern)
- Codebase analysis: `src/cli/topology.rs` (fork deep-copy with ID remapping -- reusable for import)
- Codebase analysis: `src/domains/storage/analysis.rs` (pure analysis function pattern)
- Codebase analysis: `Cargo.toml` (current dependency versions)
- Codebase analysis: `.planning/STATE.md` (decisions D003, D009, D023)

### Secondary (MEDIUM confidence)
- [serde_yaml_ng GitHub](https://github.com/acatton/serde-yaml-ng) - Conservative dtolnay fork, v0.10
- [RUSTSEC-2025-0068](https://rustsec.org/advisories/RUSTSEC-2025-0068.html) - serde_yml unsoundness advisory
- [Rust forum: serde_yaml alternatives](https://users.rust-lang.org/t/serde-yaml-deprecation-alternatives/108868) - Community consensus on replacements
- `bd prime` output analysis - Pattern for agent bootstrap documents
- CLI Design Principles (`~/.nix-config/.agents/CLI_DESIGN_PRINCIPLES.md`) - Color semantics, progressive disclosure

### Tertiary (LOW confidence)
- [ascii-dag crate](https://crates.io/crates/ascii-dag) - Investigated but not recommended (overkill for tree/network diagrams)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All dependencies are existing or well-verified (serde_yaml_ng)
- Architecture: HIGH - All patterns are direct extensions of established codebase conventions
- Pitfalls: HIGH - Identified from deep codebase analysis (D003, migration system, event system)
- Cost analysis: HIGH - Requirements are explicit and implementation follows existing analysis pattern
- YAML import/export: MEDIUM - serde_yaml_ng API assumed compatible with serde_yaml based on project claims
- ASCII diagrams: MEDIUM - Hand-rolled approach recommended; rendering quality depends on implementation

**Research date:** 2026-02-07
**Valid until:** 2026-03-07 (30 days - stable domain, no fast-moving dependencies)

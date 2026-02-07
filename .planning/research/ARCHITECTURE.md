# Architecture Patterns: Rust CLI with Versioned Graph Modeling

**Domain:** CLI tool for AI-assisted purchase decisions with topology modeling
**Researched:** 2026-02-06
**Confidence:** MEDIUM-HIGH (verified against existing codebase patterns, web research for graph storage)

## Recommended Architecture

### Overview

Extend the existing modular architecture rather than restructuring. The current codebase has clean separation:

```
src/
├── main.rs               # CLI entry, parses args, dispatches
├── lib.rs                # Crate root, module exports
├── core/                 # Domain-agnostic abstractions
│   ├── db.rs             # Database connection, transactions, migrations
│   ├── models.rs         # Item, Price, Configuration, Decision, Event
│   ├── events.rs         # Append-only audit log
│   └── specs.rs          # Typed attribute parsing
├── cli/                  # Command implementations
│   ├── mod.rs            # Cli struct, Commands enum, dispatch
│   ├── item.rs           # sp item add/list/show/compare
│   ├── price.rs          # sp price add/list
│   ├── config.rs         # sp config create/add-item
│   ├── decide.rs         # sp decide create/compare/choose
│   ├── analyze.rs        # sp analyze (runs analysis)
│   ├── prime.rs          # sp prime (context for AI)
│   └── ...
├── domains/storage/      # Storage-specific domain logic
│   ├── models.rs         # Node, Volume, Dataset, SyncRegime
│   └── analysis.rs       # Pure analysis functions
└── pricing/              # External API integrations
    ├── bestbuy.rs
    ├── ebay.rs
    └── ...
```

### Recommended Additions

```
src/
├── core/
│   ├── topology.rs       # NEW: Topology struct, versioning, tags
│   └── graph.rs          # NEW: Graph operations (load, traverse, diff)
├── cli/
│   ├── topology.rs       # NEW: sp topology create/fork/diff/show
│   └── node.rs           # NEW: sp node add/edit (convenience for topology)
├── domains/storage/
│   ├── topology_analysis.rs  # NEW: Redundancy, failure sim, RPO, bandwidth
│   └── projections.rs    # NEW: Capacity, cost projections
└── output/               # NEW: Structured output formatting
    ├── text.rs           # Human-readable tables, styled output
    ├── json.rs           # JSON serialization
    └── prime.rs          # AI-optimized context format
```

## Component Boundaries

| Component | Responsibility | Communicates With |
|-----------|---------------|-------------------|
| `cli/` | Parse args, validate input, call core/domain, format output | `core/`, `domains/`, `output/` |
| `core/db` | Connection management, transactions, migrations | rusqlite only |
| `core/models` | Data structs, CRUD operations, from_row/insert | `core/db` |
| `core/topology` | Topology versioning, fork/tag operations | `core/db`, `core/models` |
| `core/graph` | Graph construction, traversal, diffing | `core/topology`, `domains/storage/models` |
| `domains/storage/models` | Node, Volume, Dataset, SyncRegime structs | None (pure data) |
| `domains/storage/analysis` | Pure analysis functions (no I/O) | `domains/storage/models` only |
| `output/` | Format data for display (text, JSON, YAML) | Data structs only |

### Key Principle: Layered Dependencies

```
cli/ (presentation layer)
  ↓
output/ (formatting)
  ↓
domains/ (business logic, analysis)
  ↓
core/ (data access, models)
  ↓
rusqlite, serde (external)
```

**No upward dependencies.** `core/` never imports from `cli/`. `domains/` never imports from `cli/` or `output/`.

## Data Flow

### Command Execution Flow

```
User input → Cli::parse() → Commands::run()
                                ↓
                         Open database (core/db)
                                ↓
                         Load models (core/models)
                                ↓
                         Domain logic (domains/)
                                ↓
                         Format output (output/)
                                ↓
                         Print to stdout
```

### Graph Loading Flow (for topology commands)

```
sp topology show <id>
        ↓
topology.rs: load Topology from DB
        ↓
graph.rs: construct in-memory graph from:
  - Nodes table
  - Volumes table (join to nodes)
  - Datasets table
  - Dataset_placements table (join datasets to volumes)
  - Sync_regimes table (edges: volume → volume)
  - Links table (edges: node ↔ node)
        ↓
Return structured Graph for analysis or display
```

### Analysis Flow

```
sp analyze --topology=<id>
        ↓
Load topology graph (as above)
        ↓
Call pure analysis functions:
  - analyze_redundancy(nodes, volumes, datasets, placements, syncs)
  - analyze_capacity(volumes, datasets)
  - analyze_rpo(datasets, syncs)
  - simulate_failure(graph, failing_node)
        ↓
Collect AnalysisResult
        ↓
Format and print
```

## Patterns to Follow

### Pattern 1: Transaction-Scoped Operations

**What:** All mutations happen within `db.transaction()`. The closure receives `&Transaction` and returns `Result<T>`. Commit on success, rollback on error.

**When:** Any command that modifies database state.

**Example (from existing code):**
```rust
db.transaction(|tx| {
    item.insert(tx)?;
    EventLog::record(tx, EventType::Created, EntityType::Item, &item.id, ...)?;
    Ok(())
})?;
```

**Why:** Ensures atomic operations. Events are always recorded with their corresponding mutations.

### Pattern 2: Pure Analysis Functions

**What:** Analysis functions take immutable data references, return results. No I/O inside analysis.

**When:** All analysis in `domains/storage/analysis.rs`.

**Example (from existing code):**
```rust
pub fn analyze_redundancy(
    nodes: &[Node],
    volumes: &[Volume],
    datasets: &[Dataset],
    syncs: &[SyncRegime],
) -> RedundancyReport {
    // Pure computation, no database access
}
```

**Why:**
- Testable with mock data
- Composable (call multiple analyses)
- Clear what data is needed
- No hidden state

### Pattern 3: Typed Spec Parsing

**What:** Parse string specs (capacity, noise, bandwidth) into typed values with units.

**When:** Any spec that needs arithmetic or comparison.

**Example (from existing code):**
```rust
// In core/specs.rs
pub struct Capacity {
    pub bytes: u64,
}

pub fn get_capacity(specs: &JsonValue) -> Option<Capacity> {
    // Parse "4TB", "500GB", etc.
}
```

**Why:** Avoids string comparison bugs ("4TB" vs "4000GB"). Enables correct arithmetic.

### Pattern 4: Command-per-File with Shared Types

**What:** Each CLI subcommand lives in its own file. Shared types (Args structs, output formats) defined in `cli/mod.rs`.

**When:** Any new command group.

**Example (existing pattern):**
```rust
// cli/mod.rs
pub enum Commands {
    Item(item::ItemCommands),
    Price(price::PriceCommands),
    Topology(topology::TopologyCommands),  // NEW
    // ...
}

// cli/topology.rs
#[derive(Subcommand)]
pub enum TopologyCommands {
    Create(CreateArgs),
    Fork(ForkArgs),
    Show(ShowArgs),
    Diff(DiffArgs),
}
```

**Why:**
- Clear ownership (one file per command domain)
- Parallel development (different contributors can work on different commands)
- Easy to find code (command name maps to filename)

### Pattern 5: AI-Friendly Output via OutputFormat

**What:** Commands accept `--format` flag (text/json/yaml). Output functions branch on format.

**When:** Any command that produces output.

**Example (from existing code):**
```rust
match format {
    OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&result)?),
    OutputFormat::Yaml => println!("{}", serde_yaml::to_string(&result)?),
    OutputFormat::Text => print_human_readable(&result),
}
```

**Why:** AI agents parse JSON/YAML reliably. Humans prefer styled text.

## Graph Storage: Normalized Tables (Recommended)

### Comparison: Normalized vs JSON Blob

| Approach | Query | Schema Flexibility | Versioning | Recommendation |
|----------|-------|-------------------|------------|----------------|
| **Normalized** | SQL for any field | Fixed schema, migrations | Fork via INSERT with parent_id | **Use this** |
| **JSON Blob** | json_extract(), limited | Fully flexible | Full copy on fork | Not recommended |
| **Hybrid** | Core normalized, extras in JSON | Best of both | Same as normalized | Consider for metadata |

### Why Normalized

1. **Queryability**: "Find all topologies where node X is a Mac mini" is a simple SQL query, not JSON parsing
2. **Referential integrity**: Foreign keys prevent orphaned volumes, broken sync regimes
3. **Efficient forking**: Copy only changed rows, not entire JSON document
4. **Existing pattern**: The codebase already uses normalized tables (items, prices, etc.)

### Recommended Schema

```sql
-- Topologies: versioned roots of the graph
CREATE TABLE topologies (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    parent_id TEXT REFERENCES topologies(id),  -- for forking
    version INTEGER NOT NULL DEFAULT 1,
    tags TEXT NOT NULL DEFAULT '[]',  -- JSON array: ["current"], ["exploring"], ["archived"]
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    metadata TEXT NOT NULL DEFAULT '{}'
);

-- Ensure only one topology has "current" tag
-- (enforced in application code, not SQL constraint)

-- Nodes: equipment instances
CREATE TABLE nodes (
    id TEXT PRIMARY KEY,
    topology_id TEXT NOT NULL REFERENCES topologies(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    node_type TEXT NOT NULL,  -- desktop, nas, server, cloud, external
    location TEXT NOT NULL,
    item_id TEXT REFERENCES items(id),  -- optional link to catalog
    metadata TEXT NOT NULL DEFAULT '{}'
);

-- Volumes: storage attached to nodes
CREATE TABLE volumes (
    id TEXT PRIMARY KEY,
    topology_id TEXT NOT NULL REFERENCES topologies(id) ON DELETE CASCADE,
    node_id TEXT NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    capacity_bytes INTEGER NOT NULL,
    volume_type TEXT NOT NULL,  -- ssd, hdd, raid, cloud
    raid_level TEXT,
    item_id TEXT REFERENCES items(id),  -- optional link to catalog product
    metadata TEXT NOT NULL DEFAULT '{}'
);

-- Datasets: logical data groups with requirements
CREATE TABLE datasets (
    id TEXT PRIMARY KEY,
    topology_id TEXT NOT NULL REFERENCES topologies(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    growth_rate_bytes_per_month INTEGER,
    criticality TEXT NOT NULL,  -- critical, important, normal, archive
    min_copies INTEGER NOT NULL DEFAULT 1,
    min_locations INTEGER NOT NULL DEFAULT 1,
    max_rpo_hours INTEGER,
    metadata TEXT NOT NULL DEFAULT '{}'
);

-- Dataset placements: which datasets live on which volumes
CREATE TABLE dataset_placements (
    id TEXT PRIMARY KEY,
    topology_id TEXT NOT NULL REFERENCES topologies(id) ON DELETE CASCADE,
    dataset_id TEXT NOT NULL REFERENCES datasets(id) ON DELETE CASCADE,
    volume_id TEXT NOT NULL REFERENCES volumes(id) ON DELETE CASCADE,
    is_primary BOOLEAN NOT NULL DEFAULT 0,
    UNIQUE(dataset_id, volume_id)
);

-- Sync regimes: data movement edges
CREATE TABLE sync_regimes (
    id TEXT PRIMARY KEY,
    topology_id TEXT NOT NULL REFERENCES topologies(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    source_volume_id TEXT NOT NULL REFERENCES volumes(id) ON DELETE CASCADE,
    target_volume_id TEXT NOT NULL REFERENCES volumes(id) ON DELETE CASCADE,
    sync_type TEXT NOT NULL,  -- rsync, rclone, zfs, resilio, manual
    direction TEXT NOT NULL DEFAULT 'one-way',  -- one-way, bidirectional
    schedule TEXT,  -- cron expression or "continuous"
    method_item_id TEXT REFERENCES items(id),  -- optional: software/service used
    metadata TEXT NOT NULL DEFAULT '{}'
);

-- Network links: connectivity between nodes
CREATE TABLE links (
    id TEXT PRIMARY KEY,
    topology_id TEXT NOT NULL REFERENCES topologies(id) ON DELETE CASCADE,
    source_node_id TEXT NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    target_node_id TEXT NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    link_type TEXT NOT NULL,  -- lan, wan, internet, usb, thunderbolt
    bandwidth_bytes_per_sec INTEGER,
    latency_ms INTEGER,
    metadata TEXT NOT NULL DEFAULT '{}'
);

-- Indexes for common queries
CREATE INDEX idx_nodes_topology ON nodes(topology_id);
CREATE INDEX idx_volumes_topology ON volumes(topology_id);
CREATE INDEX idx_volumes_node ON volumes(node_id);
CREATE INDEX idx_datasets_topology ON datasets(topology_id);
CREATE INDEX idx_placements_topology ON dataset_placements(topology_id);
CREATE INDEX idx_syncs_topology ON sync_regimes(topology_id);
CREATE INDEX idx_links_topology ON links(topology_id);
```

### Forking a Topology

When user runs `sp topology fork`:

1. Create new topology row with `parent_id` pointing to source
2. Copy all nodes, volumes, datasets, placements, sync_regimes, links
3. Update foreign keys to point to new topology
4. Return new topology ID

This is a shallow copy at the topology level, deep copy of contents.

## Graph Operations in Memory

### When to Use petgraph

For complex graph algorithms (cycle detection, shortest path, transitive closure), build a petgraph from loaded data:

```rust
use petgraph::graph::{DiGraph, NodeIndex};

pub struct TopologyGraph {
    // Physical graph: nodes connected by links
    physical: DiGraph<NodeId, LinkId>,
    // Data flow graph: volumes connected by sync regimes
    data_flow: DiGraph<VolumeId, SyncRegimeId>,
    // Lookups
    node_indices: HashMap<NodeId, NodeIndex>,
    volume_indices: HashMap<VolumeId, NodeIndex>,
}

impl TopologyGraph {
    pub fn from_topology(topo: &Topology) -> Self {
        // Build graphs from loaded data
    }

    pub fn reachable_from(&self, volume_id: &VolumeId) -> Vec<VolumeId> {
        // Use petgraph's Bfs or Dfs
    }

    pub fn widest_path(&self, from: &NodeId, to: &NodeId) -> Option<u64> {
        // Custom algorithm using petgraph traversal
    }
}
```

### When NOT to Use petgraph

For simple queries (list all volumes on a node, find datasets on a volume), use SQL directly. Don't build a graph just to iterate.

## Anti-Patterns to Avoid

### Anti-Pattern 1: Database in Analysis Functions

**What:** Passing `&Database` to analysis functions.

**Why bad:**
- Analysis becomes hard to test
- Hidden I/O in "pure" functions
- Can't parallelize analysis

**Instead:** Load all data first, pass slices to analysis.

```rust
// BAD
pub fn analyze_redundancy(db: &Database, topology_id: &str) -> Result<Report>

// GOOD
pub fn analyze_redundancy(
    nodes: &[Node],
    volumes: &[Volume],
    datasets: &[Dataset],
    placements: &[DatasetPlacement],
    syncs: &[SyncRegime],
) -> RedundancyReport
```

### Anti-Pattern 2: CLI Logic in Models

**What:** Putting CLI-specific code (printing, arg parsing) in `core/models.rs`.

**Why bad:** Models become untestable, coupled to presentation.

**Instead:** Models are pure data + CRUD. CLI formats for display.

### Anti-Pattern 3: God Transaction

**What:** One massive transaction for entire command.

**Why bad:** Long lock times, hard to reason about.

**Instead:** Multiple focused transactions, or one transaction with clear phases.

### Anti-Pattern 4: JSON Blob for Everything

**What:** Storing entire topology as single JSON column.

**Why bad:**
- No foreign keys
- Can't query fields efficiently
- Forking requires full copy
- No type safety from database

**Instead:** Normalized tables with JSON only for truly flexible metadata.

### Anti-Pattern 5: Tight Coupling Between Commands

**What:** `sp topology fork` directly calling logic from `sp decide`.

**Why bad:** Circular dependencies, hard to test in isolation.

**Instead:** Shared logic lives in `core/` or `domains/`. Commands are thin wrappers.

## Build Order (Suggested Phase Structure)

Based on dependencies between components:

### Phase 1: Schema and Core Types

**Build:**
- Migration in `core/db.rs` for new tables
- `core/topology.rs` with Topology struct, CRUD
- `domains/storage/models.rs` updates (already exists, may need adjustments)

**Dependency:** None (foundational)

**Deliverable:** Can store/load topologies via direct database calls.

### Phase 2: Basic Topology Commands

**Build:**
- `cli/topology.rs` with create, show, list
- `cli/node.rs` with add, remove (modifies topology)
- `cli/volume.rs` with add, remove
- `cli/dataset.rs` with add, place

**Dependency:** Phase 1 complete

**Deliverable:** Can create and view topologies via CLI.

### Phase 3: Topology Versioning

**Build:**
- Fork operation in `core/topology.rs`
- Tag operations (add/remove tag, ensure unique "current")
- Diff operation in `core/graph.rs`

**Dependency:** Phase 2 complete (need topologies to fork)

**Deliverable:** Can fork topologies and compare versions.

### Phase 4: Graph Analysis

**Build:**
- `core/graph.rs` for in-memory graph construction
- `domains/storage/topology_analysis.rs`:
  - `analyze_redundancy()`
  - `analyze_rpo()`
  - `simulate_failure()`
  - `analyze_bandwidth()`
  - `project_capacity()`

**Dependency:** Phase 1 (need loaded topology data)

**Deliverable:** Can run analysis on topologies.

### Phase 5: Decision Integration

**Build:**
- Link decisions to topologies (decision.topology_id)
- Topology staleness detection (decision references old version)
- Update `sp prime` to include topology context

**Dependency:** Phases 2, 3, 4 complete

**Deliverable:** Full decision workflow with topology support.

## Output Formatting for AI

### AI-Friendly Output Principles

1. **Structured data for parsing**: JSON/YAML for machine consumption
2. **Semantic sections**: Clear headers so AI can navigate
3. **Include IDs**: Always include entity IDs so AI can reference them
4. **Indicate staleness**: If data is old, say so
5. **Actionable hints**: Suggest next commands

### sp prime Output Structure (Enhanced)

```yaml
# sp prime --format=yaml
session:
  active_decision:
    id: "dec-001"
    purpose: "Replace NAS"
    status: "in_progress"
    open_questions: ["Which drives?"]
    constraints:
      budget_max: 1000
      noise_max_db: 0
      capacity_min_bytes: 8000000000000

  current_topology:
    id: "topo-v3-sata"
    name: "SATA Option"
    parent: "topo-v2"
    tags: ["current", "exploring"]
    summary:
      nodes: 4
      volumes: 6
      datasets: 7
      unmet_requirements: 0

  exploring_topologies:
    - id: "topo-v3-nvme"
      name: "NVMe Option"
      comparison_to_current:
        cost_diff: +50
        speed_diff: "+2600MB/s"

catalog:
  items_count: 67
  prices_freshness: "1 day ago"
  stale_prices: ["samsung-870-evo-4tb"]

suggested_commands:
  - "sp topology show topo-v3-sata"
  - "sp analyze --topology=topo-v3-sata"
  - "sp topology compare topo-v3-sata topo-v3-nvme"
```

## Scalability Considerations

| Concern | At 10 topologies | At 100 topologies | At 1000 topologies |
|---------|------------------|-------------------|---------------------|
| Query speed | Instant | Instant | Add indexes on name, tags |
| Fork speed | <100ms | <100ms | Consider lazy copying |
| Graph loading | <10ms | <10ms | Cache in memory |
| Analysis | <100ms | <100ms | Profile if slow |
| Database size | <1MB | <10MB | Still fine for SQLite |

This is a single-user local CLI. SQLite handles this scale trivially.

## Sources

- [Rust Module System Guide](https://dev.to/ajtech0001/rusts-module-system-explained-a-complete-guide-to-organizing-your-code-3i8i) - Module organization patterns
- [CLI Structure in Rust](https://kbknapp.dev/cli-structure-01/) by Kevin Knapp (clap author) - Command architecture patterns
- [SQLite Recursive CTEs](https://sqlite.org/lang_with.html) - Graph traversal in SQL
- [SQLite as Graph Database](https://jeqo.github.io/notes/2022-05-09-sqlite-as-document-and-graph-db/) - SQLite for graph patterns
- [petgraph documentation](https://docs.rs/petgraph/latest/petgraph/) - Rust graph library
- [Clean Architecture in Rust](https://navy.systems/articles/clean-architecture-and-domain-driven-design-in-rust/) - Layer separation
- [Functional Domain Modeling in Rust](https://xebia.com/blog/functional-domain-modeling-in-rust-part-1/) - Pure function patterns
- [sqlite-es crate](https://docs.rs/sqlite-es) - Event sourcing with SQLite (for versioning patterns)
- Existing codebase: `/Users/morgan/code/storage-planner/src/` (analyzed directly)

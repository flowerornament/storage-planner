# Phase 1: Schema and Core Types - Research

**Researched:** 2026-02-06
**Domain:** SQLite schema design, Rust type modeling, undo/redo event sourcing
**Confidence:** HIGH

## Summary

Phase 1 creates the database foundation for topology modeling in an existing Rust CLI tool. The codebase already has a working SQLite database with tables for items, prices, configurations, decisions, and events, plus a CLI built with clap 4, rusqlite 0.31, and standard Rust serialization libraries. The new work adds 7 topology tables (topologies, nodes, volumes, datasets, placements, links, sync_regimes), redesigns the events table with before/after state for undo/redo, adds migration tracking via PRAGMA user_version, and scaffolds nested CLI commands for the new entity types.

The critical technical challenge is the undo/redo system. The user explicitly requires `sp undo` and `sp redo` commands with multi-level stack behavior in Phase 1. This means every mutating command must record enough state (before + after snapshots) to reverse itself. The command pattern with a sequential event log and undo/redo pointer is the standard approach.

**Primary recommendation:** Use the existing codebase patterns (rusqlite direct SQL, clap derive, anyhow errors) and extend them. Do NOT introduce new crates. Use PRAGMA user_version for migration tracking. Store undo/redo state as before/after JSON snapshots in a redesigned events table with an integer sequence number as the undo/redo pointer.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Datasets are **independent entities** that get "placed" on volumes via a placement junction table -- not children of a single volume. Supports multi-volume replication.
- Sync regimes operate at **dataset level** (per-dataset placement pair), not volume-to-volume. e.g., "photos on NAS -> photos on backup drive, daily." Enables precise RPO analysis per dataset.
- Nodes carry a **full hardware profile**: name, role, physical location, available bays, interface types, power draw. Not just name+role.
- Volumes are **rich**: capacity (raw + usable), filesystem type, RAID level/pool type (ZFS mirror, RAID5, etc.), AND a foreign key to catalog items. A volume can reference the actual drive being considered for purchase.
- Links model **full network characteristics**: bandwidth, connection type (LAN/WAN/USB/Thunderbolt), latency, metered/unmetered, cost-per-GB. Supports bandwidth cost analysis in Phase 6.
- **Strictly typed columns** throughout -- no JSON metadata blobs. Schema is self-documenting. Changes require migrations.
- Events store both **structured JSON payload** and a **human-readable summary** -- best of both worlds.
- Events track **source**: user, agent, import, or migration -- important for AI session continuity.
- Events store **before/after state** for full undo/redo capability.
- Event schema should be **redesigned from scratch** (not extending existing events table).
- **Full undo/redo in Phase 1**: `sp undo` and `sp redo` commands ship in this phase.
- Multiple levels of undo -- can undo repeatedly, redo after undo, standard undo/redo stack behavior.
- Fresh start OK -- existing .sp/decisions.db data does not need preservation.
- SQLite remains the database.
- Typically few topologies (2-5): one "current" plus a couple of forks/alternatives.

### Claude's Discretion
- Dataset properties (size, growth rate, min_copies, min_locations, max_rpo) -- design based on what Phase 4 analysis functions need
- Placement table properties (pure junction vs role/priority fields) -- based on sync regime and analysis needs
- Event logging granularity (major actions only vs all mutations) -- based on what's useful for decision tracking
- All naming/identity decisions: how users reference entities in CLI commands, name scoping, active/default topology, volume reference style, naming conventions, ID scheme consistency
- Migration tracking method: PRAGMA user_version or migration table
- Schema organization: unified vs versioned steps

### Deferred Ideas (OUT OF SCOPE)
- PostgreSQL as an alternative backend -- user mentioned openness to it, but SQLite fits the local CLI use case. Revisit only if scaling needs change.
</user_constraints>

## Existing Codebase Analysis

### What Already Exists (HIGH confidence -- read from source)

**Dependencies (Cargo.toml):**
| Crate | Version | Purpose |
|-------|---------|---------|
| rusqlite | 0.31 (bundled, serde_json) | SQLite database |
| clap | 4 (derive, env) | CLI framework |
| serde | 1 (derive) | Serialization |
| serde_json | 1 | JSON handling |
| serde_yaml | 0.9 | YAML output |
| chrono | 0.4 (serde) | Timestamps |
| uuid | 1 (v4, serde) | ID generation |
| anyhow | 1 | Error handling |
| console | 0.15 | Terminal output/styling |
| camino | 1 (serde1) | UTF-8 paths |
| fs-err | 3 | Filesystem ops |
| ureq | 2 (json) | HTTP client |
| xshell | 0.2 | Shell commands |

**Database layer (src/core/db.rs):**
- `Database` struct wrapping `rusqlite::Connection`
- `open(path)`, `open_memory()`, `transaction()`, `migrate()`, `is_initialized()`, `stats()`
- PRAGMAs: `foreign_keys = ON`, `journal_mode = WAL`, `synchronous = NORMAL`
- Schema is a single `const SCHEMA: &str` applied with `CREATE TABLE IF NOT EXISTS`
- No migration versioning -- just idempotent creation

**Current schema tables:**
- `items` -- purchasable products (TEXT id PK, name, category, brand, specs JSON, tags JSON, metadata JSON)
- `prices` -- price observations (TEXT id PK, item_id FK, source, price, condition, observed_at)
- `configurations` -- named item compositions (TEXT id PK, items JSON array, domain_data JSON)
- `decisions` -- decision sessions (TEXT id PK, purpose, status, options JSON, chosen_option)
- `events` -- audit log (TEXT id PK, event_type, entity_type, entity_id, payload JSON, timestamp, actor)
- `items_fts` -- FTS5 virtual table for item search

**Established patterns:**
- IDs: UUID v4 strings for auto-generated IDs, slug-style strings for user-provided IDs (items)
- Model structs: `#[derive(Debug, Clone, Serialize, Deserialize)]` with `insert(&self, tx)` and `from_row(row)` methods
- Event logging: `EventLog::record(tx, event_type, entity_type, entity_id, payload, actor)` within transactions
- CLI structure: `Cli` with `Commands` enum, each command dispatches to module function
- Output: `OutputFormat` enum (Text/Json/Yaml) passed through, match on format for display
- Error handling: `anyhow::Result<()>` throughout, `bail!()` for errors
- Database check: `if !db_path.exists() { bail!("...Run sp init first.") }`
- Actor: `current_actor()` reads `$USER` env var

**Existing domain models (src/domains/storage/models.rs):**
These are **in-memory only** Rust structs, not yet backed by database tables:
- `Node { id, name, node_type: NodeType, location, volumes: Vec<String> }`
- `Volume { id, name, node_id, item_id, capacity_bytes, raid_level, datasets: Vec<String> }`
- `Dataset { id, name, size_bytes, growth_rate, criticality, rpo_hours, rto_hours }`
- `SyncRegime { id, name, source_volume, target_volume, sync_type, schedule, datasets: Vec<String> }`

These are **insufficient** for the new requirements (lack hardware profiles, no usable capacity, no placements table, etc.) but show the team's initial thinking.

**Existing analysis (src/domains/storage/analysis.rs):**
- `analyze_redundancy()`, `analyze_capacity()`, `analyze_rpo_rto()` -- all operate on in-memory structs
- These will need to be updated in Phase 4 to query from database

### What Must Change

1. **Schema**: Add 7 new tables, redesign events table, add PRAGMA user_version tracking
2. **Database layer**: Add migration versioning support to `Database::migrate()`
3. **Models**: New Rust structs for all topology entities with `insert`/`from_row`/`update`/`delete`
4. **Events**: Redesign from scratch with before/after state, human summary, source tracking
5. **CLI**: Add nested subcommands (topology, node, volume, dataset, link, sync, undo, redo)
6. **Undo/Redo**: New system with command-pattern event log

## Standard Stack

### Core (already in use -- no new crates needed)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| rusqlite | 0.31 | SQLite database | Already in use, bundled SQLite |
| clap | 4.5 | CLI framework (derive) | Already in use, nested subcommands work |
| serde/serde_json | 1 | Serialization | Already in use for JSON payloads |
| chrono | 0.4 | Timestamps | Already in use |
| uuid | 1 | ID generation | Already in use for v4 UUIDs |
| anyhow | 1 | Error handling | Already in use |
| console | 0.15 | Terminal styling | Already in use |

### Considered but NOT recommended
| Library | Why Not |
|---------|---------|
| rusqlite_migration | Adds dependency for something achievable with 20 lines of PRAGMA user_version code. The existing codebase prefers minimal dependencies. |
| diesel/sqlx | Way too heavy for this use case. Raw rusqlite is the established pattern. |
| sea-query | Schema builder would fight the explicit SQL approach already in use |

## Architecture Patterns

### Recommended Project Structure Changes

```
src/
  core/
    db.rs             # MODIFY: Add migration versioning via PRAGMA user_version
    events.rs         # REWRITE: New event system with undo/redo support
    models.rs         # KEEP: Existing Item, Price, Configuration, Decision
    specs.rs          # KEEP: Capacity, Speed, NoiseLevel parsers
    mod.rs            # KEEP
  cli/
    mod.rs            # MODIFY: Add Topology, Undo, Redo commands to Commands enum
    topology.rs       # NEW: sp topology {create,list,show,set-active}
    node.rs           # NEW: sp node {add,remove,list} (PLACEHOLDER - Phase 2 commands)
    volume.rs         # NEW: sp volume {add,remove,list} (PLACEHOLDER - Phase 2 commands)
    dataset.rs        # NEW: sp dataset {add,remove,list} (PLACEHOLDER - Phase 2 commands)
    link.rs           # NEW: sp link {add,remove,list} (PLACEHOLDER - Phase 2 commands)
    sync_regime.rs    # NEW: sp sync {add,remove,list} (PLACEHOLDER - Phase 2 commands)
    undo.rs           # NEW: sp undo
    redo.rs           # NEW: sp redo
    events.rs         # MODIFY: Update for new event schema
    init.rs           # MODIFY: Use new migration system
    ... existing files kept ...
  domains/
    storage/
      models.rs       # REWRITE: New DB-backed topology models
      analysis.rs     # KEEP for now (Phase 4 rewrites)
      mod.rs          # KEEP
```

### Pattern 1: Migration via PRAGMA user_version

**What:** Use SQLite's built-in `PRAGMA user_version` integer to track schema version. Each migration is a numbered step. On startup, read current version, apply any unapplied migrations sequentially.

**Why:** The existing code uses `CREATE TABLE IF NOT EXISTS` which provides no versioning. PRAGMA user_version is simpler than a migration table, zero-overhead, and is the recommended lightweight approach for embedded SQLite applications.

**Example:**
```rust
// In db.rs
const CURRENT_VERSION: i32 = 1; // Bump with each migration

struct Migration {
    version: i32,
    up: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        up: "
            -- Phase 1: Topology schema + redesigned events
            CREATE TABLE topologies (...);
            CREATE TABLE nodes (...);
            ...
            PRAGMA user_version = 1;
        ",
    },
];

impl Database {
    pub fn migrate(&mut self) -> Result<()> {
        let current: i32 = self.conn.pragma_query_value(
            None, "user_version", |row| row.get(0)
        )?;

        for migration in MIGRATIONS {
            if migration.version > current {
                self.conn.execute_batch(migration.up)?;
            }
        }
        Ok(())
    }
}
```

**Recommendation for schema organization:** Single migration for Phase 1 (version 1) that creates ALL topology tables plus the redesigned events table. Future phases increment the version number. This is simpler than per-table migrations and aligns with the "fresh start OK" constraint.

### Pattern 2: Undo/Redo via Event Log with Sequence Pointer

**What:** Every mutating command records an event with: (a) the SQL/action to undo, (b) the SQL/action to redo, stored as before/after entity state. A global sequence counter tracks position. Undo pops the stack backward; redo moves forward.

**Why:** The user requires multi-level undo/redo with `sp undo` and `sp redo` commands. The command pattern with before/after state in the events table is the cleanest approach for a CLI where each invocation is a separate process (no in-memory stack persists between commands).

**Design:**
```
events table:
  id (TEXT PK)
  sequence (INTEGER UNIQUE) -- monotonically increasing, THE undo/redo pointer
  event_type (TEXT)         -- 'topology.created', 'node.added', etc.
  entity_type (TEXT)        -- 'topology', 'node', 'volume', etc.
  entity_id (TEXT)
  summary (TEXT)            -- human-readable: "Added node 'mac-mini' to topology 'current'"
  before_state (TEXT)       -- JSON snapshot of entity before change (NULL for creates)
  after_state (TEXT)        -- JSON snapshot of entity after change (NULL for deletes)
  source (TEXT)             -- 'user', 'agent', 'import', 'migration'
  actor (TEXT)              -- who did it
  timestamp (TEXT)

undo_pointer table (single row):
  current_sequence (INTEGER) -- points to last "active" event; events after this are "undone"
```

**Undo algorithm:**
1. Read `current_sequence` from undo_pointer
2. Find event at that sequence number
3. Apply `before_state` to restore entity (or DELETE for creates, INSERT for deletes)
4. Decrement `current_sequence`

**Redo algorithm:**
1. Read `current_sequence` from undo_pointer
2. Find event at `current_sequence + 1`
3. Apply `after_state` (or INSERT for creates, DELETE for deletes)
4. Increment `current_sequence`

**New action while undone:** When a new mutation happens while `current_sequence < max(sequence)`, delete all events with `sequence > current_sequence` (the "redo tail"), then insert new event. Standard undo/redo stack behavior.

### Pattern 3: Naming and Identity Scheme

**Recommendation (Claude's discretion area):**

| Entity | ID Strategy | User Reference | Rationale |
|--------|------------|----------------|-----------|
| Topologies | UUID v4 (TEXT) | User-provided slug name (UNIQUE within non-archived) | Few topologies, names are natural. `sp topology show current-setup` |
| Nodes | UUID v4 (TEXT) | User-provided slug name scoped to topology | `sp node add mac-mini --topology=current` |
| Volumes | UUID v4 (TEXT) | User-provided slug name scoped to node | `sp volume add main-ssd --node=mac-mini` |
| Datasets | UUID v4 (TEXT) | User-provided slug name scoped to topology | `sp dataset add photos --topology=current` |
| Placements | UUID v4 (TEXT) | Referenced by dataset+volume pair | Junction table, not directly named |
| Links | UUID v4 (TEXT) | Referenced by source_node+target_node | `sp link add --from=mac-mini --to=nas` |
| Sync regimes | UUID v4 (TEXT) | User-provided slug name scoped to topology | `sp sync add daily-backup --topology=current` |

**Active topology:** Add an `is_active` boolean column (only one can be TRUE, enforced in application code). Commands default to active topology when `--topology` is not specified. This mirrors the existing `is_current` pattern on configurations.

**Naming convention:** Slug-style (lowercase, hyphens) consistent with existing item IDs like `samsung-870-evo-4tb`. Names must be unique within their scope (topology for nodes/datasets/syncs, node for volumes).

### Pattern 4: Strictly Typed Columns (No JSON Blobs)

**What:** Per the locked decision, ALL columns are strongly typed SQL columns. No `metadata TEXT DEFAULT '{}'` patterns. This is a departure from the existing codebase which uses JSON blobs extensively (items.specs, items.tags, items.metadata, configurations.items, configurations.domain_data).

**Impact on existing tables:** The locked decision says "strictly typed columns throughout" and "schema is self-documenting." For Phase 1, this applies to the NEW topology tables. The existing tables (items, prices, configurations, decisions) keep their current schema -- they are not being modified in Phase 1.

**For new tables, example:**
```sql
-- YES: strictly typed
CREATE TABLE nodes (
    id TEXT PRIMARY KEY,
    topology_id TEXT NOT NULL REFERENCES topologies(id),
    name TEXT NOT NULL,
    role TEXT NOT NULL,           -- 'desktop', 'nas', 'server', 'cloud', 'external'
    location TEXT NOT NULL,
    available_bays INTEGER,
    interface_types TEXT NOT NULL, -- comma-separated: 'usb3,thunderbolt4,ethernet'
    power_draw_watts REAL,
    ...
);

-- NO: JSON blob
CREATE TABLE nodes (
    ...
    hardware_profile TEXT DEFAULT '{}',  -- JSON blob
);
```

**Interface types consideration:** The "no JSON" constraint means list-valued fields like `interface_types` need a design choice. Options: (a) comma-separated TEXT column, (b) separate junction table `node_interfaces(node_id, interface_type)`. For simplicity with few values, comma-separated TEXT is reasonable. For querying, a junction table is better. Recommendation: use comma-separated TEXT for simple lists (interface_types) where we only need to display them, and junction tables where we need to query/filter.

### Anti-Patterns to Avoid

- **JSON metadata blobs on new tables**: The user explicitly banned this. Every field must be a real column.
- **Extending existing events table**: The user explicitly said "redesign from scratch."
- **Preserving existing data**: Fresh start is OK. Don't add complexity for backwards compatibility.
- **Over-engineering Phase 1**: This phase is SCHEMA + TYPES + MIGRATION + UNDO/REDO + CLI HELP. The actual CRUD commands for nodes/volumes/etc are Phase 2. Phase 1 just needs the tables, types, migration infrastructure, event/undo system, and CLI command stubs that show help.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| UUID generation | Custom ID generator | `uuid::Uuid::new_v4()` | Already in codebase, battle-tested |
| Timestamp handling | Custom formatting | `chrono::Utc::now().to_rfc3339()` | Already in codebase |
| CLI argument parsing | Manual arg parsing | `clap::Parser` derive macros | Already in codebase |
| SQL injection prevention | String interpolation | `rusqlite::params![]` | Already in codebase |
| Transaction management | Manual commit/rollback | `Database::transaction()` | Already in codebase |

## Common Pitfalls

### Pitfall 1: Foreign Key Cascade Design
**What goes wrong:** Deleting a topology leaves orphaned nodes/volumes/datasets. Or CASCADE deletes destroy data the user wanted to keep.
**Why it happens:** Topology tables form a deep hierarchy: topology -> nodes -> volumes, topology -> datasets -> placements, etc.
**How to avoid:** Use `ON DELETE CASCADE` for the topology->child relationships. When a topology is deleted, all its children go with it. This is correct because entities are scoped to topologies.
**Warning signs:** Orphan rows in child tables after parent deletion.

### Pitfall 2: Undo/Redo for Multi-Row Operations
**What goes wrong:** A single user action (e.g., "delete node") cascades to delete volumes and placements, but undo only restores the node row.
**Why it happens:** The event records only the top-level entity change, not the cascaded changes.
**How to avoid:** Record the COMPLETE before/after state. For a node deletion, the event's `before_state` must include the node AND all its volumes AND all their placements. The undo operation must restore everything. Alternatively, group cascading changes into a single event with a compound before/after snapshot.
**Warning signs:** Undo restores parent but not children.

### Pitfall 3: Active Topology Race Condition
**What goes wrong:** Two commands run concurrently, both try to set different topologies as active.
**Why it happens:** No database-level constraint on `is_active`.
**How to avoid:** Use a transaction that first clears all `is_active` flags, then sets the target one. SQLite's WAL mode with exclusive write lock handles this naturally for a single-user CLI. The real protection is the transaction.

### Pitfall 4: Redo Stack Invalidation
**What goes wrong:** User undoes 3 actions, then does a NEW action, but the redo stack still contains the old future events.
**Why it happens:** Standard undo/redo stacks require clearing the redo stack on new actions.
**How to avoid:** When inserting a new event while `current_sequence < max(sequence)`, DELETE all events with `sequence > current_sequence` first. This is standard undo/redo behavior.

### Pitfall 5: Schema Migration with Existing Databases
**What goes wrong:** `PRAGMA user_version` starts at 0 for new databases AND for existing databases that never set it. Migration code can't distinguish "brand new" from "existing v0."
**How to avoid:** Since "fresh start OK" is a locked decision, the simplest approach is: if `user_version = 0` AND tables exist (like `items`), drop everything and start fresh. Or: just always run the full migration from scratch. The user said existing data doesn't need preservation.

### Pitfall 6: Event Before/After State Size
**What goes wrong:** Storing full entity JSON snapshots in every event bloats the database.
**Why it happens:** Each event stores complete before AND after state.
**How to avoid:** This is acceptable for this use case. Topologies have few entities (2-5 topologies, maybe 5-10 nodes, 10-20 volumes). The data volume is tiny. Don't optimize prematurely. If needed later, store diffs instead of full snapshots.

## Code Examples

### Entity Model Pattern (following existing codebase conventions)

```rust
// Source: Derived from existing src/core/models.rs patterns
use chrono::{DateTime, Utc};
use rusqlite::{params, Row, Transaction};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Topology {
    pub id: String,
    pub name: String,
    pub description: String,
    pub parent_id: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Topology {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            description: description.into(),
            parent_id: None,
            is_active: false,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn insert(&self, tx: &Transaction) -> rusqlite::Result<()> {
        tx.execute(
            "INSERT INTO topologies (id, name, description, parent_id, is_active, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                self.id,
                self.name,
                self.description,
                self.parent_id,
                self.is_active as i32,
                self.created_at.to_rfc3339(),
                self.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let created_str: String = row.get("created_at")?;
        let updated_str: String = row.get("updated_at")?;
        Ok(Self {
            id: row.get("id")?,
            name: row.get("name")?,
            description: row.get("description")?,
            parent_id: row.get("parent_id")?,
            is_active: row.get::<_, i32>("is_active")? != 0,
            created_at: DateTime::parse_from_rfc3339(&created_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_at: DateTime::parse_from_rfc3339(&updated_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        })
    }
}
```

### New Event Model Pattern

```rust
// Source: Redesigned from existing src/core/events.rs + CONTEXT.md requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub sequence: i64,             // monotonically increasing, undo/redo pointer
    pub event_type: String,        // 'topology.created', 'node.added', etc.
    pub entity_type: String,       // 'topology', 'node', 'volume', etc.
    pub entity_id: String,
    pub summary: String,           // human-readable description
    pub before_state: Option<String>, // JSON snapshot before change
    pub after_state: Option<String>,  // JSON snapshot after change
    pub source: EventSource,       // user, agent, import, migration
    pub actor: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventSource {
    User,
    Agent,
    Import,
    Migration,
}
```

### Undo Implementation Pattern

```rust
// Source: Command pattern for CLI undo/redo
pub fn undo(db: &mut Database) -> Result<()> {
    db.transaction(|tx| {
        // Get current undo pointer
        let current_seq: i64 = tx.query_row(
            "SELECT current_sequence FROM undo_pointer", [], |r| r.get(0)
        )?;

        if current_seq < 1 {
            bail!("Nothing to undo");
        }

        // Get event at current sequence
        let event = get_event_by_sequence(tx, current_seq)?;

        // Apply before_state to restore previous state
        match event.event_type.as_str() {
            t if t.ends_with(".created") => {
                // Undo create = delete the entity
                delete_entity(tx, &event.entity_type, &event.entity_id)?;
            }
            t if t.ends_with(".deleted") => {
                // Undo delete = re-insert from before_state
                let state = event.before_state.as_ref()
                    .ok_or_else(|| anyhow!("Missing before_state for undo"))?;
                restore_entity(tx, &event.entity_type, state)?;
            }
            t if t.ends_with(".updated") => {
                // Undo update = restore before_state
                let state = event.before_state.as_ref()
                    .ok_or_else(|| anyhow!("Missing before_state for undo"))?;
                update_entity_from_state(tx, &event.entity_type, &event.entity_id, state)?;
            }
            _ => bail!("Unknown event type for undo: {}", event.event_type),
        }

        // Decrement pointer
        tx.execute(
            "UPDATE undo_pointer SET current_sequence = ?1",
            [current_seq - 1],
        )?;

        println!("Undone: {}", event.summary);
        Ok(())
    })
}
```

### CLI Nested Subcommand Pattern

```rust
// Source: Derived from existing src/cli/mod.rs pattern
#[derive(Subcommand)]
pub enum Commands {
    // ... existing commands ...

    /// Manage topologies
    #[command(subcommand)]
    Topology(topology::TopologyCommands),

    /// Manage nodes within a topology
    #[command(subcommand)]
    Node(node::NodeCommands),

    /// Manage volumes within a topology
    #[command(subcommand)]
    Volume(volume::VolumeCommands),

    /// Manage datasets within a topology
    #[command(subcommand)]
    Dataset(dataset::DatasetCommands),

    /// Manage network links between nodes
    #[command(subcommand)]
    Link(link::LinkCommands),

    /// Manage sync regimes
    #[command(subcommand)]
    Sync(sync_regime::SyncCommands),  // Note: 'sync' name conflicts with existing sync.rs

    /// Undo the last action
    Undo(undo::UndoArgs),

    /// Redo the last undone action
    Redo(redo::RedoArgs),
}
```

### Migration Schema Pattern

```sql
-- Phase 1 migration (version 1)
-- Fresh start: drop old tables, create new schema

-- Topology tables
CREATE TABLE topologies (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL DEFAULT '',
    parent_id TEXT REFERENCES topologies(id),
    is_active INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE nodes (
    id TEXT PRIMARY KEY,
    topology_id TEXT NOT NULL REFERENCES topologies(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    role TEXT NOT NULL,  -- 'desktop', 'nas', 'server', 'cloud', 'external'
    location TEXT NOT NULL DEFAULT '',
    available_bays INTEGER,
    interface_types TEXT NOT NULL DEFAULT '',  -- comma-separated
    power_draw_watts REAL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(topology_id, name)
);

CREATE TABLE volumes (
    id TEXT PRIMARY KEY,
    topology_id TEXT NOT NULL REFERENCES topologies(id) ON DELETE CASCADE,
    node_id TEXT NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    capacity_bytes INTEGER NOT NULL,
    usable_bytes INTEGER,
    filesystem TEXT,          -- 'apfs', 'zfs', 'ext4', 'ntfs', 'btrfs'
    raid_level TEXT,          -- 'mirror', 'raidz1', 'raidz2', 'raid5', 'raid6', 'stripe', 'single'
    pool_type TEXT,           -- 'zfs', 'mdraid', 'lvm', etc.
    item_id TEXT REFERENCES items(id),  -- FK to catalog
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(topology_id, node_id, name)
);

CREATE TABLE datasets (
    id TEXT PRIMARY KEY,
    topology_id TEXT NOT NULL REFERENCES topologies(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    growth_rate_bytes_month REAL,
    criticality TEXT NOT NULL DEFAULT 'normal', -- 'critical', 'important', 'normal', 'archive'
    min_copies INTEGER NOT NULL DEFAULT 1,
    min_locations INTEGER NOT NULL DEFAULT 1,
    max_rpo_hours INTEGER,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(topology_id, name)
);

CREATE TABLE placements (
    id TEXT PRIMARY KEY,
    topology_id TEXT NOT NULL REFERENCES topologies(id) ON DELETE CASCADE,
    dataset_id TEXT NOT NULL REFERENCES datasets(id) ON DELETE CASCADE,
    volume_id TEXT NOT NULL REFERENCES volumes(id) ON DELETE CASCADE,
    role TEXT NOT NULL DEFAULT 'primary',  -- 'primary', 'replica', 'backup', 'cache'
    priority INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(dataset_id, volume_id)
);

CREATE TABLE links (
    id TEXT PRIMARY KEY,
    topology_id TEXT NOT NULL REFERENCES topologies(id) ON DELETE CASCADE,
    source_node_id TEXT NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    target_node_id TEXT NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    bandwidth_bytes_sec INTEGER,
    connection_type TEXT NOT NULL,  -- 'lan', 'wan', 'usb', 'thunderbolt', 'direct'
    latency_ms REAL,
    is_metered INTEGER NOT NULL DEFAULT 0,
    cost_per_gb_cents INTEGER,       -- stored as cents to avoid float issues
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(topology_id, source_node_id, target_node_id)
);

CREATE TABLE sync_regimes (
    id TEXT PRIMARY KEY,
    topology_id TEXT NOT NULL REFERENCES topologies(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    dataset_id TEXT NOT NULL REFERENCES datasets(id) ON DELETE CASCADE,
    source_volume_id TEXT NOT NULL REFERENCES volumes(id) ON DELETE CASCADE,
    target_volume_id TEXT NOT NULL REFERENCES volumes(id) ON DELETE CASCADE,
    sync_type TEXT NOT NULL,    -- 'rsync', 'rclone', 'zfs_send', 'time_machine', 'resilio', 'manual'
    schedule TEXT,              -- cron expression or 'continuous'
    direction TEXT NOT NULL DEFAULT 'push',  -- 'push', 'pull', 'bidirectional'
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(topology_id, name)
);

-- Redesigned events table
CREATE TABLE events (
    id TEXT PRIMARY KEY,
    sequence INTEGER NOT NULL UNIQUE,
    event_type TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    summary TEXT NOT NULL,
    before_state TEXT,          -- JSON, NULL for creates
    after_state TEXT,           -- JSON, NULL for deletes
    source TEXT NOT NULL DEFAULT 'user',
    actor TEXT NOT NULL DEFAULT 'unknown',
    timestamp TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_events_sequence ON events(sequence);
CREATE INDEX idx_events_entity ON events(entity_type, entity_id);
CREATE INDEX idx_events_timestamp ON events(timestamp);

-- Undo pointer (single row)
CREATE TABLE undo_pointer (
    id INTEGER PRIMARY KEY CHECK (id = 1),  -- ensures single row
    current_sequence INTEGER NOT NULL DEFAULT 0
);
INSERT INTO undo_pointer (id, current_sequence) VALUES (1, 0);

-- Indexes for topology tables
CREATE INDEX idx_nodes_topology ON nodes(topology_id);
CREATE INDEX idx_volumes_topology ON volumes(topology_id);
CREATE INDEX idx_volumes_node ON volumes(node_id);
CREATE INDEX idx_datasets_topology ON datasets(topology_id);
CREATE INDEX idx_placements_dataset ON placements(dataset_id);
CREATE INDEX idx_placements_volume ON placements(volume_id);
CREATE INDEX idx_links_topology ON links(topology_id);
CREATE INDEX idx_sync_regimes_topology ON sync_regimes(topology_id);
CREATE INDEX idx_sync_regimes_dataset ON sync_regimes(dataset_id);
```

## Schema Design Recommendations (Claude's Discretion Areas)

### Dataset Properties
Based on Phase 4 analysis needs (redundancy checking, RPO compliance, capacity projection):
- `size_bytes` (INTEGER) -- for capacity analysis
- `growth_rate_bytes_month` (REAL) -- for capacity projection
- `criticality` (TEXT enum) -- for priority-based analysis
- `min_copies` (INTEGER, default 1) -- for redundancy checking
- `min_locations` (INTEGER, default 1) -- for geographic redundancy
- `max_rpo_hours` (INTEGER, nullable) -- for RPO compliance checking

### Placement Table Properties
Beyond a pure junction table, placements should carry:
- `role` (TEXT: 'primary', 'replica', 'backup', 'cache') -- distinguishes the purpose of each placement
- `priority` (INTEGER) -- for sync ordering and restore priority

Rationale: Phase 4 analysis needs to know which placement is the "source of truth" vs a backup copy. Phase 5 decision comparisons need to show placement roles.

### Event Logging Granularity
Recommendation: Log ALL topology mutations (not just "major" ones). Rationale:
- Every mutation needs an event for undo/redo to work
- The undo/redo system IS the event log
- With few topologies and entities, volume is not a concern
- Better to have too much history than too little

### CLI Name Conflict: sync
The existing `src/cli/sync.rs` handles `sp sync` (YAML export). The new sync regimes would also want `sp sync`. Options:
- Rename new command to `sp sync-regime` (verbose but clear)
- Rename existing export to `sp export` (breaking change but better name)
- Use `sp regime` for sync regimes (shorter)

Recommendation: Rename existing `sp sync` to `sp export` since "fresh start OK" means we're not preserving backwards compatibility. Then `sp sync` can be the sync regime command. This is cleaner long-term.

## State of the Art

| Old Approach (current codebase) | New Approach (Phase 1) | Impact |
|--------------------------------|------------------------|--------|
| `CREATE TABLE IF NOT EXISTS` (no versioning) | PRAGMA user_version migrations | Enables schema evolution across 6 phases |
| Events: append-only audit log, no undo | Events: before/after state, undo/redo pointer | Full undo/redo capability |
| JSON blobs for flexible fields (specs, metadata) | Strictly typed columns for new tables | Self-documenting schema, queryable |
| Storage models in memory only (domains/storage/) | Database-backed topology models | Persistent topology state |
| Flat CLI commands | Nested subcommands (topology, node, etc.) | Better organization for many entity types |

## Open Questions

1. **What happens to existing tables during migration?**
   - We know: Fresh start is OK, no need to preserve data
   - Unclear: Should we DROP and recreate existing tables (items, prices, configs, decisions) too, or leave them as-is and only add new tables?
   - Recommendation: Keep existing tables, only add new ones + redesigned events. The existing tables still serve the catalog/pricing functionality. The `items` table is referenced by `volumes.item_id` FK.

2. **Sync regime per dataset-placement-pair vs per dataset**
   - We know: Context says "per-dataset placement pair"
   - The schema above models it as sync_regime having `dataset_id + source_volume_id + target_volume_id`
   - This means one sync regime record per dataset-per-volume-pair
   - If 5 datasets all sync from NAS to backup via the same rsync job, that's 5 records
   - Alternative: sync regimes could cover multiple datasets (many-to-many via junction)
   - Recommendation: Keep one-to-one (dataset per sync regime row) for simplicity and precise RPO analysis. A user who wants "same schedule for all" just creates them with the same schedule string.

3. **Undo across existing vs new commands**
   - We know: Phase 1 introduces undo/redo
   - Unclear: Should existing commands (sp item add, sp price add) also gain undo capability, or only new topology commands?
   - Recommendation: Only new topology commands get undo in Phase 1. Retrofitting existing commands is a separate concern. The event schema supports it, but the implementation effort is separate.

## Sources

### Primary (HIGH confidence)
- Existing codebase: `src/core/db.rs`, `src/core/models.rs`, `src/core/events.rs`, `src/cli/mod.rs` -- read directly
- Cargo.toml and Cargo.lock -- exact versions verified
- CONTEXT.md -- user decisions
- ROADMAP.md, REQUIREMENTS.md -- phase scope and requirements

### Secondary (MEDIUM confidence)
- [SQLite Automatic Undo/Redo](https://www.sqlite.org/undoredo.html) -- official SQLite documentation on trigger-based undo/redo
- [rusqlite_migration crate](https://docs.rs/rusqlite_migration/latest/rusqlite_migration/) -- reference for PRAGMA user_version pattern (not using the crate itself)
- [Command Pattern for Undo/Redo](https://gernotklingler.com/blog/implementing-undoredo-with-the-command-pattern/) -- general pattern reference

### Tertiary (LOW confidence)
- [Event Sourcing with Undo/Redo](https://ericjinks.com/blog/2025/event-sourcing/) -- blog post on event sourcing patterns

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- directly read from Cargo.toml and source files, no new libraries needed
- Architecture: HIGH -- patterns derived from existing codebase conventions, straightforward extensions
- Schema design: HIGH -- requirements are well-specified in CONTEXT.md, standard SQLite patterns
- Undo/redo: MEDIUM -- the pattern is well-understood but implementation details (multi-entity undo, cascade handling) need care during execution
- Pitfalls: MEDIUM -- based on experience with similar systems, not all edge cases may be identified

**Research date:** 2026-02-06
**Valid until:** 2026-03-06 (stable domain, no external dependency changes expected)

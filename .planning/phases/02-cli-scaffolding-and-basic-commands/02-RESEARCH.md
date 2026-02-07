# Phase 2: CLI Scaffolding and Basic Commands - Research

**Researched:** 2026-02-07
**Domain:** Rust CLI CRUD implementation, entity resolution, capacity parsing, output formatting
**Confidence:** HIGH

## Summary

Phase 2 transforms placeholder CLI commands into working CRUD operations for all 7 topology entity types (nodes, volumes, datasets, placements, links, sync regimes) plus enhancements to the existing topology commands. The foundation is solid: Phase 1 delivered a working database with all 9 tables, model structs with `new`/`insert`/`from_row`/`to_json` methods, a proven undo/redo event system, and working topology CRUD that demonstrates the exact pattern each new command must follow. No new libraries are needed.

The primary technical challenges are: (1) building a shared entity resolver that looks up entities by name first, then UUID prefix, with proper ambiguity handling, (2) implementing capacity/bandwidth parsing for CLI input using the existing `specs.rs` module, (3) designing cascade-delete with event logging that preserves undo capability, and (4) ensuring all commands pass `&mut Database` and `OutputFormat` through consistently.

The topology CRUD implementation in `src/cli/topology.rs` is the template. Each new entity command follows the identical transaction-event pattern: open transaction, perform mutation, call `record_event()` with before/after JSON state, commit, display result. The 5 placeholder command files already define arg structures that just need `--topology` override added and their `run()` functions connected to database operations.

**Primary recommendation:** Follow the topology.rs pattern exactly for each entity. Extract a shared `resolve` module for name-or-ID entity lookup. Use the existing `Capacity::parse()` from `specs.rs` for size inputs. No new crates needed.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Entities referenced by **name or ID** everywhere -- try name lookup first, fall back to UUID prefix match
- All entity references resolve the same way -- parent refs (--node, --dataset) use the same name-or-ID resolver
- Names are **slug-like only**: alphanumeric, hyphens, underscores. No spaces, no quoting needed in shell
- Volumes disambiguated via `--node` flag when names collide across nodes (not path syntax)
- Error if a volume name is ambiguous without `--node`, listing the options
- All commands default to active topology, with `--topology=name` override to target a different one without switching
- Deleting an entity with dependents: **warn then cascade** -- output lists what will be deleted, but proceeds without interactive prompt
- Undo is available for recovery
- Nodes, volumes, datasets: support update-in-place (partial updates, only change specified fields)
- Placements, links, sync regimes: immutable -- delete and recreate to change
- `sp topology show` displays **summary by default** (name, description, active, counts), with `--tree` or `--verbose` flag for full hierarchy
- `sp node show` displays **node properties + its volumes** inline
- Agent-friendliness is a first-class concern -- `--format=json` on all commands, no interactive prompts, consistent resolver logic
- The `--topology` override avoids unnecessary `set-active` calls when agents work across topologies
- Slug-like naming chosen specifically so shell quoting is never needed

### Claude's Discretion
- Placement command structure (dedicated `sp placement` vs `sp dataset place` subcommand)
- Link naming strategy (auto-name from nodes vs user-provided)
- Create output format (ID in text output or JSON-only)
- List command format (one-liner vs table)
- Size display format (auto-scaling e.g. "4.0 TB", "500 GB")
- Capacity/bandwidth parsing details (accept human units, binary vs decimal, bandwidth conventions)
- ID prefix minimum length for disambiguation
- Rename support for entities
- Topology update command
- Cross-entity validation warnings (likely defer to Phase 4)

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

## Existing Codebase Analysis

### What Phase 1 Delivered (HIGH confidence -- read from source)

**Working Infrastructure:**
- `Database` struct with `open()`, `open_memory()`, `transaction()`, `conn()`, `migrate()`
- PRAGMA user_version = 1 migration creating all 9 tables + events + undo_pointer
- Foreign keys ON, WAL mode, cascade deletes configured
- 34 passing tests covering models, events, undo/redo

**Working Model Structs (src/core/models.rs):**
All 7 entity types + Event, each with: `new()`, `insert(&Transaction)`, `from_row(&Row)`, `to_json()`
- `Topology { id, name, description, parent_id, is_active, created_at, updated_at }`
- `Node { id, topology_id, name, role, location, available_bays, interface_types, power_draw_watts, created_at, updated_at }`
- `Volume { id, topology_id, node_id, name, capacity_bytes, usable_bytes, filesystem, raid_level, pool_type, item_id, created_at, updated_at }`
- `Dataset { id, topology_id, name, size_bytes, growth_rate_bytes_month, criticality, min_copies, min_locations, max_rpo_hours, created_at, updated_at }`
- `Placement { id, topology_id, dataset_id, volume_id, role, priority, created_at }`
- `Link { id, topology_id, source_node_id, target_node_id, bandwidth_bytes_sec, connection_type, latency_ms, is_metered, cost_per_gb_cents, created_at, updated_at }`
- `SyncRegime { id, topology_id, name, dataset_id, source_volume_id, target_volume_id, sync_type, schedule, direction, created_at, updated_at }`

**Working Event System (src/core/events.rs):**
- `record_event(tx, event_type, entity_type, entity_id, summary, before_state, after_state, source)` -- records event within transaction, manages undo pointer, clears redo stack
- `undo(db)` / `redo(db)` -- full multi-level undo/redo with before/after state restoration
- `restore_entity_from_json(tx, entity_type, json)` -- deserializes any entity type from JSON and inserts
- `delete_entity(tx, entity_type, entity_id)` -- deletes any entity by type
- `entity_table_name(entity_type)` -- maps entity type to table name

**Working Topology CRUD (src/cli/topology.rs -- THE TEMPLATE):**
- `create`: New Topology, insert, record "topology.created" event, auto-activate if first
- `list`: Query all topologies, format as text (name + active indicator + description) or JSON
- `show`: Query topology by name, show summary with entity counts, or full JSON
- `set-active`: Transaction: deactivate all, activate target, record "topology.updated" event
- `delete`: Delete topology, record "topology.deleted" event with before_state

**Placeholder Commands (5 files, all identical pattern):**
Each defines clap Subcommand enum with args, but `run()` just prints "Coming in Phase 2."
- `node.rs`: Add(name, role, location, bays), List, Show(name), Remove(name)
- `volume.rs`: Add(name, node, capacity, filesystem, raid), List, Show(name), Remove(name)
- `dataset.rs`: Add(name, size, criticality, min_copies), List, Show(name), Remove(name)
- `link.rs`: Add(from, to, type, bandwidth, metered), List, Show(name), Remove(name)
- `sync_regime.rs`: Add(name, dataset, from, to, type, schedule), List, Show(name), Remove(name)

**CLI Shell (src/cli/mod.rs):**
- `Cli` struct with `--dir`, `--format` (global), `Commands` enum
- `OutputFormat` enum: Text, Json
- `open_db(path)` helper -- checks existence, opens database
- Commands dispatch to module functions
- Node/volume/dataset/link/sync run functions currently DON'T receive `db` or `format` (they just print placeholder text)

**Capacity Parsing (src/core/specs.rs -- ALREADY EXISTS):**
- `Capacity::parse("4TB")` -- handles KB/MB/GB/TB/PB + KiB/MiB/GiB/TiB, case-insensitive
- `Capacity::from_bytes(n)` -- construct from raw bytes
- `Capacity` Display impl: auto-scales to appropriate unit (TB/GB/MB/B)
- `Speed::parse("560MB/s")` -- handles MB/s, GB/s notation
- `Speed` Display impl: auto-scales to GB/s or MB/s
- All fully tested and working

### What Phase 2 Must Change

1. **CLI mod.rs**: Route node/volume/dataset/link/sync commands through `open_db()` and pass `&mut Database` + `OutputFormat`
2. **Each placeholder file**: Replace "Coming in Phase 2" with real CRUD implementations following topology.rs pattern
3. **New resolver module**: Shared name-or-ID entity resolution logic
4. **Topology commands**: Add `--topology` override, enhance `show` with `--tree`/`--verbose`, add `update` subcommand
5. **Placement commands**: New command (either dedicated or as subcommand)
6. **Arg structures**: Add `--topology` to all entity commands, extend existing args where needed

## Standard Stack

### Core (already in use -- no new crates needed)
| Library | Version | Purpose | Status |
|---------|---------|---------|--------|
| rusqlite | 0.31 | SQLite queries and transactions | Already in Cargo.toml |
| clap | 4 (derive) | CLI framework with subcommands | Already in Cargo.toml |
| serde/serde_json | 1 | JSON serialization for events | Already in Cargo.toml |
| chrono | 0.4 | Timestamps | Already in Cargo.toml |
| uuid | 1 | ID generation | Already in Cargo.toml |
| anyhow | 1 | Error handling | Already in Cargo.toml |
| console | 0.15 | Terminal styling | Already in Cargo.toml |

### Supporting (already available)
| Module | Purpose | Status |
|--------|---------|--------|
| `core::specs::Capacity` | Parse "4TB", "500GB" capacity strings | Already implemented and tested |
| `core::specs::Speed` | Parse "560MB/s", "1GB/s" speed strings | Already implemented and tested |
| `core::events::record_event` | Event recording within transactions | Already implemented and tested |
| `core::events::undo`/`redo` | Multi-level undo/redo | Already implemented and tested |
| `core::events::restore_entity_from_json` | Restore any entity type from JSON | Already handles all 7 entity types |

### No New Dependencies Needed
No new crates. Everything needed is already in Cargo.toml or implementable with standard Rust.

## Architecture Patterns

### Pattern 1: Entity CRUD Command (THE pattern for Phase 2)

**What:** Every entity command follows the exact same structure as topology.rs. This is the repeatable unit of work.

**Template (from working topology.rs):**
```rust
// Create pattern:
fn create(db: &mut Database, args, format: OutputFormat) -> Result<()> {
    let entity = Entity::new(/* args */);
    let after_json = entity.to_json()?;
    let entity_id = entity.id.clone();

    db.transaction(|tx| {
        entity.insert(tx)?;
        record_event(tx, "entity_type.created", "entity_type", &entity_id,
            &format!("Created entity '{}'", name),
            None, Some(&after_json), &EventSource::User)?;
        Ok(())
    })?;

    // Output based on format
    match format {
        OutputFormat::Text => println!("Created entity '{}'", name),
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&json!({
            "action": "created", "entity_type": name, "id": entity_id
        }))?),
    }
    Ok(())
}

// Delete pattern:
fn delete(db: &mut Database, name: &str, format: OutputFormat) -> Result<()> {
    db.transaction(|tx| {
        let entity = find_entity(tx, name)?;  // resolver
        let before_json = entity.to_json()?;

        // Warn about cascaded dependents (print, don't prompt)
        let dependents = count_dependents(tx, &entity.id)?;
        if dependents > 0 {
            eprintln!("Warning: cascading delete will remove {} dependent entities", dependents);
        }

        tx.execute("DELETE FROM table WHERE id = ?1", [&entity.id])?;
        record_event(tx, "entity_type.deleted", "entity_type", &entity.id,
            &format!("Deleted entity '{}'", name),
            Some(&before_json), None, &EventSource::User)?;
        Ok(())
    })?;
    Ok(())
}
```

### Pattern 2: Entity Resolver (name-or-ID lookup)

**What:** A shared function that resolves an entity reference to an ID. Every command uses this to find entities by user-provided name or UUID prefix.

**Design:**
```rust
// In a new src/cli/resolve.rs module
pub fn resolve_entity(
    conn: &Connection,
    table: &str,
    name_column: &str,  // usually "name"
    reference: &str,
    topology_id: &str,
    scope_column: Option<(&str, &str)>,  // e.g. ("node_id", node_id) for volumes
) -> Result<String> {
    // 1. Try exact name match within topology
    //    SELECT id FROM {table} WHERE {name_column} = ?1 AND topology_id = ?2
    //    If scope_column provided: AND {scope_col} = ?3

    // 2. If no name match, try UUID prefix (minimum 4 chars)
    //    SELECT id FROM {table} WHERE id LIKE ?1 || '%' AND topology_id = ?2

    // 3. If multiple matches, error with list
    // 4. If zero matches, error "not found"
}

pub fn get_active_topology_id(conn: &Connection) -> Result<String> {
    // SELECT id FROM topologies WHERE is_active = 1
    // Error if none active
}

pub fn resolve_topology_id(
    conn: &Connection,
    topology_override: Option<&str>,
) -> Result<String> {
    match topology_override {
        Some(ref_str) => resolve_entity(conn, "topologies", "name", ref_str, ...),
        None => get_active_topology_id(conn),
    }
}
```

**Why this pattern:** The context requires ALL entity references to resolve the same way. A shared function enforces this and prevents per-command reimplementation.

### Pattern 3: Cascade Delete with Warning

**What:** When deleting an entity that has dependents, print what will be deleted but proceed without prompting.

**Design:**
```rust
fn warn_cascade(tx: &Transaction, entity_type: &str, entity_id: &str) -> Result<()> {
    // Count dependent entities based on type
    match entity_type {
        "node" => {
            let vol_count: i64 = tx.query_row(
                "SELECT COUNT(*) FROM volumes WHERE node_id = ?1", [entity_id], |r| r.get(0))?;
            if vol_count > 0 {
                eprintln!("  Will delete: {} volume(s)", vol_count);
                // Also count placements on those volumes, etc.
            }
        }
        "volume" => {
            let pl_count: i64 = tx.query_row(
                "SELECT COUNT(*) FROM placements WHERE volume_id = ?1", [entity_id], |r| r.get(0))?;
            // etc.
        }
        // ... other entity types
    }
    Ok(())
}
```

**Key detail:** The cascade happens via SQL foreign keys (`ON DELETE CASCADE`), so the warning is informational. The actual delete is just `DELETE FROM table WHERE id = ?`. The event's `before_state` captures only the top-level entity; cascaded children are NOT individually captured in events. This means undo of a cascade delete only restores the parent -- this is a known limitation. The user accepted this since undo is "available for recovery" as a safety net, not a perfect inverse.

### Pattern 4: Active Topology as Default Scope

**What:** Every entity command operates on the active topology by default, with `--topology` override.

**Design:**
```rust
// In each entity command file:
#[derive(Subcommand)]
pub enum NodeCommands {
    Add {
        name: String,
        #[arg(long)]
        role: String,
        // ... other args

        /// Target topology (default: active)
        #[arg(long)]
        topology: Option<String>,
    },
    // ...
}

fn add(db: &mut Database, name: &str, role: &str, topology_override: Option<&str>, format: OutputFormat) -> Result<()> {
    let topology_id = resolve_topology_id(db.conn(), topology_override)?;
    // ... use topology_id for insert
}
```

### Pattern 5: Update-in-Place with Partial Fields

**What:** For mutable entities (nodes, volumes, datasets, topologies), update only the fields specified by the user. Unspecified fields remain unchanged.

**Design:**
```rust
// Update subcommand uses Option<T> for all updateable fields
Update {
    /// Entity to update (name or ID)
    name: String,
    /// New name
    #[arg(long)]
    rename: Option<String>,
    /// New location
    #[arg(long)]
    location: Option<String>,
    // ... all optional
}

fn update(db: &mut Database, name: &str, updates: UpdateArgs, format: OutputFormat) -> Result<()> {
    db.transaction(|tx| {
        let entity = find_entity(tx, name)?;
        let before_json = entity.to_json()?;

        // Build SET clause dynamically based on which args were provided
        let mut sets = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        if let Some(ref new_name) = updates.rename {
            sets.push("name = ?");
            params.push(Box::new(new_name.clone()));
        }
        // ... etc for each optional field

        if sets.is_empty() {
            bail!("No updates specified");
        }

        sets.push("updated_at = datetime('now')");
        let sql = format!("UPDATE {} SET {} WHERE id = ?", table, sets.join(", "));
        params.push(Box::new(entity.id.clone()));
        tx.execute(&sql, rusqlite::params_from_iter(params))?;

        // Re-read entity for after_state
        let updated = load_entity(tx, &entity.id)?;
        let after_json = updated.to_json()?;

        record_event(tx, "entity.updated", ...)?;
        Ok(())
    })?;
    Ok(())
}
```

### Pattern 6: Slug Name Validation

**What:** Validate that entity names are slug-like (alphanumeric + hyphens + underscores only).

**Design:**
```rust
fn validate_slug(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("Name cannot be empty");
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        bail!("Name must contain only alphanumeric characters, hyphens, and underscores: '{}'", name);
    }
    Ok(())
}
```

### Anti-Patterns to Avoid

- **Per-command entity resolution**: Do NOT implement name-or-ID lookup separately in each command file. Extract it once to a shared module.
- **Interactive prompts**: The tool is agent-friendly. NEVER prompt for confirmation. Always proceed after printing warnings.
- **Inconsistent output**: Every command that produces output must respect `--format`. Text format for humans, JSON for agents. Never mix.
- **Undo of cascade-delete recording all children**: This would be extremely complex. Record only the top-level entity. Accept the limitation.
- **String-building SQL**: Use `rusqlite::params![]` for all values. The update-in-place pattern with dynamic SET clauses is the one exception where dynamic SQL building is needed, and even there, values go through params.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Capacity parsing | Custom parser | `core::specs::Capacity::parse()` | Already exists, handles TB/GB/MB/KiB/GiB/TiB, tested |
| Speed/bandwidth parsing | Custom parser | `core::specs::Speed::parse()` | Already exists, handles MB/s and GB/s |
| Capacity display | Custom formatter | `Capacity` Display impl | Already auto-scales to appropriate unit |
| UUID generation | Custom IDs | `uuid::Uuid::new_v4()` | Already in codebase |
| Event recording | Custom event logic | `events::record_event()` | Already handles sequence, undo pointer, redo stack clearing |
| Entity undo/redo | Custom undo per entity | `events::undo()`/`redo()` | Already handles all 7 entity types via `restore_entity_from_json` |
| Transaction management | Manual commit/rollback | `Database::transaction()` | Already in codebase |
| CLI arg parsing | Manual parsing | clap derive macros | Already in codebase |
| Name validation | Complex regex | Simple `chars().all()` check | Slugs are simple: alphanumeric + hyphens + underscores |

**Key insight:** Phase 1 built almost all the infrastructure needed. Phase 2's job is mostly wiring -- connecting CLI args to database operations through the existing patterns. The only genuinely new piece is the entity resolver.

## Claude's Discretion Recommendations

### Placement Command Structure
**Recommendation: Dedicated `sp placement` command**

Rationale: Placements are a distinct entity type in the database with their own CRUD lifecycle. Using `sp dataset place` would make the dataset command responsible for two entity types. A dedicated `sp placement add --dataset=X --volume=Y` keeps each command file responsible for one entity, consistent with all other commands. The existing `src/cli/mod.rs` already routes to separate command modules.

### Link Naming Strategy
**Recommendation: Auto-name from nodes (e.g., `mac-mini--nas`)**

Rationale: Links are uniquely identified by their source and target nodes within a topology (UNIQUE constraint on `topology_id, source_node_id, target_node_id`). Auto-naming from node names provides a natural, memorable identifier. The `--` separator matches the convention already hinted at in the placeholder `link.rs` (Show and Remove use "source--target"). The link table has no `name` column in the schema, so "names" are just display labels derived from node names. Users reference links by the auto-generated name or by `--from` + `--to` node refs.

Note: The `links` table does NOT have a `name` column. This means links are referenced by their node pair, not by name. The resolver for links needs to work differently: parse `source--target` format or require `--from`/`--to` flags.

### Create Output Format
**Recommendation: Show entity ID in text output**

Rationale: Since entities can be referenced by ID prefix, showing the ID on create helps users immediately reference the entity. Keep it compact: `Created node 'mac-mini' (id: a1b2c3d4)`. JSON output includes the full ID as a field. The truncated ID in text output gives just enough for prefix matching.

Format: `Created {entity_type} '{name}' (id: {first_8_chars})`

### List Command Format
**Recommendation: Compact one-liner per entity**

Rationale: The topology `list` command already uses this format (`  my-topology (active) - description`). Follow the same pattern for all entity types. Tables with headers add visual noise for small lists (typical topology has 2-5 nodes, 5-10 volumes). One-liner format is scannable and works well in both terminal and piped output. JSON format provides full detail when needed.

Format examples:
```
Nodes:
  mac-mini  desktop  office  (0 bays)
  nas       nas      closet  (8 bays)

Volumes:
  main-ssd  mac-mini  1.0TB  apfs
  pool      nas       4.0TB  zfs (raidz1)

Datasets:
  photos    500.0GB  critical  3 copies
  documents 100.0GB  normal    2 copies
```

### Size Display Format
**Recommendation: Use existing `Capacity` Display impl (auto-scaling)**

The `Capacity` Display implementation already produces `"4.0TB"`, `"500.0GB"`, `"100.0MB"`. This is suitable. For consistency, always show one decimal place. The existing implementation handles this.

### Capacity/Bandwidth Parsing Details
**Recommendation: Use existing `Capacity::parse()` and `Speed::parse()`**

Already implemented in `specs.rs`:
- Accepts: `4TB`, `500GB`, `100MB`, `1.5TB` (decimal units)
- Accepts: `4TiB`, `500GiB` (binary units)
- Case insensitive: `4tb` = `4TB`
- Speed: `560MB/s`, `1GB/s` (with or without `/s` suffix)
- For bandwidth on links, use `Speed::parse()` and store as `bytes_per_sec`
- Raw byte input (all digits): Not currently supported by `Capacity::parse()`. Add a simple check: if all characters are digits, treat as bytes. This is a minor enhancement.

### ID Prefix Minimum Length
**Recommendation: 4 characters minimum**

Rationale: UUIDs are 36 characters (8-4-4-4-12 hex). With 4 hex characters, there are 65,536 possible prefixes. For a system with typically <100 entities, collision probability is negligible. 4 characters is short enough to type easily but long enough to be unambiguous.

If a prefix matches multiple entities, error with: `"Ambiguous ID prefix 'a1b2' matches: entity-name-1 (a1b2c3d4...), entity-name-2 (a1b2e5f6...)"`

### Rename Support
**Recommendation: Yes, via `--rename` flag on update commands**

Rationale: Names are user-facing slugs, IDs are the real identifiers. Renaming is safe because nothing references entities by name internally (all FKs use IDs). Add `--rename=new-name` to the update subcommand for nodes, volumes, datasets, topologies. Check uniqueness of the new name within scope.

### Topology Update Command
**Recommendation: Yes, add `sp topology update <name>`**

Support updating: `--description`, `--rename`. The topology command already exists with create/list/show/set-active/delete. Adding update follows the same pattern and is needed for completeness. Record a "topology.updated" event with before/after state.

### Cross-Entity Validation Warnings
**Recommendation: Defer to Phase 4**

Phase 2 is about CRUD, not analysis. No warnings about over-provisioning, unplaced datasets, disconnected nodes, etc. Those are analysis functions. Phase 2 commands just do what they're told: create, read, update, delete.

## Common Pitfalls

### Pitfall 1: Cascade Delete Event Recording
**What goes wrong:** Deleting a node cascades to delete its volumes, placements on those volumes, and sync regimes referencing those volumes. The undo event only captures the node's before_state, so undoing the delete restores the node but not its volumes.
**Why it happens:** SQL CASCADE handles the actual deletion, but the event system only records the top-level entity.
**How to avoid:** Accept the limitation. Document it. The cascade warning output tells the user what was deleted. For complete recovery, the user can undo further back to before the volumes were created. Alternatively, capture cascaded entities in the before_state JSON as a nested object -- but this adds significant complexity.
**Recommendation:** For Phase 2, record only the top-level entity in events. The cascade warning serves as documentation. Enhancing undo for cascades can be a future improvement if needed.

### Pitfall 2: Volume Name Ambiguity
**What goes wrong:** Two nodes both have a volume named "ssd-1". User runs `sp volume show ssd-1` and gets an error.
**Why it happens:** Volume names are unique within a node (UNIQUE(topology_id, node_id, name)), not within a topology.
**How to avoid:** When a volume name resolves to multiple matches within a topology, require `--node` to disambiguate. Error message: `"Volume 'ssd-1' exists on multiple nodes: mac-mini, nas. Use --node to specify."`
**Warning signs:** Tests that assume volume names are globally unique.

### Pitfall 3: Placeholder Command Signatures Don't Match Real Needs
**What goes wrong:** The placeholder arg structs were designed before Phase 2 decisions. Some args may be missing (e.g., `--topology` override, `--usable` for volumes) or have wrong types (e.g., capacity as String vs parsed Capacity).
**Why it happens:** D008 says "placeholder commands define full arg structure for Phase 2" but some details were deferred to Claude's discretion.
**How to avoid:** Treat placeholder arg structs as starting points, not final. Add `--topology` to all commands. Add missing args. Change types if needed (e.g., keep capacity as String in clap, parse with `Capacity::parse()` in the handler).

### Pitfall 4: run() Signature Mismatch in cli/mod.rs
**What goes wrong:** Placeholder commands have `pub fn run(cmd: NodeCommands) -> Result<()>` with no `db` or `format` parameter. Real commands need `pub fn run(cmd: NodeCommands, db: &mut Database, format: OutputFormat) -> Result<()>`.
**Why it happens:** Phase 1 intentionally kept placeholders simple.
**How to avoid:** Update all `run()` signatures and the dispatch in `cli/mod.rs` simultaneously. The compiler will catch any mismatches.

### Pitfall 5: Forgetting to Record Events
**What goes wrong:** A command creates/deletes/updates an entity but doesn't call `record_event()`. Undo doesn't work for that operation.
**Why it happens:** Easy to forget in the pattern of "just make it work."
**How to avoid:** Every mutating function must follow the transaction-event pattern. Tests should verify events are recorded. The topology.rs pattern shows exactly where `record_event()` goes.

### Pitfall 6: JSON State Mismatches After Update
**What goes wrong:** An update command modifies the database row directly with SQL, then tries to build after_state by re-reading the row. The `from_row()` and the SQL UPDATE might disagree on column interpretation.
**Why it happens:** Partial updates use dynamic SQL but after_state uses `from_row()` which expects all columns.
**How to avoid:** Always re-read the full entity after update: `SELECT * FROM table WHERE id = ?`. Use the same `from_row()` that roundtrip tests validate. This guarantees the JSON state exactly represents the database row.

## Code Examples

### Entity Resolver (new code needed)

```rust
// src/cli/resolve.rs
use anyhow::{bail, Result};
use rusqlite::Connection;

/// Resolve an entity reference (name or ID prefix) to its full ID.
/// Searches within the given topology scope.
pub fn resolve_entity_id(
    conn: &Connection,
    table: &str,
    reference: &str,
    topology_id: &str,
) -> Result<String> {
    // Try name match first
    let name_sql = format!(
        "SELECT id FROM {} WHERE name = ?1 AND topology_id = ?2",
        table
    );
    let name_result: Vec<String> = {
        let mut stmt = conn.prepare(&name_sql)?;
        stmt.query_map(rusqlite::params![reference, topology_id], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?
    };

    if name_result.len() == 1 {
        return Ok(name_result[0].clone());
    }

    // Try ID prefix match (minimum 4 chars)
    if reference.len() >= 4 {
        let prefix_sql = format!(
            "SELECT id, name FROM {} WHERE id LIKE ?1 AND topology_id = ?2",
            table
        );
        let prefix_pattern = format!("{}%", reference);
        let mut stmt = conn.prepare(&prefix_sql)?;
        let matches: Vec<(String, String)> = stmt
            .query_map(rusqlite::params![prefix_pattern, topology_id], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        match matches.len() {
            0 => {}  // fall through to not-found error
            1 => return Ok(matches[0].0.clone()),
            _ => {
                let options: Vec<String> = matches.iter()
                    .map(|(id, name)| format!("  {} ({}...)", name, &id[..8]))
                    .collect();
                bail!("Ambiguous ID prefix '{}' matches:\n{}", reference, options.join("\n"));
            }
        }
    }

    bail!("{} '{}' not found in active topology", table.trim_end_matches('s'), reference);
}

/// Resolve volume with optional node disambiguation
pub fn resolve_volume_id(
    conn: &Connection,
    reference: &str,
    topology_id: &str,
    node_hint: Option<&str>,
) -> Result<String> {
    // Similar to above but with node scoping for disambiguation
    // ...
}

/// Get the active topology ID, or resolve from --topology override
pub fn resolve_topology_id(
    conn: &Connection,
    topology_override: Option<&str>,
) -> Result<String> {
    match topology_override {
        Some(name_or_id) => {
            // Resolve topology by name or ID (no topology_id scope needed)
            let sql = "SELECT id FROM topologies WHERE name = ?1";
            match conn.query_row(sql, [name_or_id], |row| row.get::<_, String>(0)) {
                Ok(id) => Ok(id),
                Err(_) => {
                    // Try ID prefix
                    if name_or_id.len() >= 4 {
                        let prefix = format!("{}%", name_or_id);
                        let mut stmt = conn.prepare(
                            "SELECT id FROM topologies WHERE id LIKE ?1"
                        )?;
                        let matches: Vec<String> = stmt
                            .query_map([&prefix], |row| row.get(0))?
                            .collect::<Result<Vec<_>, _>>()?;
                        match matches.len() {
                            1 => Ok(matches[0].clone()),
                            0 => bail!("Topology '{}' not found", name_or_id),
                            _ => bail!("Ambiguous topology reference '{}'", name_or_id),
                        }
                    } else {
                        bail!("Topology '{}' not found", name_or_id)
                    }
                }
            }
        }
        None => {
            conn.query_row(
                "SELECT id FROM topologies WHERE is_active = 1",
                [],
                |row| row.get(0),
            ).map_err(|_| anyhow::anyhow!("No active topology. Create one with 'sp topology create <name>' or set one with 'sp topology set-active <name>'"))
        }
    }
}
```

### Node Add Command (representative of all add commands)

```rust
fn add(
    db: &mut Database,
    name: &str,
    role: &str,
    location: Option<&str>,
    bays: Option<i32>,
    topology_override: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    validate_slug(name)?;
    let topology_id = resolve_topology_id(db.conn(), topology_override)?;

    let mut node = Node::new(&topology_id, name, role);
    if let Some(loc) = location {
        node.location = loc.to_string();
    }
    if let Some(b) = bays {
        node.available_bays = Some(b);
    }

    let after_json = node.to_json()?;
    let node_id = node.id.clone();
    let node_name = node.name.clone();

    db.transaction(|tx| {
        node.insert(tx)?;
        record_event(
            tx, "node.created", "node", &node_id,
            &format!("Created node '{}'", node_name),
            None, Some(&after_json), &EventSource::User,
        )?;
        Ok(())
    })?;

    match format {
        OutputFormat::Text => println!("Created node '{}' (id: {})", name, &node_id[..8]),
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "action": "created", "entity_type": "node",
            "name": name, "id": node_id
        }))?),
    }
    Ok(())
}
```

### Volume Add with Capacity Parsing

```rust
fn add(
    db: &mut Database,
    name: &str,
    node_ref: &str,
    capacity_str: &str,
    usable_str: Option<&str>,
    filesystem: Option<&str>,
    raid: Option<&str>,
    topology_override: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    validate_slug(name)?;
    let topology_id = resolve_topology_id(db.conn(), topology_override)?;
    let node_id = resolve_entity_id(db.conn(), "nodes", node_ref, &topology_id)?;

    let capacity = Capacity::parse(capacity_str)
        .map_err(|e| anyhow::anyhow!("Invalid capacity '{}': {}", capacity_str, e))?;

    let usable = usable_str.map(|s| Capacity::parse(s))
        .transpose()
        .map_err(|e| anyhow::anyhow!("Invalid usable capacity: {}", e))?;

    let mut vol = Volume::new(&topology_id, &node_id, name, capacity.bytes as i64);
    vol.usable_bytes = usable.map(|c| c.bytes as i64);
    vol.filesystem = filesystem.map(|s| s.to_string());
    vol.raid_level = raid.map(|s| s.to_string());

    // ... same transaction-event pattern as node add
}
```

### Topology Show with --tree Flag

```rust
fn show(db: &mut Database, name: &str, tree: bool, format: OutputFormat) -> Result<()> {
    let topo = find_topology_by_name(db.conn(), name)?;

    match format {
        OutputFormat::Text => {
            // Summary (always shown)
            println!("Topology: {}", topo.name);
            println!("  ID:          {}", topo.id);
            println!("  Description: {}", topo.description);
            println!("  Active:      {}", if topo.is_active { "yes" } else { "no" });

            let node_count = count_nodes(db.conn(), &topo.id)?;
            let vol_count = count_volumes(db.conn(), &topo.id)?;
            let ds_count = count_datasets(db.conn(), &topo.id)?;
            println!("  Nodes:       {}", node_count);
            println!("  Volumes:     {}", vol_count);
            println!("  Datasets:    {}", ds_count);

            if tree {
                // Full hierarchy
                println!();
                let nodes = list_nodes(db.conn(), &topo.id)?;
                for node in &nodes {
                    println!("  {} ({}, {})", node.name, node.role, node.location);
                    let volumes = list_volumes_for_node(db.conn(), &node.id)?;
                    for vol in &volumes {
                        let cap = Capacity::from_bytes(vol.capacity_bytes as u64);
                        println!("    {} {}", vol.name, cap);
                    }
                }
                // ... datasets, links, sync regimes
            }
        }
        OutputFormat::Json => {
            if tree {
                // Include nested entities in JSON
            } else {
                println!("{}", serde_json::to_string_pretty(&topo)?);
            }
        }
    }
    Ok(())
}
```

## State of the Art

| Current State (Phase 1) | Phase 2 Target | Impact |
|--------------------------|----------------|--------|
| Placeholder commands print "Coming in Phase 2" | Full CRUD for all 7 entity types | Users can actually build topologies |
| Topology show has basic entity counts | Summary + `--tree` for full hierarchy | Users can inspect topology structure |
| Entity lookup by exact name only (topology) | Name-or-ID resolver with prefix matching | Consistent, flexible entity referencing |
| Capacity stored as raw i64 bytes | Human-readable input ("4TB") via Capacity::parse() | Natural CLI input |
| No `--topology` override | Every entity command supports `--topology` | Agents can work across topologies without set-active |
| No update commands | Update-in-place for mutable entities | Users can modify entities without delete/recreate |
| No cascade warnings | Print cascade info before delete | Users know what they're deleting |
| No name validation | Slug validation on create/rename | Prevents shell quoting issues |

## Open Questions

1. **Placement undo with cascade delete**
   - What we know: Deleting a volume cascades to delete its placements. The event only records the volume.
   - What's unclear: Is this acceptable or should cascade-deleted placements be recorded?
   - Recommendation: Accept the limitation for Phase 2. The cascade warning lists what will be deleted. Users can undo further back if needed.

2. **Link table has no name column**
   - What we know: The `links` table schema uses UNIQUE(topology_id, source_node_id, target_node_id). There is no `name` column.
   - What's unclear: The resolver pattern assumes entities have names. Links don't.
   - Recommendation: Links are referenced by auto-generated display name (`source--target`) or by `--from`/`--to` flags. The link resolver is a special case that parses the `--` separator or matches on node names. For ID-based access, standard prefix matching works. Add a `name` field to the Link model (computed, not stored) or handle links as a special case in the resolver.

3. **Bandwidth input format**
   - What we know: `Speed::parse()` handles "560MB/s" and "1GB/s". Links store `bandwidth_bytes_sec`.
   - What's unclear: Should we also accept networking conventions like "10Gbps" (bits per second)?
   - Recommendation: For Phase 2, accept the formats `Speed::parse()` already handles (byte-based). Networking conventions ("Gbps") can be added later if needed. Document that bandwidth is in bytes/sec.

4. **Topology commands need `--topology` but topology IS the entity**
   - What we know: `sp topology show my-topo` references the topology by name directly. No `--topology` override needed.
   - What's unclear: Should topology commands also accept ID prefix matching?
   - Recommendation: Yes, enhance topology commands to use the same resolver. `sp topology show my-topo` works by name, `sp topology show a1b2` works by ID prefix. This is consistent with the "all entity references resolve the same way" decision. The topology resolver just doesn't need a topology_id scope.

## Sources

### Primary (HIGH confidence)
- Existing codebase files read directly:
  - `src/cli/mod.rs`, `src/cli/topology.rs` -- working CRUD template
  - `src/cli/node.rs`, `volume.rs`, `dataset.rs`, `link.rs`, `sync_regime.rs` -- placeholder arg structures
  - `src/core/models.rs` -- all entity model structs with insert/from_row/to_json
  - `src/core/events.rs` -- event recording, undo/redo engine
  - `src/core/db.rs` -- database schema, migrations, transaction interface
  - `src/core/specs.rs` -- Capacity::parse(), Speed::parse() implementations
  - `Cargo.toml` -- dependency versions
- Phase 1 research and plans: `.planning/phases/01-schema-and-core-types/01-RESEARCH.md`, `01-02-PLAN.md`
- Phase 2 context: `.planning/phases/02-cli-scaffolding-and-basic-commands/02-CONTEXT.md`
- Project state: `.planning/STATE.md`
- Requirements: `.planning/REQUIREMENTS.md`

### Secondary (MEDIUM confidence)
- Context7: clap derive API documentation for custom type parsing and ValueEnum patterns
- Codebase conventions: `.planning/codebase/CONVENTIONS.md`, `ARCHITECTURE.md`

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- no new libraries, everything is already in Cargo.toml
- Architecture: HIGH -- topology.rs is a proven template, patterns are directly derived from working code
- Entity resolver: HIGH -- design is straightforward, similar patterns are well-established in CLI tools
- Pitfalls: HIGH -- identified from actual schema constraints and code patterns in the codebase
- Claude's discretion recommendations: MEDIUM -- reasonable defaults, may need adjustment during implementation

**Research date:** 2026-02-07
**Valid until:** 2026-03-07 (stable domain, no external dependency changes expected)

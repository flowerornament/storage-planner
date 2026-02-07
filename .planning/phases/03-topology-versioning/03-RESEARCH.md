# Phase 3: Topology Versioning - Research

**Researched:** 2026-02-07
**Domain:** Topology forking, tagging, diffing, lineage display in a Rust CLI with SQLite
**Confidence:** HIGH

## Summary

Phase 3 adds exploration workflows on top of the Phase 2 CRUD: forking topologies to create alternatives, tagging them with lifecycle states (current/exploring/archived), diffing two topologies to see what changed, and viewing fork lineage as a tree. The codebase already has a `parent_id` column on the `topologies` table (added in Phase 1 schema), an `is_active` boolean, and full entity CRUD with undo/redo. The `console` crate (v0.15) already in the dependency tree provides styled terminal output for diff presentation.

The core challenge is the fork operation (deep-copying all child entities with new UUIDs while preserving structural relationships) and the diff operation (comparing two topologies entity-by-entity with field-level detail). Both are pure SQL/Rust operations with no external library requirements. The tag system can cleanly replace the existing `is_active` boolean with a richer `tag` column that maps "current" to what `is_active` was doing.

**Primary recommendation:** Use the existing schema's `parent_id` for lineage, add a `tag` TEXT column to topologies (replacing `is_active`), implement deep copy via INSERT-SELECT with UUID remapping for fork, and build diff as in-memory comparison of entity collections loaded from two topologies.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- User can fork from any topology, not just the active one
- Fork name is optional -- user can provide via flag, otherwise auto-generate
- Topologies have lifecycle tags: current, exploring, archived
- Only one topology can be "current" at a time (enforced)
- Full detail diff: entity-level changes PLUS field-level diffs (e.g., "node capacity: 4TB -> 8TB")
- Diff supports filtering by entity type via flags (e.g., `--nodes --volumes`)
- If only one topology specified, diff uses the current/active topology as the implicit base
- Two lineage commands: tree view (all topologies as fork tree) and log view (single topology's ancestry)
- Tree view shows tags alongside each topology name (e.g., "my-topo [current]")

### Claude's Discretion
- Deep copy vs shallow strategy, auto-generated name format
- Whether tags replace or coexist with the existing set-active concept
- One tag per topology vs multiple tags
- What happens to the previous "current" when a new one is set
- Whether archived topologies are hidden by default in list
- Terminal presentation style for diff (git-style, side-by-side, summary+detail)
- Fork depth limits, detail level in topology show for parent/child info

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

## Existing Codebase Analysis

### Schema Foundation Already in Place (HIGH confidence -- read from source)

The Phase 1 schema already has the critical infrastructure for this phase:

**topologies table:**
```sql
CREATE TABLE topologies (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL DEFAULT '',
    parent_id TEXT REFERENCES topologies(id),  -- ALREADY EXISTS for fork lineage
    is_active INTEGER NOT NULL DEFAULT 0,       -- Will be replaced by tag column
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

**Key observations:**
- `parent_id` already exists with self-referencing FK -- ready for fork lineage
- `is_active` is a simple boolean -- needs to be replaced/augmented with tag system
- All child tables (nodes, volumes, datasets, placements, links, sync_regimes) use `topology_id` FK with `ON DELETE CASCADE`
- All entity types have `new()`, `insert()`, `from_row()`, `to_json()` methods
- All entities use UUID v4 string IDs
- The `set_active` command already deactivates all topologies then activates one (pattern reusable for "current" tag)

### Entity Relationships to Copy During Fork

During a fork, ALL of these must be deep-copied with remapped IDs:

| Entity | Count | FK Dependencies | Copy Complexity |
|--------|-------|-----------------|-----------------|
| topologies | 1 | parent_id -> source topology | New UUID, set parent_id |
| nodes | N | topology_id | New UUIDs, remap topology_id |
| volumes | N | topology_id, node_id | New UUIDs, remap topology_id AND node_id |
| datasets | N | topology_id | New UUIDs, remap topology_id |
| placements | N | topology_id, dataset_id, volume_id | New UUIDs, remap all three FKs |
| links | N | topology_id, source_node_id, target_node_id | New UUIDs, remap all three FKs |
| sync_regimes | N | topology_id, dataset_id, source_volume_id, target_volume_id | New UUIDs, remap all four FKs |

**Order matters**: Must copy in dependency order: topology -> nodes -> volumes -> datasets -> placements -> links -> sync_regimes.

### Existing CLI Patterns (HIGH confidence)

All commands follow these patterns:
1. Resolve entities outside transactions (D009)
2. Build before/after state for events
3. Execute mutations in `db.transaction()`
4. Record events with `record_event()`
5. Print results based on `OutputFormat` (Text/Json)
6. Global `--format` flag already exists on `Cli` struct

### Available Dependencies

No new crates needed. Everything uses:
- `rusqlite` for SQL
- `serde_json` for JSON serialization/comparison
- `console` for styled terminal output (already has `style()`, `Style::new().red()`, `.green()`, `.bold()`, `pad_str()`)
- `clap` for CLI argument parsing
- `uuid` for new ID generation

## Architecture Patterns

### Recommended Approach for Each Feature

### Pattern 1: Schema Migration v2 -- Add Tag Column

**What:** Add a `tag` TEXT column to topologies, migrate existing `is_active` data, then remove `is_active`.

**Approach:** Since SQLite does not support `DROP COLUMN` prior to 3.35.0 (and rusqlite 0.31 bundles SQLite 3.45+, which DOES support it), we have two options:
1. Add `tag` column alongside `is_active`, drop `is_active` via `ALTER TABLE ... DROP COLUMN`
2. Keep `is_active` for backwards compatibility but ignore it, use `tag` as the source of truth

**Recommendation:** Use approach 1. SQLite bundled with rusqlite 0.31 supports `ALTER TABLE ... DROP COLUMN`. The migration:

```sql
-- Migration v2: Add topology tags, replace is_active
ALTER TABLE topologies ADD COLUMN tag TEXT DEFAULT NULL;

-- Migrate existing data: is_active=1 -> tag='current'
UPDATE topologies SET tag = 'current' WHERE is_active = 1;

-- Drop the old column
ALTER TABLE topologies DROP COLUMN is_active;

PRAGMA user_version = 2;
```

**Tag semantics (Claude's discretion recommendation):**
- One tag per topology (single TEXT column, not a junction table). Multiple tags add complexity with no clear benefit for 3 fixed lifecycle states.
- Valid values: `current`, `exploring`, `archived`, or NULL (untagged = implicitly "exploring")
- Tags REPLACE `set-active`. The `set-active` command becomes `sp topology tag <name> current`. This is cleaner because "active" and "current" were already confusing duplicates.
- When a topology is tagged `current`, the previous `current` tag is automatically cleared (moved to NULL/untagged). This mirrors the existing `set_active` behavior.
- Archived topologies are NOT hidden by default in `list` (too surprising). Instead, `list` shows the tag inline: `my-topo [current]`, `old-setup [archived]`. A `--hide-archived` flag can be added if needed.

### Pattern 2: Deep Copy for Fork

**What:** Fork creates a complete independent copy of a topology and all its children with new UUIDs.

**Why deep copy (not shallow):** Shallow copy (sharing child entities) would mean editing nodes in a fork also changes them in the parent. This defeats the purpose of forking. Deep copy ensures independence.

**Implementation approach:**

```rust
// In a single transaction:
fn fork_topology(db: &mut Database, source_id: &str, new_name: &str) -> Result<String> {
    let source = resolve_topology(db, source_id)?;

    // Build ID remapping tables
    let mut node_id_map: HashMap<String, String> = HashMap::new();
    let mut volume_id_map: HashMap<String, String> = HashMap::new();
    let mut dataset_id_map: HashMap<String, String> = HashMap::new();

    let new_topo_id = Uuid::new_v4().to_string();

    db.transaction(|tx| {
        // 1. Create new topology with parent_id = source.id
        // 2. Copy nodes, building node_id_map
        // 3. Copy volumes, using node_id_map to remap node_id
        // 4. Copy datasets, building dataset_id_map
        // 5. Copy placements, using dataset_id_map + volume_id_map
        // 6. Copy links, using node_id_map
        // 7. Copy sync_regimes, using dataset_id_map + volume_id_map
        // 8. Record event
        Ok(new_topo_id)
    })
}
```

**Auto-generated name format (Claude's discretion):** Use `{source_name}-fork-{N}` where N is the next available suffix. Example: `current-setup` forks to `current-setup-fork-1`. If that exists, `current-setup-fork-2`. Simple, descriptive, discoverable.

### Pattern 3: Diff Engine

**What:** Compare two topologies entity-by-entity, then field-by-field within matching entities.

**Matching strategy:** Entities are matched by NAME (not ID, since forks generate new IDs). This is correct because:
- Names are unique within a topology (enforced by schema)
- Users think in names ("mac-mini node changed"), not UUIDs
- Forked entities preserve names from the source

**Diff algorithm:**
1. Load all entities of each type from both topologies
2. Build name-keyed maps for each entity type
3. For each entity type:
   - Items in left but not right = "removed"
   - Items in right but not left = "added"
   - Items in both = compare field-by-field for "changed"
4. Field-level diff: compare JSON representations, report changed fields with old/new values

**Entity types to diff:** nodes, volumes, datasets, placements, links, sync_regimes.

**Filtering:** `--nodes`, `--volumes`, `--datasets`, `--placements`, `--links`, `--syncs` flags. If none specified, diff all types. If one or more specified, diff only those types.

**Terminal presentation (Claude's discretion recommendation):** Git-style unified diff with color. This is the most familiar format for CLI users.

```
Diff: base-setup -> nvme-upgrade

Nodes:
  ~ nas [modified]
    available_bays: 4 -> 8

Volumes:
  + nas/nvme-cache: 2.0TB nvme
  ~ nas/main-pool:
    capacity: 16.0TB -> 32.0TB
    raid_level: raidz1 -> raidz2
  - mac-mini/old-ssd: 500.0GB

Datasets:
  (no changes)

Summary: 1 node modified, 1 volume added, 1 modified, 1 removed
```

Color scheme (using `console` crate):
- `+` added = green
- `-` removed = red
- `~` modified = yellow/cyan
- Field changes: `old_value` in red, `new_value` in green

### Pattern 4: Lineage Display

**Tree view:** Build from `parent_id` relationships. Load all topologies, construct a tree structure, render with box-drawing characters.

```
Topologies:
  base-setup [archived]
  +-- nvme-upgrade [current]
  |   +-- nvme-upgrade-budget [exploring]
  +-- sata-expansion
```

**Log view:** Walk `parent_id` chain from a specific topology back to root.

```
Ancestry of nvme-upgrade-budget:
  base-setup (2026-01-15)
  +-- nvme-upgrade (2026-01-20) [current]
      +-- nvme-upgrade-budget (2026-02-01) [exploring]  <-- you are here
```

**Fork depth limits (Claude's discretion):** No artificial limit. With 2-5 topologies (per D034 from Phase 1 research), depth will naturally stay shallow. Adding a limit adds complexity with no benefit.

**Detail in topology show (Claude's discretion):** Add parent name and child count to `topology show`:
```
Topology: nvme-upgrade [current]
  Description: NVMe upgrade path
  Forked from: base-setup
  Forks: 1 (nvme-upgrade-budget)
  Nodes: 3 | Volumes: 5 | Datasets: 4
```

### Pattern 5: Event Recording for New Commands

Follow existing patterns exactly:

| Command | Event Type | Entity Type | Before State | After State |
|---------|-----------|-------------|--------------|-------------|
| topology fork | topology.created | topology | None | New topology JSON |
| topology tag | topology.updated | topology | Before JSON | After JSON |
| topology untag | topology.updated | topology | Before JSON | After JSON |
| topology diff | (no event -- read-only) | N/A | N/A | N/A |
| topology tree | (no event -- read-only) | N/A | N/A | N/A |
| topology log | (no event -- read-only) | N/A | N/A | N/A |

### Anti-Patterns to Avoid

- **Shallow copy for fork:** Would create shared mutable state between parent and fork. Always deep copy.
- **Diffing by UUID:** Forked entities get new UUIDs. Always match by name within topology scope.
- **Adding a separate tags table:** Overkill for 3 fixed lifecycle states on a single entity type. A TEXT column is sufficient.
- **Hiding archived by default:** Surprises users who forget about archived topologies. Show everything with clear labels.
- **Complex diff algorithms (edit distance, LCS):** The entities are structured records with named fields. Simple set-difference + field comparison is sufficient and readable.

## Standard Stack

### Core (no new crates needed)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| rusqlite | 0.31 | SQLite queries for fork, diff, lineage | Already in use |
| serde_json | 1 | JSON comparison for field-level diff | Already in use |
| console | 0.15 | Colored diff output, tree rendering | Already in use |
| clap | 4 | New subcommands (fork, tag, diff, tree, log) | Already in use |
| uuid | 1 | New UUIDs for forked entities | Already in use |
| std::collections::HashMap | stdlib | ID remapping during fork | No dependency needed |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Manual JSON diff | similar crate | Adds dependency; our fields are simple types, manual comparison is ~20 lines |
| Manual tree rendering | ptree crate | Adds dependency; we have <10 nodes in tree, manual rendering is ~30 lines |
| console for colors | colored crate | console is already a dependency, no reason to add another |

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| UUID generation for forked entities | Custom ID scheme | `uuid::Uuid::new_v4()` | Already in use, proven |
| Terminal colors | ANSI escape codes directly | `console::style()` | Already in dependency tree |
| JSON serialization for diff | Manual field extraction | `serde_json::to_value()` then compare | Handles all types, existing pattern |

**Key insight:** This phase is mostly application logic (fork algorithm, diff algorithm, tree rendering). No novel technical challenges requiring external libraries.

## Common Pitfalls

### Pitfall 1: ID Remapping Errors During Fork
**What goes wrong:** Volumes reference old node IDs from the source topology instead of new IDs in the fork.
**Why it happens:** FK remapping is easy to get wrong with 6+ entity types and cross-references.
**How to avoid:** Build explicit `HashMap<OldId, NewId>` for each entity type. Process entities in strict dependency order. Write a test that forks a topology with all entity types and verifies all FKs point within the new topology.
**Warning signs:** FK constraint violations during fork, or entities in fork still pointing to source topology entities.

### Pitfall 2: Name Uniqueness Conflicts During Fork
**What goes wrong:** Fork fails because auto-generated name already exists.
**Why it happens:** User already has a topology with the generated name.
**How to avoid:** Check for name uniqueness before creating. Use incrementing suffix (`-fork-1`, `-fork-2`, ...) and loop until finding an available name.
**Warning signs:** Unique constraint violation on `topologies.name`.

### Pitfall 3: Diff Missing Entity Types
**What goes wrong:** Diff shows nodes and volumes but forgets placements, links, or sync_regimes.
**Why it happens:** Developer adds diff for "obvious" entity types and forgets junction/relationship entities.
**How to avoid:** Enumerate ALL 6 entity types explicitly. Test with a topology that has all entity types populated.

### Pitfall 4: Tag Migration Breaking set-active
**What goes wrong:** After migration, `set-active` still tries to write `is_active` column which was dropped.
**Why it happens:** CLI code references `is_active` in SQL and Rust model.
**How to avoid:** The migration and code change MUST happen atomically. The `Topology` struct's `is_active` field must be replaced by `tag: Option<String>`. All SQL queries referencing `is_active` must be updated. All places reading/writing `is_active` must switch to `tag`.

### Pitfall 5: Orphaned resolve_active_topology
**What goes wrong:** After replacing `is_active` with `tag`, `resolve_active_topology()` which queries `WHERE is_active = 1` breaks.
**Why it happens:** The resolver in `src/core/resolve.rs` directly queries the old column.
**How to avoid:** Update `resolve_active_topology()` to query `WHERE tag = 'current'` instead. This is a critical code path used by EVERY entity command (node, volume, dataset, placement, link, sync).

### Pitfall 6: Diff Between Unrelated Topologies
**What goes wrong:** User diffs two topologies that have no fork relationship, and the diff shows everything as added/removed with nothing matching.
**Why it happens:** Matching by name still works for unrelated topologies -- they just won't share many names.
**How to avoid:** This is actually fine behavior. Diff should work between ANY two topologies, not just parent-child. The diff output naturally shows the full difference. No special handling needed.

### Pitfall 7: Volume Name Matching in Diff
**What goes wrong:** Volume "data" on node "nas" in topology A matches volume "data" on node "mac-mini" in topology B.
**Why it happens:** Volume names are unique per (topology, node), not per topology globally. Two volumes can share a name if they're on different nodes.
**How to avoid:** Match volumes by compound key: `(node_name, volume_name)`. Similarly for placements: match by `(dataset_name, node_name/volume_name)` pair.

## Code Examples

### Fork with ID Remapping

```rust
use std::collections::HashMap;

fn fork_topology(
    db: &mut Database,
    source_name: &str,
    fork_name: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    let source = resolve_topology(db, source_name)?;

    // Generate fork name if not provided
    let name = match fork_name {
        Some(n) => {
            validate_slug(n)?;
            n.to_string()
        }
        None => generate_fork_name(db, &source.name)?,
    };

    // Check name uniqueness
    // ... (standard pattern from existing commands)

    let new_topo_id = Uuid::new_v4().to_string();
    let mut node_map: HashMap<String, String> = HashMap::new();
    let mut volume_map: HashMap<String, String> = HashMap::new();
    let mut dataset_map: HashMap<String, String> = HashMap::new();

    db.transaction(|tx| {
        // 1. Insert new topology
        let new_topo = Topology {
            id: new_topo_id.clone(),
            name: name.clone(),
            parent_id: Some(source.id.clone()),
            tag: None,  // forks start untagged
            ..source.clone()
        };
        new_topo.insert(tx)?;

        // 2. Copy nodes
        let nodes = query_nodes(tx, &source.id)?;
        for node in &nodes {
            let new_id = Uuid::new_v4().to_string();
            node_map.insert(node.id.clone(), new_id.clone());
            let mut new_node = node.clone();
            new_node.id = new_id;
            new_node.topology_id = new_topo_id.clone();
            new_node.insert(tx)?;
        }

        // 3. Copy volumes (remap node_id)
        let volumes = query_volumes(tx, &source.id)?;
        for vol in &volumes {
            let new_id = Uuid::new_v4().to_string();
            volume_map.insert(vol.id.clone(), new_id.clone());
            let mut new_vol = vol.clone();
            new_vol.id = new_id;
            new_vol.topology_id = new_topo_id.clone();
            new_vol.node_id = node_map[&vol.node_id].clone();
            new_vol.insert(tx)?;
        }

        // 4-7: Similar for datasets, placements, links, sync_regimes
        // ...

        // Record event
        record_event(tx, "topology.created", "topology", &new_topo_id,
            &format!("Forked topology '{}' from '{}'", name, source.name),
            None, Some(&new_topo.to_json()?),
            &EventSource::User)?;

        Ok(())
    })?;

    Ok(())
}
```

### Diff Engine Core

```rust
use serde_json::Value;

#[derive(Debug)]
enum DiffEntry {
    Added(String, Value),           // name, entity
    Removed(String, Value),         // name, entity
    Changed(String, Vec<FieldDiff>), // name, field changes
}

#[derive(Debug)]
struct FieldDiff {
    field: String,
    old_value: Value,
    new_value: Value,
}

fn diff_entities<T: Serialize>(
    left: &[(String, T)],   // (name, entity) from base topology
    right: &[(String, T)],  // (name, entity) from target topology
) -> Vec<DiffEntry> {
    let left_map: HashMap<&str, &T> = left.iter().map(|(n, e)| (n.as_str(), e)).collect();
    let right_map: HashMap<&str, &T> = right.iter().map(|(n, e)| (n.as_str(), e)).collect();

    let mut diffs = Vec::new();

    // Removed (in left, not in right)
    for (name, entity) in &left_map {
        if !right_map.contains_key(name) {
            diffs.push(DiffEntry::Removed(
                name.to_string(),
                serde_json::to_value(entity).unwrap(),
            ));
        }
    }

    // Added (in right, not in left)
    for (name, entity) in &right_map {
        if !left_map.contains_key(name) {
            diffs.push(DiffEntry::Added(
                name.to_string(),
                serde_json::to_value(entity).unwrap(),
            ));
        }
    }

    // Changed (in both, compare fields)
    for (name, left_entity) in &left_map {
        if let Some(right_entity) = right_map.get(name) {
            let left_json = serde_json::to_value(left_entity).unwrap();
            let right_json = serde_json::to_value(right_entity).unwrap();
            let field_diffs = diff_json_fields(&left_json, &right_json);
            if !field_diffs.is_empty() {
                diffs.push(DiffEntry::Changed(name.to_string(), field_diffs));
            }
        }
    }

    diffs
}

fn diff_json_fields(left: &Value, right: &Value) -> Vec<FieldDiff> {
    let mut diffs = Vec::new();
    // Skip id, topology_id, created_at, updated_at (metadata, not content)
    let skip_fields = ["id", "topology_id", "node_id", "created_at", "updated_at"];

    if let (Value::Object(l), Value::Object(r)) = (left, right) {
        for (key, left_val) in l {
            if skip_fields.contains(&key.as_str()) { continue; }
            if let Some(right_val) = r.get(key) {
                if left_val != right_val {
                    diffs.push(FieldDiff {
                        field: key.clone(),
                        old_value: left_val.clone(),
                        new_value: right_val.clone(),
                    });
                }
            }
        }
    }
    diffs
}
```

### Styled Diff Output

```rust
use console::style;

fn print_diff_text(label: &str, diffs: &[DiffEntry]) {
    if diffs.is_empty() {
        println!("  (no changes)");
        return;
    }

    println!("{}:", label);
    for diff in diffs {
        match diff {
            DiffEntry::Added(name, _) => {
                println!("  {} {}", style("+").green().bold(), style(name).green());
            }
            DiffEntry::Removed(name, _) => {
                println!("  {} {}", style("-").red().bold(), style(name).red());
            }
            DiffEntry::Changed(name, fields) => {
                println!("  {} {} [modified]", style("~").yellow().bold(), name);
                for f in fields {
                    println!("    {}: {} -> {}",
                        f.field,
                        style(&f.old_value).red(),
                        style(&f.new_value).green(),
                    );
                }
            }
        }
    }
}
```

### Tree Rendering

```rust
fn print_topology_tree(topologies: &[Topology]) {
    // Build parent -> children map
    let mut children: HashMap<Option<&str>, Vec<&Topology>> = HashMap::new();
    for topo in topologies {
        children
            .entry(topo.parent_id.as_deref())
            .or_default()
            .push(topo);
    }

    // Find roots (no parent)
    let roots = children.get(&None).cloned().unwrap_or_default();

    for root in &roots {
        print_tree_node(root, &children, "", true);
    }
}

fn print_tree_node(
    topo: &Topology,
    children: &HashMap<Option<&str>, Vec<&Topology>>,
    prefix: &str,
    is_last: bool,
) {
    let connector = if prefix.is_empty() { "" } else if is_last { "+-- " } else { "+-- " };
    let tag_str = topo.tag.as_ref()
        .map(|t| format!(" [{}]", t))
        .unwrap_or_default();

    println!("{}{}{}{}", prefix, connector, topo.name,
        style(tag_str).dim());

    let child_prefix = if prefix.is_empty() {
        "".to_string()
    } else if is_last {
        format!("{}    ", prefix)
    } else {
        format!("{}|   ", prefix)
    };

    if let Some(kids) = children.get(&Some(topo.id.as_str())) {
        for (i, kid) in kids.iter().enumerate() {
            let is_last_kid = i == kids.len() - 1;
            print_tree_node(kid, children, &child_prefix, is_last_kid);
        }
    }
}
```

## Recommended CLI Command Structure

### New Subcommands

```
sp topology fork <source> [--name <name>]     # Fork a topology
sp topology tag <name> <tag>                   # Tag: current, exploring, archived
sp topology untag <name>                       # Remove tag (set to NULL)
sp topology diff <target> [<base>]             # Diff two topologies
    [--nodes] [--volumes] [--datasets]         # Filter by entity type
    [--placements] [--links] [--syncs]
sp topology tree                               # Show fork tree of all topologies
sp topology log <name>                         # Show ancestry of one topology
```

### Modified Subcommands

```
sp topology list                               # Show tag inline instead of (active)
sp topology show <name>                        # Show parent/child info
sp topology set-active <name>                  # DEPRECATED: alias for 'tag <name> current'
```

### INFRA-03: Global --format Flag

The `--format` flag already exists on the `Cli` struct and is passed through to all commands. Phase 3 needs to ensure new commands (fork, tag, diff, tree, log) also accept and honor the format flag. The existing `OutputFormat` enum already supports Text and Json.

## Migration Plan

### Schema Migration v2

```sql
-- Migration v2: Topology tags replace is_active
ALTER TABLE topologies ADD COLUMN tag TEXT DEFAULT NULL;

-- Migrate existing data
UPDATE topologies SET tag = 'current' WHERE is_active = 1;

-- Drop old column (requires SQLite 3.35.0+, bundled rusqlite has 3.45+)
ALTER TABLE topologies DROP COLUMN is_active;

-- Create index for tag lookups
CREATE UNIQUE INDEX idx_topologies_current
    ON topologies(tag) WHERE tag = 'current';

PRAGMA user_version = 2;
```

The partial unique index `WHERE tag = 'current'` enforces the "only one current" constraint at the DATABASE level, not just application level. This is stronger than the current application-level enforcement.

### Code Changes Required

| File | Change | Impact |
|------|--------|--------|
| `src/core/db.rs` | Add Migration v2, bump CURRENT_VERSION to 2 | Schema upgrade |
| `src/core/models.rs` | Replace `is_active: bool` with `tag: Option<String>` on Topology | Model change |
| `src/core/resolve.rs` | Update `resolve_active_topology()` to query `WHERE tag = 'current'` | Critical path |
| `src/cli/topology.rs` | Add fork, tag, untag, diff, tree, log commands; update list/show | Main implementation |
| `src/cli/topology.rs` | Update `set_active` to use tag system (backward compat alias) | Migration |
| `src/cli/mod.rs` | No change needed (TopologyCommands enum expands internally) | Minimal |

## Discretion Recommendations Summary

| Decision Area | Recommendation | Rationale |
|---------------|---------------|-----------|
| Deep copy vs shallow | Deep copy | Shallow breaks independence of forked topologies |
| Auto-generated name | `{source}-fork-{N}` | Simple, discoverable, unique |
| Tags replace set-active | Yes, replace entirely | Eliminates confusing duplicate concepts |
| One tag per topology | One tag (TEXT column) | 3 fixed states, no benefit to multiple tags |
| Previous "current" when new set | Clear to NULL (untagged) | Matches existing set_active behavior |
| Archived hidden by default | No, show with [archived] label | Less surprising, user can always see everything |
| Diff presentation | Git-style with colors | Most familiar for CLI users |
| Fork depth limits | None | 2-5 topologies, depth is naturally shallow |
| topology show detail | Add "Forked from" and "Forks: N" | Minimal, useful context |

## Open Questions

1. **Should `set-active` be removed or kept as alias?**
   - Recommendation: Keep as alias that internally calls `tag <name> current` with a deprecation notice. This avoids breaking any existing scripts/habits from Phase 2. Can be removed in a future phase.

2. **Event recording for fork: single event or multiple?**
   - The fork creates 1 topology + N nodes + N volumes + ... Should this be 1 event (fork) or N+1 events?
   - Recommendation: Single event with event_type `topology.created` and the fork metadata in the summary. The fork is a single user action, and undo should undo the entire fork (delete the new topology, which cascades to delete all children). This matches the existing cascade delete behavior.

3. **Volume matching in diff: by (node_name, volume_name) or just volume_name?**
   - Must be (node_name, volume_name) because volume names are only unique within (topology, node), not globally. Same volume name on different nodes are different volumes.
   - Similar compound matching needed for placements: (dataset_name, node_name, volume_name)

## Sources

### Primary (HIGH confidence)
- Existing codebase: All files in `src/` read directly
- Schema: `src/core/db.rs` SCHEMA_V1 constant
- Models: `src/core/models.rs` Topology struct with parent_id field
- Resolve: `src/core/resolve.rs` resolve_active_topology() implementation
- Events: `src/core/events.rs` undo/redo system

### Secondary (MEDIUM confidence)
- console crate docs: https://docs.rs/console/0.15.11/console/ -- styling API confirmed
- SQLite DROP COLUMN support: Added in 3.35.0 (2021-03-12), rusqlite 0.31 bundles 3.45+

### Tertiary (LOW confidence)
- None -- all findings verified with codebase or official docs

## Metadata

**Confidence breakdown:**
- Schema migration: HIGH -- `parent_id` already exists, `ALTER TABLE ADD/DROP COLUMN` verified in bundled SQLite
- Fork implementation: HIGH -- straightforward deep copy with ID remapping, well-understood pattern
- Diff engine: HIGH -- simple set-difference with JSON comparison, no algorithmic complexity
- Tag system: HIGH -- simple column replacement, mirrors existing set_active pattern
- Tree/lineage display: HIGH -- standard tree rendering from parent_id relationships
- CLI structure: HIGH -- follows exact patterns established in Phase 2

**Research date:** 2026-02-07
**Valid until:** 2026-03-07 (stable domain, no external dependency changes expected)

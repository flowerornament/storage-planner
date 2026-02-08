# Continuation Prompt: Fix Post-Phase 7 Issues

You're picking up a Rust CLI tool called `sp` (storage planner) that helps users make purchase decisions for storage hardware. It models storage topologies (nodes, volumes, datasets, placements, links, sync regimes), tracks decisions with constraints, manages a product catalog, and provides analysis (redundancy, capacity, RPO, failure simulation, cost).

The project just completed 7 phases of development (21 plans, ~1.6 hours total execution). After the final phase, I did two rounds of hands-on testing and found **22 issues** ranging from critical data-loss bugs to cosmetic UX problems.

The full issue list is in `.planning/POST-PHASE7-ISSUES.md`. Read that first.

## What You Need to Know About This Codebase

### Structure

```
src/
├── main.rs                       # CLI entry point, clap derive structs
├── core/
│   ├── db.rs         (818 lines) # Database, migrations (SCHEMA_V1..V4), PRAGMA setup
│   ├── events.rs     (702 lines) # Event system, undo/redo engine ← CRITICAL FILE
│   ├── models.rs    (1000+ lines) # All entity structs (Topology, Node, Volume, Dataset, etc.)
│   ├── resolve.rs                # Entity resolution (name/UUID prefix lookup)
│   └── specs.rs                  # Capacity/speed/noise parsing
├── cli/
│   ├── prime.rs      (315 lines) # STATIC_GUIDE string + dynamic state section
│   ├── topology.rs  (1848 lines) # Topology CRUD, fork, diff, tag, tree, log
│   ├── node.rs       (696 lines) # Node CRUD with --item-id
│   ├── volume.rs     (676 lines) # Volume CRUD with --item-id
│   ├── dataset.rs    (673 lines) # Dataset CRUD
│   ├── catalog.rs    (560 lines) # Catalog items + price management
│   ├── decision.rs               # Decision lifecycle (create, consider, choose, etc.)
│   ├── analyze.rs                # All analysis commands
│   ├── status.rs                 # Status dashboard
│   └── ... (diagram, export, link, placement, sync_regime, undo, redo, init)
└── domains/storage/
    ├── analysis.rs               # Pure analysis functions (redundancy, capacity, RPO, etc.)
    └── models.rs                 # Analysis result types
```

### Key Patterns

**Entity model pattern** — Every entity (Topology, Node, Volume, etc.) has:
- `new()` → create with UUID
- `insert(&self, tx: &Transaction)` → INSERT INTO
- `from_row(row: &Row)` → SELECT result parsing
- `to_json(&self)` → serde_json::to_string

**Event system** — Every mutation records an event with `before_state` (JSON of entity before) and `after_state` (JSON after). Events have `event_type` like `topology.created`, `node.updated`, `dataset.deleted`.

**Undo/redo** — `undo_pointer` table has a single row tracking `current_sequence`. Undo decrements, redo increments. The undo handler in `events.rs:239-289` dispatches on the event_type suffix:
- `.created` → `delete_entity()` (delete by ID)
- `.deleted` → `restore_entity_from_json()` (deserialize + insert)
- `.updated` → `delete_entity()` + `restore_entity_from_json()` (delete then re-insert)

**D009 pattern** — Entity resolution happens OUTSIDE transactions. The resolved ID is passed INTO the transaction. This avoids SQLite borrow conflicts between `db.conn()` (for resolution) and the transaction.

**Schema migrations** — `SCHEMA_V1` through `SCHEMA_V4` in `db.rs`. Migrations are applied via `PRAGMA user_version`. V1 creates all tables, V2 adds `tag` column, V3 adds decisions, V4 adds catalog.

### Commands Reference (run to verify)

```bash
just check            # cargo fmt --check + clippy + test (97 tests)
just fmt              # cargo fmt
cargo build --release # builds ./target/release/sp
```

## The Issues (Prioritized)

### GROUP 1: Undo System (Critical — Fix First)

The undo system has a fundamental design flaw. For `.updated` events, it does `DELETE` + `INSERT` instead of `UPDATE`. The `DELETE` triggers `ON DELETE CASCADE` on FK relationships, destroying all child entities. The subsequent `INSERT` only restores the parent row.

**BUG-01: Undo of `.updated` events destroys cascaded children**

File: `src/core/events.rs`, lines 269-276:
```rust
} else if event.event_type.ends_with(".updated") {
    let before = event.before_state.as_deref()
        .ok_or_else(|| anyhow::anyhow!("No before_state for updated event"))?;
    delete_entity(tx, &event.entity_type, &event.entity_id)?;  // ← CASCADE!
    restore_entity_from_json(tx, &event.entity_type, before)?;  // ← only parent
}
```

The fix: Replace delete+insert with a field-level UPDATE. You need a new function like `update_entity_from_json(tx, entity_type, json_state)` that deserializes the JSON and runs `UPDATE {table} SET col1=?1, col2=?2, ... WHERE id=?N`. This must be done for each entity type (topology, node, volume, dataset, placement, link, sync_regime, decision, decision_constraint, decision_topology, catalog_item, price).

The same pattern exists in redo for `.updated` events (line 324-331) — fix both.

I tested this by:
1. Creating a topology, adding nodes/volumes/datasets
2. Forking it (creates a child with parent_id)
3. Running `sp current <name>` (creates topology.updated event)
4. Running `sp undo` → FK error blocks it entirely
5. Even without the fork, undoing any topology update (like tag/untag) would cascade-delete all nodes/volumes/datasets

**BUG-02: Undo of `.deleted` events doesn't restore cascaded children**

When you run `sp dataset remove X`, it cascade-deletes placements and sync_regimes. The event only captures the dataset's `before_state`. Undo restores the dataset but not the placements.

I tested: create dataset → add placement → remove dataset → undo → placement is gone.

This is harder to fix. Options:
- **Option A:** Record cascaded deletions as sub-events (most correct, most work)
- **Option B:** Before deleting an entity, query and store all dependent entities in a `cascade_state` JSON field on the event
- **Option C:** Document as known limitation

**BUG-03: `topologies.parent_id` FK missing ON DELETE action**

In `src/core/db.rs` line 152:
```sql
parent_id TEXT REFERENCES topologies(id),
```

Should be:
```sql
parent_id TEXT REFERENCES topologies(id) ON DELETE SET NULL,
```

This requires a migration (SCHEMA_V5). SQLite can't ALTER COLUMN, so you need to use the rename-recreate pattern:
1. Create `topologies_new` with correct FK
2. Copy data from `topologies`
3. Drop `topologies`
4. Rename `topologies_new` to `topologies`
5. Recreate indexes

The existing migration infrastructure in `db.rs` handles this — look at how `SCHEMA_V2` and `SCHEMA_V3` work. Add a `SCHEMA_V5` and increment `user_version`.

**UNDO-02: Failed undo leaves system stuck**

Once undo hits the FK error from BUG-03, every subsequent `sp undo` fails on the same event. There's no way to skip past it. If you fix BUG-01 and BUG-03, this goes away, but consider adding error handling that at minimum reports WHICH event is blocking.

### GROUP 2: Prime Guide (5 Wrong Examples)

File: `src/cli/prime.rs`, the `STATIC_GUIDE` constant (lines 14-114).

These 5 examples are wrong. Make exact string replacements:

1. **Line 52** — `--type=lan` should be `--connection-type=lan`:
   - Wrong: `sp link add --from=<source-node> --to=<target-node> --type=lan --bandwidth=1GB/s`
   - Right: `sp link add --from=<source-node> --to=<target-node> --connection-type=lan --bandwidth=1GB/s`

2. **Line 53** — `--type=rsync` should be `--sync-type=rsync`:
   - Wrong: `sp sync add <name> --dataset=<ds> --from=<vol> --to=<vol> --type=rsync`
   - Right: `sp sync add <name> --dataset=<ds> --from=<vol> --to=<vol> --sync-type=rsync`

3. **Line 75** — `--status=open` should be `--open`:
   - Wrong: `sp decision update "NAS Upgrade 2026" --status=open`
   - Right: `sp decision update "NAS Upgrade 2026" --open`

4. **Line 78** — `decision compare` doesn't exist, should be `analyze compare` or `analyze constraints`:
   - Wrong: `sp decision compare "NAS Upgrade 2026"`
   - Right: `sp analyze constraints --decision="NAS Upgrade 2026"`

5. **Line 79** — `--topology=<winner>` is a positional arg:
   - Wrong: `sp decision choose "NAS Upgrade 2026" --topology=<winner> --rationale="..."`
   - Right: `sp decision choose "NAS Upgrade 2026" <winner> --rationale="..."`

**Verify each one by running the corrected command against `--help` to confirm syntax.**

### GROUP 3: UX Fixes (Low Risk)

**UX-04: Dataset update bail message missing flag list**

File: `src/cli/dataset.rs` line 519:
```rust
bail!("Nothing to update. Provide at least one field to change.");
```

Should list the available flags like node.rs and volume.rs do. Check `sp dataset update --help` for the actual flag names and list them.

**UX-01: Price amount in cents**

`sp catalog price add --amount=549.99` fails. The help says "in cents" but this is a UX footgun. Consider either:
- Accept decimal dollars and multiply by 100 internally
- Or at minimum, improve the error message to say "amount must be in cents (e.g., 54999 for $549.99)"

**UX-02 + UX-03: Negative and zero values accepted**

`--power-draw=-100` stores -100W. `--capacity=-1TB` silently becomes 0B. `--capacity=0GB` creates a 0B volume. Add validation:
- Power draw, cost, noise, rack units: must be ≥ 0
- Capacity: must be > 0

These checks go in the respective `add()` and `update()` functions in `node.rs`, `volume.rs`.

**ANLZ-02: `$-0.00` formatting**

In `src/domains/storage/analysis.rs` or `src/cli/analyze.rs`, the compare command formats zero cost as `$-0.00`. This is likely a signed float formatting issue. Check for `-0.0` and normalize to `0.0`.

**UX-05 + UX-06: Compiler warnings**

Run `cargo build 2>&1 | grep warning` to see all dead code warnings. Either:
- Add `#[allow(dead_code)]` to intentionally-kept items
- Remove truly unused items
- Wire up unused functions

For clippy: `cargo clippy --fix --allow-dirty` may auto-fix the `print_literal` and `format_in_format_args` warnings.

### GROUP 4: Nice-to-Have (Document or Defer)

**ANLZ-01: Capacity analysis false warnings** — The analysis sums all placed dataset sizes as volume "used" bytes. This produces false "full in 0 months" warnings when dataset sizes exceed volume capacity. This is a design question more than a bug — the current approach is defensible but produces noisy output. Document the behavior or add a note to the output explaining the assumption.

**UNDO-01: Redo fails for import events** — `sp redo` after undoing a topology import fails with `Error: No after_state for created event`. The import creates the topology but may not populate `after_state` correctly. This existed before Phase 7.

**SCHEMA-02: Template export retains tag** — `sp export --template` strips UUIDs but keeps `tag: current`. Import of that template creates a topology with the current tag, which may conflict.

## How to Test

After each fix, run:

```bash
just check            # Must pass (97 tests, no fmt issues, clippy clean)
```

For undo fixes specifically, test this exact sequence:

```bash
rm -rf .sp
./target/release/sp init
./target/release/sp topology create test-topo
./target/release/sp topology tag test-topo current
./target/release/sp node add server1 --role=server
./target/release/sp volume add pool1 --node=server1 --capacity=4TB
./target/release/sp dataset add data1 --size=1TB --min-copies=2
./target/release/sp placement add data1 pool1 --role=primary
./target/release/sp topology fork test-topo --name=fork1

# This is the critical test — undo a topology update with children and a fork
./target/release/sp topology tag test-topo exploring
./target/release/sp undo
# MUST succeed. After undo:
./target/release/sp node list          # server1 must still exist
./target/release/sp volume list        # pool1 must still exist
./target/release/sp placement list     # data1→pool1 must still exist

# Test undo of deletion with cascade
./target/release/sp dataset remove data1
./target/release/sp undo
./target/release/sp placement list     # data1→pool1 should still exist (if BUG-02 is fixed)

# Test redo of update
./target/release/sp topology tag test-topo exploring
./target/release/sp undo
./target/release/sp redo               # MUST succeed, children intact
```

For prime fixes, test each corrected example:

```bash
./target/release/sp prime 2>/dev/null | grep -E "connection-type|sync-type|--open|analyze constraints|decision choose"
# Should show all 5 corrected examples
```

For duplicate errors (already fixed in Phase 7, just verify still working):

```bash
./target/release/sp topology create dup-test
./target/release/sp topology create dup-test    # Should say "already exists"
```

## Suggested Approach

1. **Start with SCHEMA_V5 migration** — Add `ON DELETE SET NULL` to `topologies.parent_id`. This unblocks the undo fix.
2. **Fix undo/redo for `.updated` events** — Write `update_entity_from_json()` that does UPDATE instead of delete+insert. Apply to both `undo()` and `redo()` in `events.rs`.
3. **Fix prime guide** — 5 string replacements in `STATIC_GUIDE`.
4. **Fix UX issues** — Dataset update message, negative value validation.
5. **Clean up warnings** — Dead code, clippy.

For the undo cascade issue (BUG-02, restoring cascaded children on undo of delete), I'd suggest documenting it as a known limitation rather than trying to solve it now. It requires either sub-events or a cascade snapshot, both of which are significant architectural changes to the event system. The `.updated` undo fix (BUG-01) is the critical one because it causes *silent* data loss — users don't even know children were destroyed.

## Key Decisions to Preserve

- D009: Resolve entities OUTSIDE transactions, pass resolved ID inside
- D018: Build complete after-state BEFORE SQL for accurate event logging
- D019: `tag` column replaces `is_active` boolean
- D020: Partial unique index `WHERE tag='current'` enforces single current at DB level

## Don't Forget

- Run `just check` after every change (fmt + clippy + test)
- Write tests for the new `update_entity_from_json()` function — the existing undo tests in `events.rs` only test `.created` events, not `.updated`
- The migration must handle existing databases (v4 → v5 upgrade path)
- Clean up `.sp/` test database when done: `rm -rf .sp`

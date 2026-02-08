# Phase 7: CLI Polish and Correctness - Research

**Researched:** 2026-02-08
**Domain:** Rust CLI correctness fixes (string literals, clap flags, rusqlite error handling, text formatting)
**Confidence:** HIGH

## Summary

This phase addresses four correctness issues discovered in post-v1 hands-on testing. All four are well-scoped, self-contained fixes to existing code with no new dependencies, no schema changes, and no architectural decisions required.

The fixes involve: (1) replacing 6 wrong command examples in a static string in `prime.rs`, (2) adding `--item-id` flags to 4 existing clap command structs in `node.rs` and `volume.rs` and wiring them to the existing `resolve_catalog_item` function, (3) catching `rusqlite::ErrorCode::ConstraintViolation` in `db.rs`'s `transaction` method (or in insert call-sites) and converting to user-friendly error messages, and (4) adding a header row to the `catalog list` text output.

**Primary recommendation:** Fix all four issues. They are independent and can be planned as separate tasks. The `--item-id` flag addition is the most impactful (unblocks cost analysis); the prime examples fix is the highest priority for agent usability.

## Standard Stack

No new dependencies required. All fixes use existing crate versions already in `Cargo.toml`:

### Core (already in use)
| Library | Version | Purpose | Relevance to Phase 7 |
|---------|---------|---------|----------------------|
| clap | 4 | CLI argument parsing | Add `--item-id` flags to existing structs |
| rusqlite | 0.31 | SQLite interface | Catch `ConstraintViolation` error codes |
| anyhow | 1 | Error handling | Convert SQLite errors to friendly messages |

### No New Dependencies
This phase requires zero new crates. All patterns needed are already demonstrated in the codebase.

## Architecture Patterns

### Existing Project Structure (no changes)
```
src/
├── cli/
│   ├── mod.rs            # Command dispatch
│   ├── prime.rs           # Fix 1: static STATIC_GUIDE string
│   ├── node.rs            # Fix 2a: add --item-id to Add/Update
│   ├── volume.rs          # Fix 2b: add --item-id to Add/Update
│   └── catalog.rs         # Fix 4: add header row to list
├── core/
│   ├── db.rs              # Fix 3: catch constraint violations in transaction()
│   ├── resolve.rs         # Existing resolve_catalog_item (used by Fix 2)
│   └── models.rs          # Node.item_id and Volume.item_id already exist
└── domains/storage/
    └── analysis.rs        # Cost analysis already reads item_id (benefits from Fix 2)
```

### Pattern 1: Adding a Flag to a Clap Struct

**What:** The established pattern for adding optional flags to existing commands.
**Confidence:** HIGH -- verified by reading 6 existing command files.

All entity update commands follow the same pattern:
1. Add `#[arg(long)]` field to the clap enum variant
2. Pass through the `run()` match arm via `.as_deref()`
3. Add to the `update()` function signature
4. Add to the "nothing to update" bail check
5. Add to the "after state" builder
6. Add a SQL UPDATE statement inside the transaction closure

**Example (from `node.rs` Update variant):**
```rust
// In NodeCommands::Update
/// Link to a catalog item for cost tracking
#[arg(long)]
item_id: Option<String>,

// In the update function:
if let Some(ref iid) = item_id {
    // Validate item exists
    resolve_catalog_item(db, iid)?;
}
// ... then in after-state builder:
if let Some(ref iid) = item_id {
    after.item_id = Some(iid.clone());
}
// ... then in transaction:
if let Some(ref iid) = item_id {
    tx.execute(
        "UPDATE nodes SET item_id = ?1, updated_at = datetime('now') WHERE id = ?2",
        params![iid, node_id],
    )?;
}
```

### Pattern 2: Entity Resolution for Validation

**What:** All entity references go through `resolve_*` functions in `core/resolve.rs`.
**Confidence:** HIGH -- every command file uses this pattern.

For the `--item-id` flag: the value should be resolved via `resolve_catalog_item(db, item_id_value)` to validate the item exists. Store the resolved item's `id` (not the user input), since the user might pass a name or UUID prefix.

**Important detail:** The resolve must happen BEFORE the transaction (following the existing pattern in all update functions where resolves are done outside the transaction).

### Pattern 3: Error Wrapping in transaction()

**What:** The `db.transaction()` method in `core/db.rs` propagates `anyhow::Result` errors from the closure.
**Confidence:** HIGH -- verified by reading `db.rs`.

```rust
pub fn transaction<T, F>(&mut self, f: F) -> Result<T>
where
    F: FnOnce(&Transaction) -> Result<T>,
{
    let tx = self.conn.transaction()?;
    let result = f(&tx)?;   // <-- errors from insert() surface here
    tx.commit()?;
    Ok(result)
}
```

The `insert()` methods on models return `rusqlite::Result<()>`. When a UNIQUE constraint fails, `rusqlite` returns `rusqlite::Error::SqliteFailure` with `ErrorCode::ConstraintViolation` (error code 2067). The `?` operator converts this to `anyhow::Error`, losing the structured error info. By the time it reaches the user, it's the raw "UNIQUE constraint failed: ..." text.

### Pattern 4: Text Output Formatting

**What:** Aligned columnar text output using format strings.
**Confidence:** HIGH -- verified by reading `catalog.rs` price_list and list functions.

The `catalog price list` command already has headers:
```rust
println!(
    "  {:<12} {:>10} {:<12} {:<12} {}",
    "Date", "Amount", "Source", "Condition", "Type"
);
```

The `catalog list` command uses the same column widths but no header row:
```rust
println!(
    "  {:<30} {:<12} {:<42} {}",
    item.name, item.category, url_str, price_str
);
```

Adding a header follows the exact same format string pattern.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Error type detection | Pattern matching on error strings | `rusqlite::Error::SqliteFailure` match | Structured error codes are reliable; string matching is fragile |
| Entity validation | Manual SQL queries for item existence | `resolve_catalog_item()` | Already handles name, UUID prefix, ambiguity detection |

**Key insight:** Every tool needed for these fixes already exists in the codebase. The only "new" code is wiring things together.

## Common Pitfalls

### Pitfall 1: Matching on Error Message Text Instead of Error Codes
**What goes wrong:** Catching constraint violations by matching on the string "UNIQUE constraint failed" instead of the structured `rusqlite::ErrorCode::ConstraintViolation`.
**Why it happens:** The `anyhow` wrapping erases the `rusqlite::Error` type, making it tempting to match on `.to_string()`.
**How to avoid:** Use `anyhow::Error::downcast_ref::<rusqlite::Error>()` to recover the typed error, then match on `ErrorCode`. Alternatively, catch the `rusqlite::Error` before the `?` converts it to `anyhow::Error`.
**Warning signs:** Code that does `.to_string().contains("UNIQUE")`.

### Pitfall 2: Not Resolving item_id Before Storing
**What goes wrong:** Storing the user's raw input (name or UUID prefix) as `item_id` instead of the resolved full UUID.
**Why it happens:** Forgetting that `--item-id` accepts names and prefixes, just like every other entity reference.
**How to avoid:** Call `resolve_catalog_item(db, input)` and store `resolved.id`, not the raw input.
**Warning signs:** `item_id` values that look like names instead of UUIDs.

### Pitfall 3: Forgetting to Update the "Nothing to Update" Check
**What goes wrong:** Adding `--item-id` to the Update variant but not adding it to the `if rename.is_none() && role.is_none() && ...` bail check.
**Why it happens:** The check is a manual enumeration of all optional fields.
**How to avoid:** Audit the "nothing to update" bail condition when adding any new optional field.
**Warning signs:** `sp node update mynode --item-id=...` produces "Nothing to update" error.

### Pitfall 4: Adding item_id to Add But Not Passing Through to Event State
**What goes wrong:** The `--item-id` value is set on the entity and inserted, but the `after_json` event state doesn't reflect it.
**Why it happens:** The `to_json()` call for event recording happens before or after the field is set.
**How to avoid:** Follow the existing pattern: set all fields on the entity struct, THEN call `to_json()`.
**Warning signs:** Undo of a node-add loses the item_id because the event's after_state didn't capture it.

### Pitfall 5: Constraint Error Scope Is Broader Than Expected
**What goes wrong:** The constraint violation handler catches errors it shouldn't, like FK violations (which should remain raw since they indicate bugs, not user errors).
**Why it happens:** `ErrorCode::ConstraintViolation` covers both UNIQUE and FK failures.
**How to avoid:** Additionally check the error message string to distinguish UNIQUE from FK violations. UNIQUE errors contain the table and column names. Alternatively, only apply the friendly wrapping at the call site where we know a UNIQUE constraint is the expected failure mode.
**Warning signs:** A FK violation (orphaned node_id) gets turned into a misleading "already exists" message.

## Code Examples

Verified patterns from the existing codebase:

### Fix 1: Prime Static Content Corrections (6 lines)

The `STATIC_GUIDE` constant in `src/cli/prime.rs` contains 6 incorrect command examples. The fix is purely mechanical string replacement. Here are the exact corrections needed (verified against the actual clap definitions):

| Line in STATIC_GUIDE | Wrong | Correct |
|---|---|---|
| Section 3 (Build Topology Content) | `sp placement add --dataset=<ds> --volume=<vol> --role=primary` | `sp placement add <ds> <vol> --role=primary` |
| Section 3 (Build Topology Content) | `sp link add <source-node> <target-node> --type=lan --bandwidth=1GB/s` | `sp link add --from=<source-node> --to=<target-node> --type=lan --bandwidth=1GB/s` |
| Section 3 (Build Topology Content) | `sp sync add <name> --dataset=<ds> --source=<vol> --target=<vol> --type=rsync` | `sp sync add <name> --dataset=<ds> --from=<vol> --to=<vol> --type=rsync` |
| Section 4 (Analyze Options) | `sp analyze compare <topo-a> <topo-b>` is actually **correct** -- the analyze Compare variant uses positional args `a: String` and `b: String` |
| Section 5 (Track Decisions) | `sp decision add-topology "NAS Upgrade 2026" <topology>` | `sp decision consider "NAS Upgrade 2026" <topology>` |
| Section 5 (Track Decisions) | `sp decision add-constraint "NAS Upgrade 2026" --type=budget --max=1500` | `sp decision constrain "NAS Upgrade 2026" --type=budget --max=1500` |

**CORRECTION to the assessment:** The assessment says `sp analyze compare --a=<t1> --b=<t2>` is wrong and should be `sp analyze compare <t1> <t2>`. But looking at the actual code, the prime output already says `sp analyze compare <topo-a> <topo-b>` (no `--a=` / `--b=` flags). And the clap definition shows `a: String` and `b: String` as positional. So the prime output for `analyze compare` is actually correct. **Only 5 examples are actually wrong**, not 6.

### Fix 2: Adding --item-id Flag

**For node add (new flag):**
```rust
// In NodeCommands::Add variant:
/// Link to a catalog item (name or ID prefix)
#[arg(long)]
item_id: Option<String>,
```

**For node update (new flag):**
```rust
// In NodeCommands::Update variant:
/// Link to a catalog item (name or ID prefix)
#[arg(long)]
item_id: Option<String>,
```

**Resolution in the add function:**
```rust
// After resolving topology but before creating the node:
let resolved_item_id = if let Some(ref iid) = item_id {
    let item = resolve_catalog_item(db, iid)?;
    Some(item.id)
} else {
    None
};
// Then: node.item_id = resolved_item_id;
```

**Same pattern for volume commands:** Volume already has `item_id: Option<String>` in the model and schema. Same approach as node.

**Imports needed:** `use crate::core::resolve::resolve_catalog_item;` -- already imported in `node.rs` resolve module but `resolve_catalog_item` specifically is not. Need to add it.

### Fix 3: Friendly Constraint Violation Errors

**Approach A (recommended): Catch at call-site using map_err**

Wrap `insert()` calls with `.map_err()` to catch and convert UNIQUE constraint errors:

```rust
// In catalog add:
db.transaction(|tx| {
    item.insert(tx).map_err(|e| {
        if is_unique_violation(&e) {
            anyhow::anyhow!("A catalog item named '{}' already exists", item_name)
        } else {
            anyhow::anyhow!(e)
        }
    })?;
    // ...
    Ok(())
})?;
```

**Helper function for detection:**
```rust
fn is_unique_violation(err: &rusqlite::Error) -> bool {
    matches!(
        err,
        rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error {
                code: rusqlite::ErrorCode::ConstraintViolation,
                ..
            },
            _
        )
    )
}
```

**Where to place the helper:** In `core/db.rs` or a new `core/errors.rs` -- the function is small enough for either.

**Approach B (alternative): Pre-check before insert**

Some commands already do this (e.g., `decision create` checks title uniqueness before insert). The advantage is a more precise error message. The disadvantage is a TOCTOU race (theoretical in single-user CLI). **Several commands already use pre-checks** -- adding them to the remaining commands would be consistent. But it doubles the query count. Since many commands already pre-check, expanding the pattern to the remaining insert paths may be the most consistent approach.

**Entities with UNIQUE constraints that need friendly errors:**
| Table | Unique Constraint | Scope |
|-------|------------------|-------|
| `topologies` | `name` | global |
| `nodes` | `(topology_id, name)` | per-topology |
| `volumes` | `(topology_id, node_id, name)` | per-node |
| `datasets` | `(topology_id, name)` | per-topology |
| `placements` | `(dataset_id, volume_id)` | global pair |
| `links` | `(topology_id, source_node_id, target_node_id)` | per-topology |
| `sync_regimes` | `(topology_id, name)` | per-topology |
| `catalog_items` | `name` | global |
| `decisions` | `title` | global |

**Which already have pre-checks:**
- `decisions.create` -- YES (checks title uniqueness)
- `placement.add` -- YES (checks dataset+volume pair)
- `link.add` -- YES (checks source+target pair)
- `sync_regime.add` -- YES (checks name uniqueness)
- `topology.create` -- NOT checked
- `node.add` -- NOT checked
- `volume.add` -- NOT checked
- `dataset.add` -- NOT checked
- `catalog.add` -- NOT checked

**Recommendation:** Add pre-insert uniqueness checks to the 5 commands that lack them (topology create, node add, volume add, dataset add, catalog add). This is more consistent with the existing pattern than the map_err approach, and gives more precise error messages.

### Fix 4: Catalog List Header Row

```rust
// Before the for loop in catalog list:
println!(
    "  {:<30} {:<12} {:<42} {}",
    "Name", "Category", "URL", "Latest Price"
);
println!("  {}", "-".repeat(90));
```

This matches the pattern from `catalog price list` which already has headers.

## State of the Art

Not applicable -- this phase fixes correctness issues in existing code, no technology migration involved.

## Open Questions

1. **Should `--item-id` accept a `--clear-item-id` for removing the association?**
   - What we know: Currently no way to unlink an entity from a catalog item once linked.
   - What's unclear: Whether this is needed in v1.
   - Recommendation: Defer to a future phase. Users can set item_id to a different item but can't clear it. This is fine for v1 since the primary workflow is linking, not unlinking.

2. **Should the constraint violation approach be call-site (map_err) or pre-check?**
   - What we know: 4 out of 9 insert paths already use pre-checks. None use map_err.
   - What's unclear: Whether map_err is more maintainable long-term.
   - Recommendation: Use pre-checks for consistency with existing code. The pre-check pattern is proven in 4 existing commands (decision create, placement add, link add, sync add).

## Sources

### Primary (HIGH confidence)
- Direct source code reading of all relevant files in `src/cli/`, `src/core/`, verified against the clap derive struct definitions
- Schema verified from `src/core/db.rs` SCHEMA constants

### Secondary (MEDIUM confidence)
- rusqlite error handling: `ErrorCode::ConstraintViolation` and `ffi::Error` struct are part of the public API in rusqlite 0.31 (verified from crate documentation)

## Metadata

**Confidence breakdown:**
- Fix 1 (prime examples): HIGH -- pure string replacement, verified each correction against clap structs
- Fix 2 (--item-id flag): HIGH -- follows exact pattern of 10+ existing flags across node/volume commands
- Fix 3 (constraint errors): HIGH -- the pre-check pattern is already used in 4 commands; extending it is mechanical
- Fix 4 (catalog headers): HIGH -- 2-line change following exact pattern from price_list

**Research date:** 2026-02-08
**Valid until:** 2026-03-10 (stable codebase, no external dependencies changing)

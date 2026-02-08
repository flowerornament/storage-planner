# Continuation Prompt: Post-Phase 8 Polish

You're picking up a Rust CLI tool called `sp` (storage planner) that helps users make purchase decisions for storage hardware. It models storage topologies (nodes, volumes, datasets, placements, links, sync regimes), tracks decisions with constraints, manages a product catalog, and provides analysis (redundancy, capacity, RPO, failure simulation, cost).

Start with CLAUDE.md, then `sp --help` and `sp prime` to orient.

## What Just Happened (Phase 8)

Phase 8 fixed the 16 highest-priority issues from POST-PHASE7-ISSUES.md:

- **Critical undo fix**: Replaced delete+insert with field-level UPDATE in undo/redo `.updated` handlers (`update_entity_from_json` in `src/core/events.rs`). This prevents ON DELETE CASCADE from destroying child entities.
- **SCHEMA_V5 migration**: Added `ON DELETE SET NULL` to `topologies.parent_id` via rename-recreate pattern (`src/core/db.rs`). CURRENT_VERSION is now 5.
- **Prime guide**: Fixed 5 wrong command examples in `STATIC_GUIDE` (`src/cli/prime.rs`).
- **Input validation**: Added non-negative checks for power_draw, cost, noise, rack_units in node add/update; capacity > 0 in volume add/update; negative price rejection.
- **UX messages**: Dataset update now lists available flags. Price help text clarified.
- **Warnings**: All dead code warnings silenced, all clippy warnings fixed. `just check` is fully clean.

102 tests pass (97 original + 5 new for undo/redo with children and forks).

## Open Issues

Run `bd list --status=open` to see all open issues. The 4 new ones from hands-on testing:

### 1. `sp current` misleading message (storage-planner-4mm, P3 bug)

After `sp topology create foo && sp current foo`, the output says "foo is already the current topology" — but it wasn't, it was just created with tag=NULL. The `sp current` command set the tag, so the message should reflect that ("Switched to 'foo'" or "Set 'foo' as current").

**File**: `src/cli/topology.rs` — find the `current` subcommand handler. It likely checks if the topology already has tag='current' and prints the "already" message, but the check is running after the tag was just set in the same flow, or the create command auto-sets it.

**Fix**: Check the tag BEFORE updating, and use different messages for "was already current" vs "now set as current".

### 2. Negative zero in non-cost compare metrics (storage-planner-5th, P3 bug)

`sp analyze compare` shows `Rack units: -0.0 U` for topologies with no rack_units set. The Phase 8 fix only normalized `-0.0` for the `total_cost` branch in `format_metric_values`. The same issue affects `total_noise`, `total_power`, and `total_rack_units`.

**File**: `src/cli/analyze.rs`, function `format_metric_values` (around line 1554).

**Fix**: Apply `if val == 0.0 { 0.0 } else { val }` normalization to both `a` and `b` at the top of the function, before the match, so all branches benefit. One-liner.

### 3. Capacity analysis false positives (storage-planner-f17, P4 feature)

Capacity analysis sums all placed dataset sizes as volume "used bytes." A 1.2TB dataset placed as a backup on a 1TB SSD reports "full in 0 months" even though the actual footprint may differ. This is ANLZ-01 from the original issue list.

**Minimum viable fix**: Add a note to capacity warning output explaining the assumption: "Note: capacity estimates assume each dataset occupies its full size on every placed volume."

**File**: `src/domains/storage/analysis.rs` (the `analyze_capacity` function) or `src/cli/analyze.rs` (where capacity results are printed).

### 4. JSON format audit (storage-planner-n53, P4 feature)

Most commands support `--format=json` but some may not. Audit every command and add json output where missing. Key candidates: `node show`, `volume show`, `dataset show`, `placement list`, `link list`, `sync list`, `topology diff`, `topology log`, `diagram`, `export`.

**Pattern**: Every CLI function already takes an `OutputFormat` enum. Check each subcommand's match arm — if it only has `OutputFormat::Text`, add `OutputFormat::Json` with a `serde_json::json!({...})` block.

## Key Patterns to Know

- **D009**: Entity resolution happens OUTSIDE transactions. Resolved IDs are passed IN.
- **Event system**: Every mutation records an event with before/after JSON state for undo/redo.
- **`update_entity_from_json`**: New in Phase 8 — does field-level UPDATE instead of delete+insert for undo/redo of `.updated` events. All 12 entity types covered.
- **Schema migrations**: `SCHEMA_V1` through `SCHEMA_V5` in `db.rs`. Use `PRAGMA user_version` tracking.

## Development

```bash
just check            # fmt + clippy + test (must pass, 102 tests)
just fmt              # cargo fmt
cargo build --release # builds ./target/release/sp
```

## Suggested Fix Order

1. **Negative zero** (5 min) — one-liner in format_metric_values
2. **Current message** (10 min) — fix the flow in topology.rs current handler
3. **Capacity note** (10 min) — add explanatory text to capacity output
4. **JSON audit** (30 min) — systematic pass through all subcommands

After each fix: `just check` must pass.

## Session Completion

```bash
bd close <completed-issues>    # close fixed issues
bd sync                        # sync beads
git add -A && git commit -m "..." && git push
```

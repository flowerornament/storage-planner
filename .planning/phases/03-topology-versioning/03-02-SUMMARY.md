---
phase: 03-topology-versioning
plan: 02
subsystem: cli
tags: [topology, fork, deep-copy, id-remapping, transaction]

# Dependency graph
requires:
  - phase: 03-topology-versioning
    plan: 01
    provides: "Tag-based lifecycle, topology model with parent_id and tag"
provides:
  - "Topology fork command with full deep copy and ID remapping"
  - "Auto-generated fork names ({source}-fork-{N})"
  - "All 6 child entity types deep-copied in dependency order"
  - "Single-transaction atomicity for fork operation"
affects: [03-03-diff-engine, 04-analysis-functions]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Deep copy with ID remapping: HashMap<OldId, NewId> for FK remapping across entity types"
    - "Block-scoped prepared statements to satisfy borrow checker before transaction"

key-files:
  created: []
  modified:
    - "src/cli/topology.rs"

key-decisions:
  - "D022: Fork starts untagged (tag=NULL) -- user decides lifecycle state after forking"
  - "D023: Block-scoped prepared statements resolve borrow checker conflict with D009 pattern"

patterns-established:
  - "Deep copy pattern: load all entities before transaction, process in dependency order, remap FKs via HashMaps"

# Metrics
duration: 3m 27s
completed: 2026-02-07
---

# Phase 3 Plan 2: Topology Fork Command Summary

**Deep copy fork command with ID remapping for all 6 entity types, auto-name generation, single-transaction atomicity**

## Performance

- **Duration:** 3m 27s
- **Started:** 2026-02-07T10:34:12Z
- **Completed:** 2026-02-07T10:37:39Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments
- Fork command (`sp topology fork <source> [--name <name>]`) creates complete independent deep copy
- All 6 child entity types copied: nodes, volumes, datasets, placements, links, sync_regimes
- ID remapping via HashMap ensures all FK references within fork point to fork-local entities
- Auto-name generation: `{source}-fork-{N}` with collision avoidance (tries up to 99 suffixes)
- Custom name with slug validation and uniqueness check
- Forked topology has parent_id pointing to source (enables "Forked from" display in show)
- Single transaction ensures atomicity (all-or-nothing)
- Fork event recorded for undo support

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement fork command with deep copy and ID remapping** - `ce859c4` (feat)

**Plan metadata:** TBD (docs: complete plan)

## Files Created/Modified
- `src/cli/topology.rs` - Added Fork subcommand, generate_fork_name helper, fork function with full deep copy

## Decisions Made
- D022: Fork starts untagged (tag=NULL). The fork is created with no lifecycle tag, letting the user decide whether to mark it as "exploring" or leave it untagged. This avoids assumptions about user intent.
- D023: Block-scoped prepared statements resolve borrow checker conflict. The D009 pattern (resolve outside transaction) creates prepared statements that hold immutable borrows on the connection, which conflicts with the mutable borrow needed for `db.transaction()`. Wrapping each query in a block with a local `let result = ...` variable ensures the statement is dropped before the transaction begins.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed borrow checker conflict with prepared statements**
- **Found during:** Task 1 (compiling fork function)
- **Issue:** The plan's code pattern of `let mut node_stmt = db.conn().prepare(...)` creates prepared statements that hold immutable borrows on `db.conn()`. When `db.transaction()` is called later, it needs a mutable borrow, causing E0502 "cannot borrow as mutable because also borrowed as immutable". Additionally, even with block scoping, the temporary `MappedRows` iterator from `query_map` held a borrow past the block boundary (E0597).
- **Fix:** Wrapped each entity query in a block `{}` and bound the result to a local variable (`let result = stmt.query_map(...).collect()?; result`) before returning from the block. This ensures both the prepared statement and the mapped rows iterator are dropped before the block exits.
- **Files modified:** src/cli/topology.rs
- **Verification:** `cargo test` passes all 46 tests, `cargo clippy` has no new warnings
- **Committed in:** ce859c4 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Required restructuring query pattern for Rust borrow checker. No scope change.

## Issues Encountered
None -- execution was straightforward after resolving the borrow checker issue.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Fork command enables the comparison workflow: fork -> modify -> diff (03-03)
- Parent_id tracking enables ancestry queries for diff engine
- Deep copy pattern established for any future entity duplication needs
- All 46 tests pass, no new clippy warnings

## Self-Check: PASSED

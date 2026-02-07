---
phase: 02-cli-scaffolding-and-basic-commands
plan: 02
subsystem: cli
tags: [clap, rusqlite, crud, node, volume, capacity-parsing, events]

# Dependency graph
requires:
  - phase: 01-schema-and-core-types
    provides: "Database schema, Node/Volume models, event system"
  - phase: 02-01
    provides: "Entity resolver, resolve_active_topology, resolve_node, resolve_volume, validate_slug"
provides:
  - "Full node CRUD: add, list, show, remove, update with event logging"
  - "Full volume CRUD: add, list, show, remove, update with capacity parsing"
  - "Volume disambiguation via --node flag"
  - "Inline volume display on node show"
affects: [02-03, 02-04, 03-analysis-engine]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "CRUD command pattern: resolve outside tx, mutate inside tx, event log, format output"
    - "Capacity parsing via Capacity::parse for human units (4TB, 500GB)"
    - "Node name lookup helper for volume display"

key-files:
  created: []
  modified:
    - src/cli/node.rs
    - src/cli/volume.rs

key-decisions:
  - "Node show displays inline volumes with formatted capacity"
  - "Volume list includes node name lookup for each volume"
  - "Volume remove warns about cascading placement deletes"
  - "Update commands build after-state for undo before executing SQL"

patterns-established:
  - "CRUD pattern: validate -> resolve topology -> resolve entity -> capture before_state -> transaction(mutate + event) -> format output"
  - "Node name lookup helper: node_name_for_id() for volume display context"

# Metrics
duration: 3min
completed: 2026-02-07
---

# Phase 2 Plan 2: Node and Volume CRUD Summary

**Full CRUD for nodes (5 commands) and volumes (5 commands) with capacity parsing, event logging, --node disambiguation, and JSON output**

## Performance

- **Duration:** 3m 15s
- **Started:** 2026-02-07T09:40:28Z
- **Completed:** 2026-02-07T09:43:43Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Node CRUD with add/list/show/remove/update, inline volume display on show
- Volume CRUD with capacity parsing (4TB, 500GB), --node disambiguation, placement cascade warnings
- All 10 commands support --format=json and --topology override
- All mutations log events with before/after state for undo/redo

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement node CRUD commands** - `a6dea37` (feat)
2. **Task 2: Implement volume CRUD commands** - `779eaad` (feat)

## Files Created/Modified
- `src/cli/node.rs` - Full node CRUD: add, list, show (with inline volumes), remove (cascade warning), update (562 lines)
- `src/cli/volume.rs` - Full volume CRUD: add (capacity parsing), list (--node filter), show, remove (placement cascade), update (632 lines)

## Decisions Made
- Node show displays volumes inline with formatted capacity, filesystem, and RAID level
- Volume list shows node name alongside each volume for context
- Volume remove counts and warns about dependent placements before deleting
- Update commands build complete after-state struct before SQL execution for accurate event logging
- Volume name uniqueness is checked within (topology, node) scope, not global

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed Rust lifetime issue in volume list**
- **Found during:** Task 2 (volume list implementation)
- **Issue:** `stmt` dropped at end of if/else arm while `query_map` temporary still borrowed
- **Fix:** Assigned query result to local variable before returning from arm
- **Files modified:** src/cli/volume.rs
- **Verification:** cargo build passes
- **Committed in:** 779eaad (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Trivial Rust borrow checker fix. No scope creep.

## Issues Encountered
None beyond the lifetime fix documented above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Node and volume CRUD complete, ready for dataset/placement/link/sync commands
- Entity resolver patterns established and proven working
- Event logging enables undo/redo for all node and volume mutations

## Self-Check: PASSED

---
*Phase: 02-cli-scaffolding-and-basic-commands*
*Completed: 2026-02-07*

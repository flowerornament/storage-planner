---
phase: 07-cli-polish-and-correctness
plan: 01
subsystem: cli
tags: [clap, prime, node, volume, catalog-item, resolve]

# Dependency graph
requires:
  - phase: 06-status-prime-current
    provides: "prime command, catalog system, resolve infrastructure"
  - phase: 03-entity-crud-validation
    provides: "node/volume add/update commands, resolve_catalog_item"
provides:
  - "Corrected sp prime STATIC_GUIDE with 5 fixed command examples"
  - "Node add/update --item-id flag for catalog item linking"
  - "Volume add/update --item-id flag for catalog item linking"
affects: [07-02, analyze-cost, agent-bootstrap]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "resolve_catalog_item before transaction (D009 pattern) in node/volume commands"

key-files:
  created: []
  modified:
    - "src/cli/prime.rs"
    - "src/cli/node.rs"
    - "src/cli/volume.rs"

key-decisions: []

patterns-established:
  - "--item-id flag pattern: resolve outside tx, store UUID, set before to_json for event fidelity"

# Metrics
duration: 4m 21s
completed: 2026-02-08
---

# Phase 7 Plan 1: Prime Corrections and Item-ID Linking Summary

**Fixed 5 incorrect command examples in sp prime and added --item-id flag to node/volume add/update for catalog item linking**

## Performance

- **Duration:** 4m 21s
- **Started:** 2026-02-08T09:34:11Z
- **Completed:** 2026-02-08T09:38:32Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Corrected all 5 wrong command examples in sp prime static guide (placement add, link add, sync add, decision consider, decision constrain)
- Added --item-id flag to node add, node update, volume add, and volume update commands
- Item resolution validates catalog item existence before storage and stores resolved UUID
- Event after_state captures item_id for undo/redo fidelity

## Task Commits

Each task was committed atomically:

1. **Task 1: Fix 5 incorrect command examples in STATIC_GUIDE** - `b70dd51` (fix)
2. **Task 2: Add --item-id flag to node and volume add/update commands** - `da9ca4c` (feat)

## Files Created/Modified
- `src/cli/prime.rs` - Corrected 5 command examples in STATIC_GUIDE constant
- `src/cli/node.rs` - Added --item-id to Add/Update enums, resolve_catalog_item import, resolution in add/update functions
- `src/cli/volume.rs` - Added --item-id to Add/Update enums, resolve_catalog_item import, resolution in add/update functions

## Decisions Made
None - followed plan as specified.

## Deviations from Plan
None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- sp prime now shows correct syntax for all commands -- agent bootstrap is accurate
- --item-id flag enables the catalog-to-topology linking needed for sp analyze cost to be useful
- Ready for 07-02 (volume update fix and undo integration)

---
*Phase: 07-cli-polish-and-correctness*
*Completed: 2026-02-08*

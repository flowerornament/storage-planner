---
phase: 05-decision-integration
plan: 01
subsystem: database
tags: [sqlite, migration, models, undo-redo, entity-resolver, clap]

# Dependency graph
requires:
  - phase: 04-analysis-functions
    provides: "Analysis engine and existing schema v2"
provides:
  - "Schema v3 with decisions, decision_constraints, decision_topologies tables"
  - "Decision, DecisionConstraint, DecisionTopology model structs"
  - "Event system undo/redo support for decision entity types"
  - "resolve_decision function for title/UUID lookup"
  - "Node fields: cost_estimate, noise_db, rack_units with CLI flags"
affects: [05-decision-integration, 06-cost-and-context]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Decision titles allow spaces/special chars (no slug validation)"
    - "CASCADE delete from decisions to constraints and topologies"

key-files:
  created: []
  modified:
    - src/core/db.rs
    - src/core/models.rs
    - src/core/events.rs
    - src/core/resolve.rs
    - src/cli/node.rs
    - src/cli/topology.rs
    - src/cli/analyze.rs

key-decisions:
  - "D031: Decision titles use free-text (not slugs) -- supports natural language naming"
  - "D032: No power_watts column added -- existing power_draw_watts covers same concept"

patterns-established:
  - "Decision entity pattern: title-based resolution (not slug-based like topology entities)"
  - "Optional node metadata: cost_estimate, noise_db, rack_units all nullable with CLI flags"

# Metrics
duration: 5min
completed: 2026-02-08
---

# Phase 5 Plan 1: Schema Migration and Decision Models Summary

**Schema v3 migration adding decision tracking tables, model structs with undo/redo support, title-based entity resolver, and node cost/noise/rack-unit extensions**

## Performance

- **Duration:** 5m 29s
- **Started:** 2026-02-08T01:17:07Z
- **Completed:** 2026-02-08T01:22:36Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- Schema migration v3 creates 3 new tables (decisions, decision_constraints, decision_topologies) and 3 new node columns (cost_estimate, noise_db, rack_units)
- Decision, DecisionConstraint, DecisionTopology structs with full new/insert/from_row/to_json methods following existing entity patterns
- Event system handles undo/redo for all 3 new decision entity types
- resolve_decision function supports title match and UUID prefix lookup
- Node CLI extended with --cost, --noise, --rack-units flags on both add and update commands

## Task Commits

Each task was committed atomically:

1. **Task 1: Schema migration v3 and decision model structs** - `12a525b` (feat)
2. **Task 2: Event system registration, entity resolver, and node CLI extensions** - `dfb5903` (feat)

## Files Created/Modified
- `src/core/db.rs` - Schema v3 migration with 3 tables, 3 node columns, 5 indexes; migration test
- `src/core/models.rs` - Decision, DecisionConstraint, DecisionTopology structs; Node field extensions; roundtrip tests
- `src/core/events.rs` - Entity table name mapping and restore_entity_from_json for 3 new types
- `src/core/resolve.rs` - resolve_decision function with title/UUID prefix; 3 resolver tests
- `src/cli/node.rs` - --cost, --noise, --rack-units flags on Add/Update; show output display
- `src/cli/topology.rs` - Updated Node SELECT queries for new columns
- `src/cli/analyze.rs` - Updated Node SELECT queries for new columns

## Decisions Made
- D031: Decision titles use free-text (not slugs) -- supports natural language naming like "NAS Upgrade 2026"
- D032: No power_watts column added -- existing power_draw_watts already covers same concept per research doc

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All decision tables and models in place for plan 05-02 (Decision CLI CRUD commands)
- Entity resolver ready for decision command argument parsing
- Event system ready for undo/redo of decision mutations
- Node field extensions ready for constraint checking in plan 05-03

## Self-Check: PASSED

- FOUND: src/core/db.rs
- FOUND: src/core/models.rs
- FOUND: commit 12a525b (Task 1)
- FOUND: commit dfb5903 (Task 2)

---
*Phase: 05-decision-integration*
*Completed: 2026-02-08*

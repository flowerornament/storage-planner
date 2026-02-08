---
phase: 06-cost-and-context
plan: 01
subsystem: database
tags: [sqlite, migration, catalog, pricing, serde-yaml, undo-redo]

# Dependency graph
requires:
  - phase: 05-decision-integration
    provides: schema v3 with decisions tables, event system with undo/redo
provides:
  - schema v4 with catalog_items and prices tables
  - CatalogItem and Price model structs with full CRUD lifecycle
  - Event system undo/redo support for catalog_item and price entities
  - Entity resolver for catalog items (name or UUID prefix)
  - nodes.item_id column for direct catalog association
  - serde_yaml_ng dependency
affects: [06-cost-and-context]

# Tech tracking
tech-stack:
  added: [serde_yaml_ng 0.10]
  patterns: [catalog-item-price-pattern, global-entity-resolver]

key-files:
  created: []
  modified:
    - src/core/db.rs
    - src/core/models.rs
    - src/core/events.rs
    - src/core/resolve.rs
    - Cargo.toml
    - src/cli/topology.rs
    - src/cli/node.rs
    - src/cli/analyze.rs
    - src/cli/decision.rs
    - src/cli/export.rs

key-decisions:
  - "nodes.item_id is nullable TEXT with no FK constraint (same pattern as volumes.item_id per D003)"

patterns-established:
  - "Global entity resolver: catalog items use resolve_catalog_item like decisions (not topology-scoped)"
  - "Price amounts stored as integer cents with amount_dollars() helper for display"

# Metrics
duration: 6min
completed: 2026-02-08
---

# Phase 6 Plan 1: Schema Migration v4 and Catalog Models Summary

**Schema v4 with catalog_items/prices tables, CatalogItem and Price model structs, event system registration, and entity resolver for catalog items**

## Performance

- **Duration:** 6m 37s
- **Started:** 2026-02-08T05:22:54Z
- **Completed:** 2026-02-08T05:29:31Z
- **Tasks:** 2
- **Files modified:** 10

## Accomplishments
- Schema migration v4 creates catalog_items and prices tables with indexes and FK cascade
- CatalogItem and Price model structs follow established entity patterns (new/insert/from_row/to_json)
- Event system supports undo/redo for both new entity types
- Entity resolver disambiguates catalog items by name or UUID prefix
- Added nodes.item_id column for direct catalog association
- Added serde_yaml_ng 0.10 dependency for YAML import/export

## Task Commits

Each task was committed atomically:

1. **Task 1: Schema migration v4 and catalog model structs** - `93ae4a8` (feat)
2. **Task 2: Event system registration and entity resolver** - `ed7e5be` (feat)

## Files Created/Modified
- `Cargo.toml` - Added serde_yaml_ng dependency
- `src/core/db.rs` - Migration v4 with catalog_items, prices tables, nodes.item_id column
- `src/core/models.rs` - CatalogItem and Price structs; Node.item_id field
- `src/core/events.rs` - catalog_item and price entity type registration for undo/redo
- `src/core/resolve.rs` - resolve_catalog_item function for name/UUID prefix lookup
- `src/cli/topology.rs` - Updated Node SELECT queries to include item_id
- `src/cli/node.rs` - Updated Node SELECT queries to include item_id
- `src/cli/analyze.rs` - Updated Node SELECT queries to include item_id
- `src/cli/decision.rs` - Updated Node SELECT queries to include item_id
- `src/cli/export.rs` - Updated Node SELECT queries to include item_id

## Decisions Made
- nodes.item_id is nullable TEXT with no FK constraint, same pattern as volumes.item_id (per D003)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Updated all Node SELECT queries across CLI modules to include item_id**
- **Found during:** Task 1 (adding item_id to Node struct)
- **Issue:** Node::from_row now expects item_id column, but CLI queries in topology.rs, node.rs, analyze.rs, decision.rs, export.rs did not include it
- **Fix:** Added item_id to all Node SELECT queries across 5 CLI files
- **Files modified:** src/cli/topology.rs, src/cli/node.rs, src/cli/analyze.rs, src/cli/decision.rs, src/cli/export.rs
- **Verification:** All 92 tests pass
- **Committed in:** 93ae4a8 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Essential for correctness -- Node::from_row requires all columns present in SELECT.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Catalog and pricing foundation complete, ready for CLI commands (Plan 02)
- Event system handles undo/redo for both new entity types
- Entity resolver ready for catalog item resolution in CLI commands
- serde_yaml_ng available for YAML import/export features

---
*Phase: 06-cost-and-context*
*Completed: 2026-02-08*

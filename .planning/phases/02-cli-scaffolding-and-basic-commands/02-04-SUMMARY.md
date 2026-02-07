---
phase: 02
plan: 04
subsystem: cli-relationships
tags: [link, sync-regime, crud, bandwidth-parsing, event-logging]
depends_on:
  requires: ["01-01", "01-02", "02-01"]
  provides: ["link-crud", "sync-regime-crud", "relationship-commands"]
  affects: ["03-*", "04-*"]
tech-stack:
  added: []
  patterns: ["immutable-entity-crud", "auto-naming", "join-resolution", "direction-validation"]
key-files:
  created: []
  modified:
    - src/cli/link.rs
    - src/cli/sync_regime.rs
decisions: []
metrics:
  duration: "2m 33s"
  completed: "2026-02-07"
---

# Phase 2 Plan 4: Link and Sync Regime CRUD Summary

Full CRUD for network links and data sync regimes -- the relationship entities connecting nodes and defining data movement patterns.

## Task Commits

| Task | Name | Commit | Key Changes |
|------|------|--------|-------------|
| 1 | Implement link CRUD commands | 2f0646c | add/list/show/remove with bandwidth parsing, auto-naming, event logging |
| 2 | Implement sync regime CRUD commands | d4e7ab0 | add/list/show/remove with entity resolution, direction validation, event logging |

## What Was Built

### Link Commands (src/cli/link.rs)
- **add**: Creates link between two nodes with connection type, bandwidth, latency, metered flag, cost-per-gb. Parses bandwidth via Speed::parse (e.g., "1GB/s", "100MB/s"). Auto-names as source--target. Prevents self-links and duplicate links.
- **list**: Shows all links with JOINed source/target node names, formatted bandwidth display.
- **show**: Parses source--target name format, resolves both nodes, displays all link details.
- **remove**: Warns about sync regimes using volumes on the linked nodes before deletion.

### Sync Regime Commands (src/cli/sync_regime.rs)
- **add**: Creates sync regime with dataset, source/target volumes (with --from-node/--to-node disambiguation), sync type, schedule, and direction. Validates direction (push/pull/bidirectional). Validates slug name. Prevents duplicates.
- **list**: JOINs across sync_regimes, datasets, volumes, and nodes to show fully resolved entity names in format: name [type] dataset: node:vol -> node:vol.
- **show**: Finds by name or UUID prefix (4-char min), resolves all entity names for display.
- **remove**: Deletes with event logging for undo/redo support.

### Common Patterns
- All mutations record events (link.created/deleted, sync_regime.created/deleted) for undo/redo
- All commands support --format=json and --topology override
- Entity resolution follows established resolve_node/resolve_volume/resolve_dataset patterns
- Immutable design: no update commands, delete-and-recreate to change

## Deviations from Plan

None -- plan executed exactly as written.

## Decisions Made

No new architectural decisions. Followed established patterns from 02-01.

## Verification Results

- cargo build: Compiles successfully (no new warnings)
- cargo test: All 45 tests pass
- cargo clippy: No new warnings (all warnings are pre-existing dead code from other modules)

## Next Phase Readiness

Phase 2 CLI scaffolding is now feature-complete for relationship entities. All 7 topology entities (topology, node, volume, dataset, placement, link, sync_regime) have CRUD commands wired up and functional.

## Self-Check: PASSED

---
phase: 01-schema-and-core-types
plan: 02
subsystem: cli-and-events
tags: [rust, clap, undo-redo, event-sourcing, cli]
dependency-graph:
  requires: ["01-01"]
  provides: ["event-system", "cli-scaffold", "topology-crud", "undo-redo"]
  affects: ["02-01", "02-02"]
tech-stack:
  added: []
  patterns: ["event-sourced-undo-redo", "clap-subcommand-dispatch", "before-after-state-capture"]
file-tracking:
  key-files:
    created:
      - src/cli/init.rs
      - src/cli/topology.rs
      - src/cli/node.rs
      - src/cli/volume.rs
      - src/cli/dataset.rs
      - src/cli/link.rs
      - src/cli/sync_regime.rs
      - src/cli/undo.rs
      - src/cli/redo.rs
    modified:
      - src/core/events.rs
      - src/cli/mod.rs
decisions:
  - id: D006
    description: "Generic undo handler using event_type suffix (.created/.deleted/.updated) to determine reversal action"
  - id: D007
    description: "set-active undo restores target topology state but does not re-activate previously-active topology (known Phase 1 limitation)"
  - id: D008
    description: "Placeholder commands define full arg structure for Phase 2 but print stub message"
metrics:
  duration: "5m 5s"
  completed: "2026-02-07"
---

# Phase 1 Plan 2: CLI Scaffold with Event System Summary

**One-liner:** Event-sourced undo/redo engine with full CLI scaffold -- topology CRUD works end-to-end with multi-level undo/redo.

## What Was Done

### Task 1: Event System with Undo/Redo Engine

Replaced the stub `src/core/events.rs` with a complete event system:

- **EventSource enum** (User, Agent, Import, Migration) with Display/FromStr
- **record_event()** -- assigns sequence numbers, clears redo stack on new action after undo, updates undo_pointer
- **undo()** -- reverses last action based on event_type suffix (.created = delete entity, .deleted = restore from before_state, .updated = restore from before_state)
- **redo()** -- re-applies undone action with inverse suffix logic
- **Helpers** -- restore_entity_from_json (deserialize + insert for all 7 entity types), delete_entity, entity_table_name, current_actor
- **10 tests** covering: record, undo create, redo after undo, multi-level undo/redo, redo stack clearing, nothing-to-undo, nothing-to-redo

### Task 2: CLI Scaffold with Topology CRUD

Rewrote `src/cli/mod.rs` and created 9 new subcommand files:

- **sp init** -- creates .sp/decisions.db, idempotent (reports if exists)
- **sp topology create/list/show/set-active/delete** -- full CRUD with event logging, JSON output support, auto-activates first topology
- **sp undo / sp redo** -- dispatch to events module
- **sp node/volume/dataset/link/sync** -- placeholder subcommands with full arg definitions for Phase 2

## Task Commits

| Task | Name | Commit | Key Files |
|------|------|--------|-----------|
| 1 | Event system with undo/redo engine | c8f6afd | src/core/events.rs |
| 2 | CLI scaffold with topology CRUD and undo/redo | c7c1cc7 | src/cli/mod.rs, src/cli/topology.rs, src/cli/init.rs, +7 more |

## Verification Results

All verification criteria met:

1. **CLI help**: `sp --help` lists all 9 commands (init, topology, node, volume, dataset, link, sync, undo, redo)
2. **Database creation**: `sp init` creates `.sp/decisions.db`
3. **Schema verification**: all 9 tables + undo_pointer present
4. **Migration tracking**: `PRAGMA user_version` returns 1
5. **Topology CRUD**: create, list, show, set-active, delete all work
6. **Event logging**: events table records event_type, summary, before/after state
7. **Undo/Redo cycle**: create->undo->list empty->redo->list shows topology; multi-level undo/redo; redo stack cleared on new action
8. **No old commands**: `sp item` correctly fails with unrecognized subcommand
9. **All tests**: 34 tests passing (10 events + 24 from plan 01-01)

## Decisions Made

- **D006**: Generic undo handler using event_type suffix (.created/.deleted/.updated) to determine reversal action. Simple, extensible pattern.
- **D007**: set-active undo restores the target topology's state but doesn't re-activate the previously-active topology. Known Phase 1 limitation -- the "deactivate all" SQL side effect is not captured in the single-event model.
- **D008**: Placeholder commands define the full arg structure (names, types, help text) for Phase 2 but print "Coming in Phase 2" for execution.

## Deviations from Plan

None -- plan executed exactly as written.

## Next Phase Readiness

Phase 1 is now complete. The foundation is ready for Phase 2 (Node/Volume/Dataset CRUD):

- All model structs with new/insert/from_row/to_json
- Database layer with migrations and transactions
- Event system with undo/redo
- CLI scaffold with placeholder commands ready to be implemented
- 34 tests providing regression safety

## Self-Check: PASSED

---
phase: 02
plan: 03
subsystem: cli-dataset-placement
tags: [rust, clap, sqlite, crud, datasets, placements, events]
dependency-graph:
  requires: [02-01]
  provides: [dataset-crud, placement-crud, data-side-topology-modeling]
  affects: [02-04, 03, 04]
tech-stack:
  added: []
  patterns: [entity-resolve-before-transaction, criticality-validation, role-validation, inline-placement-display, cascading-delete-with-count]
key-files:
  created: []
  modified:
    - src/cli/dataset.rs
    - src/cli/placement.rs
decisions:
  - id: D013
    summary: "Criticality validation rejects non-standard values (only normal/important/critical)"
  - id: D014
    summary: "Placement role validation (only primary/replica/backup/archive)"
  - id: D015
    summary: "Dataset show displays inline placements via JOIN with volumes and nodes"
metrics:
  duration: "2m 42s"
  completed: "2026-02-07"
---

# Phase 2 Plan 3: Dataset and Placement CRUD Summary

Full CRUD for datasets (add/list/show/remove/update) and placements (add/list/remove) -- the "data" side of topology modeling.

## One-liner

Dataset and placement CRUD with size parsing, criticality/role validation, inline placement display, and event-sourced undo support.

## Task Commits

| Task | Name | Commit | Key Changes |
|------|------|--------|-------------|
| 1 | Implement dataset CRUD commands | 6f87294 | 5 subcommands: add, list, show, remove, update |
| 2 | Implement placement commands | d08e29e | 3 subcommands: add, list, remove |

## What Was Built

### Dataset Commands (src/cli/dataset.rs)

- **add**: Creates dataset with Capacity::parse() for size, optional growth rate, criticality validation (normal/important/critical), min_copies, min_locations, max_rpo_hours
- **list**: Shows all datasets in active topology with formatted size and copy count
- **show**: Full detail view including inline placements resolved via JOIN with volumes and nodes tables
- **remove**: Deletes dataset with cascade counting (reports how many placements and sync regimes were cascaded)
- **update**: Dynamic field updates -- rename, size, criticality, copies, locations, RPO, growth rate. Validates uniqueness on rename.

### Placement Commands (src/cli/placement.rs)

- **add**: Places dataset on volume with role (primary/replica/backup/archive) and priority. Validates no duplicate placement (unique dataset_id + volume_id). Uses --node for volume disambiguation.
- **list**: Shows all placements in topology with resolved names via 4-way JOIN (placements, datasets, volumes, nodes)
- **remove**: Removes placement by dataset+volume lookup

### Cross-cutting

- All mutations log events (dataset.created/deleted/updated, placement.created/deleted) for full undo/redo support
- All commands support --format=json and --topology override
- Follows resolve-outside-transaction pattern (D009)
- Slug validation on dataset names (D010)

## Decisions Made

- **D013**: Criticality values are enum-like: only "normal", "important", "critical" accepted
- **D014**: Placement roles are enum-like: only "primary", "replica", "backup", "archive" accepted
- **D015**: Dataset show command joins placements->volumes->nodes for inline display rather than separate query commands

## Deviations from Plan

None -- plan executed exactly as written.

## Verification Results

- cargo build: PASS (0 errors, pre-existing warnings only in unrelated modules)
- cargo test: PASS (45/45 tests)
- cargo clippy: PASS (0 errors in modified files)

## Next Phase Readiness

- Dataset and placement commands ready for use in topology modeling workflows
- Sync regime commands (Plan 04) can build on dataset/volume/placement resolution patterns
- Link commands (Plan 04) follow the same entity resolver patterns

## Self-Check: PASSED

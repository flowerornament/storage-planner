---
phase: 03-topology-versioning
plan: 03
subsystem: cli
tags: [topology, diff, tree, log, lineage, comparison, console-styling]

# Dependency graph
requires:
  - phase: 03-topology-versioning
    plan: 02
    provides: "Fork command with deep copy, parent_id tracking"
provides:
  - "Topology diff command with entity-level and field-level change detection"
  - "Entity type filtering via --nodes, --volumes, --datasets, --placements, --links, --syncs flags"
  - "Implicit base topology (uses current when base omitted)"
  - "Topology tree command showing fork hierarchy with tags"
  - "Topology log command showing ancestry chain with 'you are here' marker"
  - "JSON output mode for all new commands"
affects: [04-analysis-functions]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Entity diff by name matching: HashMap<display_key, serde_json::Value> for field-level comparison"
    - "Compound keys for volume matching: node_name/volume_name ensures correct cross-topology comparison"
    - "DIFF_SKIP_FIELDS constant to exclude metadata fields (id, topology_id, etc.) from comparison"
    - "console::style for colored terminal output (green=added, red=removed, yellow=modified)"

key-files:
  created: []
  modified:
    - "src/cli/topology.rs"

key-decisions:
  - "D024: Diff matches entities by display name (not UUID) with compound keys for volumes and placements"
  - "D025: DIFF_SKIP_FIELDS excludes id, topology_id, and all FK fields from comparison"

patterns-established:
  - "Entity comparison pattern: load entities as (display_key, json_value) pairs, diff by key matching"
  - "Section-based diff output with summary counts"

# Metrics
duration: 4m 04s
completed: 2026-02-07
---

# Phase 3 Plan 3: Topology Diff, Tree, and Log Commands Summary

**Field-level diff engine with entity type filtering, fork tree visualization, and ancestry log with colored terminal output**

## Performance

- **Duration:** 4m 04s
- **Started:** 2026-02-07T10:40:50Z
- **Completed:** 2026-02-07T10:44:54Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments
- Diff command (`sp topology diff <target> [<base>]`) compares two topologies showing entity-level and field-level changes
- Six entity types compared: nodes, volumes, datasets, placements, links, sync regimes
- Entity matching by display name with compound keys: volumes use `node_name/volume_name`, placements use `dataset_name on node_name/volume_name`, links use `source_name -> target_name`
- Implicit base: when only target specified, uses current topology as base
- Filter flags: `--nodes`, `--volumes`, `--datasets`, `--placements`, `--links`, `--syncs` (any combination)
- Git-style colored output: green for added, red for removed, yellow for modified with field-level detail
- DIFF_SKIP_FIELDS excludes metadata fields (id, topology_id, FK IDs, timestamps) from comparison
- Field value formatting: strings without quotes, null as "(none)", booleans/numbers as-is
- Summary line with counts of added/modified/removed
- Tree command (`sp topology tree`) shows all topologies as a fork hierarchy with tags inline
- Log command (`sp topology log <name>`) shows ancestry chain from root to target with "<-- you are here" marker
- All three commands support `--format json` for structured output

## Task Commits

Each task was committed atomically:

1. **Task 1+2: Implement diff engine, tree, and log commands** - `ab8ef15` (feat)

## Files Created/Modified
- `src/cli/topology.rs` - Added Diff, Tree, Log subcommands; DiffEntry/FieldDiff types; diff engine with entity loading helpers; tree rendering with box-drawing characters; log with ancestry walk

## Decisions Made
- D024: Diff matches entities by display name (not UUID) with compound keys for volumes and placements. Volumes are matched by `node_name/volume_name` since volume names are only unique per (topology, node). Placements matched by `dataset_name on node_name/volume_name`. Links matched by `source_name -> target_name`.
- D025: DIFF_SKIP_FIELDS excludes id, topology_id, node_id, dataset_id, volume_id, source_node_id, target_node_id, source_volume_id, target_volume_id, created_at, updated_at. These fields differ between forks by definition and would produce noise in the diff.

## Deviations from Plan

None -- plan executed exactly as written.

## Issues Encountered
None -- execution was straightforward.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 3 (Topology Versioning) is now complete: create/tag/fork/diff/tree/log workflow
- The diff engine enables users to compare alternatives and make informed decisions
- Tree/log provide exploration history visualization
- All 46 tests pass, no new clippy warnings
- Ready for Phase 4 (Analysis Functions)

## Self-Check: PASSED

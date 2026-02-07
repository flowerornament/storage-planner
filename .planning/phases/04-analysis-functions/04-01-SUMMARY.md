---
phase: 04-analysis-functions
plan: 01
subsystem: analysis
tags: [redundancy, capacity, scoring, cli, serde, analysis-engine]

# Dependency graph
requires:
  - phase: 02-cli-scaffolding-and-basic-commands
    provides: "Entity models (Dataset, Volume, Node, Placement), CLI framework, resolve helpers"
  - phase: 03-topology-versioning
    provides: "Topology tag lifecycle, resolve_active_topology"
provides:
  - "Pure analysis functions: analyze_redundancy, analyze_capacity"
  - "PlacementWithContext enriched loader with JOIN query"
  - "RedundancyReport and CapacityReport scored result types (Serialize/Deserialize)"
  - "sp analyze redundancy command with text/json output"
  - "sp analyze capacity command with timeline projections"
  - "Shared CLI helpers: load_datasets, load_volumes, print_analysis_header"
affects: [04-02-PLAN, phase-05, phase-06]

# Tech tracking
tech-stack:
  added: [croner 3.0.1 (dependency for Plan 02)]
  patterns: [pure-analysis-functions, scored-reports, exit-code-1-on-issues]

key-files:
  created:
    - src/domains/storage/analysis.rs
    - src/cli/analyze.rs
  modified:
    - Cargo.toml
    - src/domains/storage/mod.rs
    - src/cli/mod.rs

key-decisions:
  - "Empty-string locations count as separate unknowns in redundancy analysis (avoids false-positive merging)"
  - "Volumes with zero growth data excluded from capacity scoring but included in projections"
  - "Exit code 1 when issues found, 0 when clean -- enables scripting"

patterns-established:
  - "Analysis pattern: pure functions in domains/storage/analysis.rs, thin CLI wrappers in cli/analyze.rs"
  - "Scored reports: 0-100 score with green/yellow/red thresholds (100/75/<75)"
  - "Verbose flag pattern: default shows issues only, --verbose shows all items"
  - "JSON wrapper pattern: topology name/id envelope around serialized report"

# Metrics
duration: 7min
completed: 2026-02-07
---

# Phase 4 Plan 01: Redundancy and Capacity Analysis Summary

**Pure analysis engine with scored redundancy/capacity reports, CLI commands with text/JSON output, verbose mode, and 10 unit tests**

## Performance

- **Duration:** 7 min
- **Started:** 2026-02-07T20:46:29Z
- **Completed:** 2026-02-07T20:53:02Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- Created analysis engine with PlacementWithContext JOIN loader and two pure analysis functions
- Redundancy analysis scores datasets against min_copies and min_locations requirements with fix suggestions
- Capacity analysis projects months-until-full per volume with 3/6/12 month timeline
- Both commands support --verbose for full breakdown, --format=json for agent consumption, exit code 1 on issues
- 10 unit tests covering edge cases: no datasets, unplaced datasets, no growth, usable_bytes precedence

## Task Commits

Each task was committed atomically:

1. **Task 1: Analysis engine types and pure functions** - `f543f53` (feat)
2. **Task 2: CLI analyze command with redundancy and capacity subcommands** - `0121fb8` (feat)

## Files Created/Modified
- `src/domains/storage/analysis.rs` - Pure analysis functions, result types, PlacementWithContext loader, 10 unit tests
- `src/cli/analyze.rs` - CLI layer with AnalyzeCommands enum, redundancy/capacity handlers, text/JSON formatters
- `src/cli/mod.rs` - Added Analyze variant to Commands enum with dispatch
- `src/domains/storage/mod.rs` - Added `pub mod analysis;` export
- `Cargo.toml` - Added croner 3.0.1 dependency (for Plan 02)

## Decisions Made
- Empty-string node locations each count as separate unknown locations in redundancy analysis to avoid false-positive "same location" merging
- Volumes with zero growth data are excluded from capacity scoring but still appear in projections with months_until_full = None
- Exit code 1 when any issues are found enables scripting and CI integration

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Analysis engine architecture established (pure functions + thin CLI wrappers)
- Plan 02 can add RPO and failure analysis subcommands to existing AnalyzeCommands enum
- croner dependency already available for RPO schedule parsing
- PlacementWithContext loader reusable by all analysis functions

## Self-Check: PASSED

- FOUND: src/domains/storage/analysis.rs
- FOUND: src/cli/analyze.rs
- FOUND: f543f53
- FOUND: 0121fb8

---
*Phase: 04-analysis-functions*
*Completed: 2026-02-07*

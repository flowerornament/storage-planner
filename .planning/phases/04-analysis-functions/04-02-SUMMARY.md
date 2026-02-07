---
phase: 04-analysis-functions
plan: 02
subsystem: analysis
tags: [rpo, failure-simulation, croner, cron, dashboard, cli]

requires:
  - phase: 04-analysis-functions/01
    provides: "Redundancy and capacity analysis engine, PlacementWithContext loader, CLI analyze scaffold"
provides:
  - "RPO compliance analysis via cron interval parsing"
  - "Node failure simulation with LOST/DEGRADED/AT RISK severity tiers"
  - "Combined analysis dashboard (sp analyze with no subcommand)"
  - "SyncRegimeWithContext JOINed loader"
affects: [05-decision-integration, 06-cost-and-context]

tech-stack:
  added: []
  patterns:
    - "Optional subcommand for combined dashboard behavior"
    - "Severity enum ordering for failure simulation results"
    - "cron_interval_hours via croner for RPO gap calculation"

key-files:
  created: []
  modified:
    - src/domains/storage/analysis.rs
    - src/cli/analyze.rs
    - src/cli/mod.rs

key-decisions:
  - "D029: Failure severity checks min_copies/min_locations before general degraded to correctly classify AT RISK"
  - "D030: Optional clap subcommand with top-level --topology/--verbose/--warn-months for dashboard mode"

patterns-established:
  - "Severity ordering: Lost > Degraded > AtRisk with Ord derive for sorting"
  - "load_sync_regimes_with_context JOINed loader pattern matching load_placements_with_context"

duration: 6m 41s
completed: 2026-02-07
---

# Phase 4 Plan 2: RPO, Failure Simulation, and Combined Dashboard Summary

**RPO compliance via croner cron parsing, node failure simulation with LOST/DEGRADED/AT RISK severity, and combined `sp analyze` dashboard**

## Performance

- **Duration:** 6m 41s
- **Started:** 2026-02-07T20:56:58Z
- **Completed:** 2026-02-07T21:03:39Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- RPO analysis computes sync interval gaps via croner, compares against max_rpo_hours, and produces scored reports
- Failure simulation accepts node names, resolves to IDs, computes volume and dataset impact with three severity tiers
- Combined dashboard (`sp analyze` with no subcommand) shows redundancy + RPO + capacity in one view
- 11 new unit tests covering RPO edge cases and failure scenarios (21 total in analysis module)

## Task Commits

Each task was committed atomically:

1. **Task 1: RPO analysis and failure simulation engine** - `5b9e8b6` (feat)
2. **Task 2: CLI RPO, failure, and combined dashboard commands** - `031c74c` (feat)

## Files Created/Modified
- `src/domains/storage/analysis.rs` - RPO types/functions, failure simulation types/functions, SyncRegimeWithContext loader, 11 new tests
- `src/cli/analyze.rs` - Rpo and Failure subcommands, combined dashboard handler, load_nodes and load_sync_regimes helpers
- `src/cli/mod.rs` - Optional subcommand pattern for Analyze variant with top-level args

## Decisions Made
- D029: Failure severity classification checks remaining_copies against min_copies and remaining_locations against min_locations before classifying as general Degraded, correctly identifying AT RISK cases where copies are sufficient but location requirements are broken
- D030: Used optional subcommand with clap (`Option<AnalyzeCommands>`) and top-level --topology/--verbose/--warn-months flags for clean dashboard behavior

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed failure severity classification for AT RISK**
- **Found during:** Task 1 (failure simulation tests)
- **Issue:** Original severity logic checked `remaining_copies < total_copies` for Degraded before checking AT RISK condition, causing datasets that met min_copies but failed min_locations to be classified as Degraded instead of AtRisk
- **Fix:** Restructured severity checks to evaluate min_copies/min_locations first, then general degradation
- **Files modified:** src/domains/storage/analysis.rs
- **Verification:** test_failure_at_risk passes
- **Committed in:** 5b9e8b6 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 bug fix)
**Impact on plan:** Essential fix for correct severity classification. No scope creep.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 4 complete: all four analysis commands (redundancy, rpo, capacity, failure) operational
- Combined dashboard provides single-command topology health overview
- Decision integration (Phase 5) can reference analysis results for topology comparison
- All analysis functions are pure (take data, return reports) -- easy to compose in Phase 5 comparison views

---
*Phase: 04-analysis-functions*
*Completed: 2026-02-07*

---
phase: 05-decision-integration
plan: 03
subsystem: cli, analysis
tags: [decision-lifecycle, constraints, comparison, snapshot, clap, sqlite]

# Dependency graph
requires:
  - phase: 05-decision-integration
    provides: "Decision CLI CRUD commands, constraint/topology management, analysis pure functions"
provides:
  - "Decision lifecycle commands: choose, abandon, reopen with state machine validation"
  - "Constraint checking analysis: pass/warn/fail per constraint with colored output"
  - "Topology comparison: side-by-side metrics with advantage indicators and optional diff"
  - "Snapshot generation: JSON blob capturing comparison data at close time"
  - "Exit code 1 on constraint failures (D028 pattern)"
affects: [06-cost-and-context]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Snapshot generation at close time captures all considered topologies with metrics and constraints"
    - "Constraint checking as pure function with pass/warn/fail threshold at 90% of limit"
    - "Topology comparison with lower-is-better/higher-is-better/neutral metric classification"
    - "RFC3339 timestamps for all datetime columns (not SQLite datetime('now'))"

key-files:
  created: []
  modified:
    - src/domains/storage/analysis.rs
    - src/cli/decision.rs
    - src/cli/analyze.rs

key-decisions:
  - "D033: RFC3339 timestamps used for closed_at to ensure consistent parsing in Decision::from_row"

patterns-established:
  - "Constraint checking: check_constraints(constraints, nodes) -> ConstraintReport with pass/warn/fail"
  - "Topology metrics: compute_topology_metrics aggregates all data into comparable struct"
  - "Comparison: compare_topologies produces MetricComparison vec with advantage indicators"
  - "Snapshot: JSON blob generated at close/abandon time capturing all metrics for historical record"

# Metrics
duration: 8min
completed: 2026-02-08
---

# Phase 5 Plan 3: Decision Lifecycle and Comparison Summary

**Decision lifecycle (choose/abandon/reopen) with state machine validation, constraint checking with pass/warn/fail scoring, topology comparison with advantage indicators, and snapshot generation at close time**

## Performance

- **Duration:** 7m 43s
- **Started:** 2026-02-08T01:33:58Z
- **Completed:** 2026-02-08T01:41:41Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Pure constraint checking and topology comparison functions with 6 new unit tests
- DEC-09 choose validates open status, topology is considered, generates snapshot, requires rationale
- DEC-10 abandon works on draft/open decisions with optional reason and snapshot generation
- DEC-11 reopen clears chosen topology, rationale, closed_at; keeps constraints and considered topologies
- ANLZ-02 constraint checking with colored pass/warn/fail output and exit code 1 on failures
- ANLZ-08 topology comparison with side-by-side metrics, advantage arrows, optional diff, optional decision context
- Snapshot JSON blob captures all considered topologies with metrics and constraints at close time
- All commands support text and JSON output formats

## Task Commits

Each task was committed atomically:

1. **Task 1: Constraint checking and topology comparison functions** - `d2fa139` (feat)
2. **Task 2: Choose/abandon/reopen commands and analyze constraints/compare subcommands** - `7477aed` (feat)

## Files Created/Modified
- `src/domains/storage/analysis.rs` - Added ConstraintStatus/ConstraintResult/ConstraintReport types, check_constraints function, TopologyMetrics/MetricComparison/ComparisonReport types, compute_topology_metrics and compare_topologies functions, 6 new tests
- `src/cli/decision.rs` - Implemented choose/abandon/reopen replacing stubs, added snapshot generation, RFC3339 timestamp handling, load helper functions for topology data
- `src/cli/analyze.rs` - Added Constraints and Compare subcommands to AnalyzeCommands enum, run_constraints with colored pass/warn/fail output, run_compare with side-by-side metrics table, structural diff support

## Decisions Made
- D033: Used RFC3339 timestamps for closed_at instead of SQLite datetime('now') to ensure consistent parsing in Decision::from_row (which uses DateTime::parse_from_rfc3339)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed closed_at datetime format for consistent parsing**
- **Found during:** Task 2 (end-to-end testing of choose command)
- **Issue:** Using SQLite `datetime('now')` produces `YYYY-MM-DD HH:MM:SS` format which fails RFC3339 parsing in `Decision::from_row`, causing `closed_at` to silently become `None` in JSON output
- **Fix:** Changed choose/abandon/reopen to use `Utc::now().to_rfc3339()` format, matching the pattern used by all other timestamp columns
- **Files modified:** src/cli/decision.rs
- **Verification:** JSON output correctly shows closed_at with timezone after choose command
- **Committed in:** 7477aed (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Essential fix for correct datetime display. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 5 complete: all 13 decision integration requirements (DEC-01 through DEC-11, ANLZ-02, ANLZ-08) implemented
- Full decision lifecycle: create -> constrain -> consider -> open -> compare -> choose/abandon -> reopen
- All analysis functions available: redundancy, capacity, RPO, failure, constraints, compare
- Ready for Phase 6: Cost and Context (catalog, pricing, bandwidth analysis, sp prime)

## Self-Check: PASSED

- FOUND: src/domains/storage/analysis.rs
- FOUND: src/cli/decision.rs
- FOUND: src/cli/analyze.rs
- FOUND: .planning/phases/05-decision-integration/05-03-SUMMARY.md
- FOUND: commit d2fa139 (Task 1)
- FOUND: commit 7477aed (Task 2)

---
*Phase: 05-decision-integration*
*Completed: 2026-02-08*

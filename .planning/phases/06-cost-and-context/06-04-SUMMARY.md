---
phase: 06-cost-and-context
plan: 04
subsystem: cli
tags: [status, prime, context, agent-bootstrap, dashboard]

# Dependency graph
requires:
  - phase: 06-02
    provides: catalog CLI (items and prices for status catalog section)
  - phase: 06-03
    provides: bandwidth/cost analysis functions (used by status problem detection)
provides:
  - sp status -- system health dashboard with problems-first display
  - sp prime -- AI agent bootstrap document with workflow guide and dynamic state
  - sp current -- quick show/set for current topology
affects: [agent-workflows, user-onboarding]

# Tech tracking
tech-stack:
  added: []
  patterns: [problems-first dashboard, static-plus-dynamic agent bootstrap]

key-files:
  created:
    - src/cli/status.rs
    - src/cli/prime.rs
  modified:
    - src/cli/mod.rs

key-decisions:
  - "D040: Status problems section uses 6-month threshold for capacity warnings (shorter than analyze default of 12)"
  - "D041: Prime outputs markdown-formatted text (not JSON) as agent bootstrap is read by LLMs"
  - "D042: sp current sets tag via same pattern as topology tag command (clear existing current first per D020)"

patterns-established:
  - "Problems-first display: status shows alerts at top, skips section when healthy"
  - "Static-plus-dynamic: prime outputs const guide then queries DB for state"

# Metrics
duration: 5m 31s
completed: 2026-02-08
---

# Phase 6 Plan 4: Status Dashboard and Prime Summary

**Three context commands: status dashboard with problems-first display, prime agent bootstrap with workflow guide and dynamic state, current topology quick-switch**

## Performance

- **Duration:** 5m 31s
- **Started:** 2026-02-08T05:45:21Z
- **Completed:** 2026-02-08T05:50:52Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Status command surfaces problems first (redundancy issues, capacity warnings, stale decisions), then shows current topology with entity counts and analysis scores, open decisions table, catalog stats, and recent activity
- Prime command outputs complete agent bootstrap with 6 workflow sections (Explore, Topologies, Build, Analyze, Decisions, Catalog) with concrete example commands, followed by dynamic state summary
- Current command provides quick topology show/set shortcut with event recording
- Status supports --format=json for machine-readable output

## Task Commits

Each task was committed atomically:

1. **Task 1: Status dashboard and current topology commands** - `884cdb1` (feat)
2. **Task 2: Prime command -- AI agent bootstrap** - `f18dc20` (feat)

## Files Created/Modified
- `src/cli/status.rs` - Status dashboard (problems, topology, decisions, catalog, activity) and current topology show/set
- `src/cli/prime.rs` - Agent bootstrap document with static workflow guide and dynamic state summary
- `src/cli/mod.rs` - Wired Status, Prime, Current variants into CLI enum with match arms

## Decisions Made
- D040: Status uses 6-month capacity warning threshold (vs 12-month in analyze) for tighter problem detection
- D041: Prime outputs markdown text (not JSON) since its primary consumer is LLM agents that parse markdown
- D042: sp current reuses the same tag-clearing pattern as topology tag command (D020 enforcement)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All Phase 6 plans complete (06-01 through 06-05)
- Full CLI surface area implemented: topology management, analysis, decisions, catalog, export/import, diagrams, status, prime, current
- 97 tests passing across all modules

---
*Phase: 06-cost-and-context*
*Completed: 2026-02-08*

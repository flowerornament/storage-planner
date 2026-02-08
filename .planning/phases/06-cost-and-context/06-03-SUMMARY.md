---
phase: 06-cost-and-context
plan: "03"
subsystem: analysis
tags: [bandwidth, cost, tco, sync-regimes, links, catalog]

# Dependency graph
requires:
  - phase: 04-analysis-engine
    provides: analysis module pattern (pure functions, report structs, scoring)
  - phase: 06-cost-and-context
    provides: catalog_items and prices tables (plan 01), catalog CLI (plan 02)
provides:
  - bandwidth analysis with link capacity vs sync demand comparison
  - cost analysis with per-entity breakdown, category summary, and TCO projection
  - BandwidthReport and CostReport structs for downstream use
  - bandwidth and cost CLI subcommands under sp analyze
affects: [06-cost-and-context plan 04 (decision integration)]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "volume-to-node resolution via HashMap lookup for bandwidth analysis"
    - "bidirectional link indexing (source->target and target->source)"
    - "latest-price-per-item query pattern for cost aggregation"
    - "one-time vs recurring cost separation (never a single total)"
    - "TCO formula: one_time + (monthly * 12 * years) + (annual * years)"

key-files:
  created: []
  modified:
    - src/domains/storage/analysis.rs
    - src/cli/analyze.rs

key-decisions:
  - "D036: SyncRegimeWithContext lacks node IDs -- resolve volume_id to node_id via volumes parameter"
  - "D037: Bidirectional link matching -- index both directions for link lookup"
  - "D038: Latest price observation used per item for cost calculation"
  - "D039: Direct link checking only for bandwidth (path-finding deferred to ANLZ-10)"

patterns-established:
  - "Volume-to-node resolution: build HashMap<volume_id, node_id> from volumes list"
  - "Link capacity lookup: index links bidirectionally for O(1) access"
  - "Cost aggregation: separate one_time/monthly/annual, never flatten to single number"

# Metrics
duration: 6min
completed: 2026-02-08
---

# Phase 6 Plan 03: Bandwidth and Cost Analysis Summary

**Bandwidth analysis comparing sync regime demand against link capacity, plus per-entity cost breakdown with category summary and TCO projection modes**

## Performance

- **Duration:** ~6 min 19s
- **Started:** 2026-02-08T05:35:41Z
- **Completed:** 2026-02-08T05:42:00Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Bandwidth analysis engine: resolves sync regimes to node pairs, finds direct links, computes required vs available bandwidth with Adequate/Tight/Insufficient/NoLink status
- Cost analysis engine: aggregates per-entity costs from catalog items and price observations, separates one-time/monthly/annual, computes TCO over configurable periods
- Five unit tests covering bandwidth (adequate, insufficient, no-link) and cost (aggregation, TCO projection)
- CLI subcommands: `sp analyze bandwidth` and `sp analyze cost` with --summary and --tco flags
- Dashboard integration: one-line cost summary added to `sp analyze` combined view

## Task Commits

Each task was committed atomically:

1. **Task 1: Bandwidth and cost analysis pure functions** - `6ddd8b2` (feat)
2. **Task 2: Bandwidth and cost CLI subcommands** - `33b1962` (feat)

**Plan metadata:** (pending)

## Files Created/Modified
- `src/domains/storage/analysis.rs` - Added BandwidthStatus, BandwidthResult, BandwidthReport, EntityCost, CostReport structs; analyze_bandwidth(), analyze_cost(), compute_tco_cents() functions; 5 unit tests
- `src/cli/analyze.rs` - Added Bandwidth and Cost subcommands to AnalyzeCommands enum; run_bandwidth, run_cost handlers; format helpers; dashboard cost summary line; load_links helper

## Decisions Made
- D036: SyncRegimeWithContext lacks source/target node IDs -- bandwidth analysis takes volumes parameter and builds volume_id->node_id HashMap for resolution
- D037: Links indexed bidirectionally (source->target and target->source) so either direction matches
- D038: Latest price observation per item used for cost calculation (most recent recorded price)
- D039: Only direct links checked for bandwidth (no path-finding -- that is ANLZ-10 scope)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed temporary borrow in print_bandwidth_text**
- **Found during:** Task 2 (CLI subcommands)
- **Issue:** `unwrap_or(&result.source_node.as_str())` created a temporary value dropped while still borrowed
- **Fix:** Bound fallback strings to named variables before passing to unwrap_or
- **Files modified:** src/cli/analyze.rs
- **Verification:** Build succeeded, clippy clean
- **Committed in:** 33b1962 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 bug fix)
**Impact on plan:** Minor Rust borrow issue, no scope change.

## Issues Encountered
None beyond the temporary borrow fix above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Bandwidth and cost analysis functions available for decision integration (plan 04)
- BandwidthReport and CostReport structs ready for consumption by decision comparison views
- All 97 tests passing, 0 clippy warnings

---
*Phase: 06-cost-and-context*
*Completed: 2026-02-08*

## Self-Check: PASSED
- [x] src/domains/storage/analysis.rs exists
- [x] src/cli/analyze.rs exists
- [x] 06-03-SUMMARY.md exists
- [x] Commit 6ddd8b2 (Task 1) exists
- [x] Commit 33b1962 (Task 2) exists
- [x] 97 tests passing, 0 failures

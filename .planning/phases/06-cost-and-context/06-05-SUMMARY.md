---
phase: 06-cost-and-context
plan: 05
subsystem: cli
tags: [yaml, serde-yaml-ng, ascii-diagram, export, import, topology]

# Dependency graph
requires:
  - phase: 06-cost-and-context
    provides: serde_yaml_ng dependency in Cargo.toml (plan 01)
  - phase: 03-topology-lifecycle
    provides: fork ID remapping pattern, resolve helpers
provides:
  - sp export command with --template, --only, --output flags (TOPO-11)
  - sp import command with ID remapping and name collision handling (TOPO-10)
  - sp diagram command with --tree and --network views (TOPO-09)
  - TopologyExport YAML serialization struct
affects: [topology-sharing, backup-restore, visualization]

# Tech tracking
tech-stack:
  added: [serde_yaml_ng (shared with plan 01)]
  patterns: [YAML round-trip serialization, Unicode box-drawing tree rendering]

key-files:
  created:
    - src/cli/export.rs
    - src/cli/diagram.rs
  modified:
    - src/cli/mod.rs

key-decisions:
  - "Template mode strips IDs to empty strings rather than using Option wrappers"
  - "Import uses name-based fallback keys for template mode FK resolution"
  - "Diagram tree uses Unicode box-drawing chars (U+251C, U+2514, U+2502, U+2500)"
  - "Network mode formats bandwidth with human-readable units (KB/s, MB/s, GB/s)"

patterns-established:
  - "TopologyExport struct pattern for full topology serialization/deserialization"
  - "Block-scoped D023 pattern consistently applied in new modules"

# Metrics
duration: 5min
completed: 2026-02-08
---

# Phase 6 Plan 5: YAML Export/Import and ASCII Diagram Summary

**YAML topology export/import with identity preservation and template modes, plus Unicode ASCII diagram with tree and network perspectives**

## Performance

- **Duration:** 4m 39s
- **Started:** 2026-02-08T05:24:05Z
- **Completed:** 2026-02-08T05:28:44Z
- **Tasks:** 2
- **Files created:** 2, **modified:** 1

## Accomplishments
- Full YAML export preserving topology identity (all entities with IDs) or as template (IDs stripped for reuse)
- Import with fork-pattern ID remapping, name collision handling, and single-transaction insertion
- Partial export via --only flag for any combination of entity types
- ASCII tree diagram showing node-volume-dataset hierarchy with Unicode box-drawing characters
- Network diagram showing link topology with human-readable bandwidth formatting

## Task Commits

Each task was committed atomically:

1. **Task 1: YAML export and import commands** - `c695529` (feat)
2. **Task 2: ASCII diagram command** - `c573311` (feat)

## Files Created/Modified
- `src/cli/export.rs` - TopologyExport struct, sp export (TOPO-11) and sp import (TOPO-10) commands
- `src/cli/diagram.rs` - sp diagram command with --tree and --network views (TOPO-09)
- `src/cli/mod.rs` - Wired Export, Import, and Diagram commands into CLI

## Decisions Made
- Template mode strips IDs to empty strings (simple approach vs. Option wrappers -- empty string checked during import to determine template vs. identity mode)
- Import uses compound name-based fallback keys (`node:name`, `volume:name`, `dataset:name`) for template mode FK resolution where original IDs are empty
- Imported topologies start untagged (tag=NULL) with no parent_id, consistent with D022
- Diagram defaults to --tree mode when neither --tree nor --network specified; shows both when both passed

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed borrow checker with D023 block-scoped pattern**
- **Found during:** Task 1 (export entity loading)
- **Issue:** Rust borrow checker rejected `stmt.query_map(...).collect()` as tail expression in if blocks
- **Fix:** Applied the established D023 pattern: `let result = stmt.query_map(...)...; result`
- **Files modified:** src/cli/export.rs
- **Verification:** cargo build succeeds
- **Committed in:** c695529 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Known Rust borrow pattern, applied consistently. No scope creep.

## Issues Encountered
None beyond the D023 borrow pattern fix documented above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All 5 Phase 6 wave-1 plans can complete (this is one of them)
- Export/import enables topology sharing and backup workflows
- Diagram enables quick visual inspection of topology structure

---
*Phase: 06-cost-and-context*
*Completed: 2026-02-08*

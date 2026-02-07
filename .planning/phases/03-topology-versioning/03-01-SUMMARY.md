---
phase: 03-topology-versioning
plan: 01
subsystem: database
tags: [sqlite, migration, tag-lifecycle, topology, cli]

# Dependency graph
requires:
  - phase: 02-cli-scaffolding-and-basic-commands
    provides: "Topology CRUD commands, entity resolver, is_active boolean model"
provides:
  - "Migration v2: tag column replaces is_active boolean"
  - "Topology.tag field (Option<String>) with current/exploring/archived values"
  - "Partial unique index enforcing single 'current' topology at DB level"
  - "tag/untag CLI commands with validation"
  - "Backward-compat set-active with deprecation notice"
  - "show command displays parent name and fork count"
affects: [03-02-fork-command, 03-03-diff-engine, 04-analysis-functions]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Tag-based lifecycle: topology states via tag column instead of boolean"
    - "Partial unique index for business rule enforcement at DB level"
    - "Backward-compat alias with deprecation notice for renamed commands"

key-files:
  created: []
  modified:
    - "src/core/db.rs"
    - "src/core/models.rs"
    - "src/core/resolve.rs"
    - "src/cli/topology.rs"

key-decisions:
  - "D019: Tag column replaces is_active boolean (current/exploring/archived/null)"
  - "D020: Partial unique index WHERE tag='current' enforces single current at DB level"
  - "D021: set-active preserved as backward-compat alias with deprecation notice"

patterns-established:
  - "Tag-based lifecycle: topologies transition through null -> current/exploring/archived states"
  - "Schema migration v2: ALTER TABLE add/drop columns with data migration in single batch"

# Metrics
duration: 3m 42s
completed: 2026-02-07
---

# Phase 3 Plan 1: Tag-Based Lifecycle Migration Summary

**Replaced is_active boolean with tag column (current/exploring/archived), added tag/untag commands, partial unique index enforcement**

## Performance

- **Duration:** 3m 42s
- **Started:** 2026-02-07T10:25:17Z
- **Completed:** 2026-02-07T10:28:59Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Migration v2 converts is_active data to tag column and drops the old column
- Partial unique index prevents multiple topologies from having the "current" tag simultaneously
- New `tag` and `untag` commands with validation (only current/exploring/archived accepted)
- `list` shows `[current]`, `[exploring]`, `[archived]` inline instead of `(active)`
- `show` displays parent topology name and fork count
- `set-active` preserved as backward-compat alias with deprecation notice

## Task Commits

Each task was committed atomically:

1. **Task 1: Schema migration v2 and Topology model update** - `85ae355` (feat)
2. **Task 2: Update resolve, existing commands, and add tag/untag** - `1de8d34` (feat)

**Plan metadata:** TBD (docs: complete plan)

## Files Created/Modified
- `src/core/db.rs` - Added SCHEMA_V2 migration, bumped CURRENT_VERSION to 2, added migration test
- `src/core/models.rs` - Replaced Topology.is_active: bool with tag: Option<String>, updated insert/from_row/tests
- `src/core/resolve.rs` - Updated resolve_active_topology to query tag='current', updated SELECT columns
- `src/cli/topology.rs` - Added Tag/Untag commands, updated list/show/create/set-active for tag system

## Decisions Made
- D019: Tag column replaces is_active boolean. The tag field is an Option<String> that holds "current", "exploring", "archived", or None. This gives richer lifecycle states than a simple boolean.
- D020: Partial unique index (WHERE tag='current') enforces the single-current constraint at the database level, which is stronger than application-level enforcement.
- D021: set-active is preserved as a backward-compatible alias that internally uses the tag system. It prints a deprecation notice directing users to `sp topology tag <name> current`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed tag variable shadowing in match arm**
- **Found during:** Task 2 (wiring Tag variant in run function)
- **Issue:** The `tag` field destructured from `TopologyCommands::Tag { name, tag }` shadowed the `tag()` function, causing a "expected function, found String" compilation error.
- **Fix:** Renamed the destructured binding to `tag_value` in the match arm: `TopologyCommands::Tag { name, tag: tag_value }`.
- **Files modified:** src/cli/topology.rs
- **Verification:** `cargo test` compiles and passes all 46 tests
- **Committed in:** 1de8d34 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Minor naming fix required for compilation. No scope change.

## Issues Encountered
None -- execution was straightforward.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Tag system is the foundation for fork (03-02) and diff (03-03) plans
- Fork command can use tag to mark forked topologies as "exploring"
- Diff command can use tag display in output
- All 46 tests pass, zero is_active references remain in active code

## Self-Check: PASSED

---
*Phase: 03-topology-versioning*
*Completed: 2026-02-07*

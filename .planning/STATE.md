# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-02-05)

**Core value:** Session continuity for AI-assisted purchase decisions
**Current focus:** Phase 7 -- CLI polish and correctness

## Current Position

Phase: 7 of 7 (CLI Polish and Correctness)
Plan: 1 of 2 in current phase
Status: In progress
Last activity: 2026-02-08 - Completed 07-01-PLAN.md

Progress: [████████████████████░] 95% (20/21 plans complete)

## Performance Metrics

**Velocity:**
- Total plans completed: 20
- Average duration: 4m 49s
- Total execution time: ~1.58 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 01 | 2/2 | 10m 34s | 5m 17s |
| 02 | 4/4 | 13m 27s | 3m 22s |
| 03 | 3/3 | 11m 13s | 3m 44s |
| 04 | 2/2 | 13m 41s | 6m 51s |
| 05 | 3/3 | 16m 56s | 5m 39s |
| 06 | 5/5 | 27m 33s | 5m 31s |
| 07 | 1/2 | 4m 21s | 4m 21s |

**Recent Trend:**
- Last 5 plans: 07-01 (4m 21s), 06-05 (4m 39s), 06-01 (6m 37s), 06-02 (4m 27s), 06-03 (6m 19s)
- Trend: Consistent execution

*Updated after each plan completion*

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

- D001: Removed 5 crates from Cargo.toml (ureq, xshell, serde_yaml, camino, fs-err)
- D002: PRAGMA user_version for migration tracking
- D003: volumes.item_id TEXT with no FK constraint (deferred to Phase 6)
- D004: Single v1 migration creates all 9 tables
- D005: Deleted old CLI/pricing modules entirely (clean rewrite)
- D006: Generic undo handler using event_type suffix (.created/.deleted/.updated)
- D007: set-active undo restores target topology but doesn't re-activate previous (Phase 1 limitation)
- D008: Placeholder commands define full arg structure for Phase 2
- D009: Resolve entities outside transactions, use resolved ID inside (avoids conn conflicts)
- D010: Slug validation requires alphanumeric, hyphens, underscores (no spaces or special chars)
- D011: UUID prefix minimum 4 chars for disambiguation
- D012: Volume disambiguation via --node flag when same name on multiple nodes
- D013: Criticality validation rejects non-standard values (only normal/important/critical)
- D014: Placement role validation (only primary/replica/backup/archive)
- D015: Dataset show displays inline placements via JOIN with volumes and nodes
- D016: Node show displays inline volumes with formatted capacity
- D017: Volume name uniqueness scoped to (topology, node), not global
- D018: Update commands build complete after-state before SQL for accurate event logging
- D019: Tag column replaces is_active boolean (current/exploring/archived/null)
- D020: Partial unique index WHERE tag='current' enforces single current at DB level
- D021: set-active preserved as backward-compat alias with deprecation notice
- D022: Fork starts untagged (tag=NULL) -- user decides lifecycle state after forking
- D023: Block-scoped prepared statements resolve borrow checker conflict with D009 pattern
- D024: Diff matches entities by display name (not UUID) with compound keys for volumes and placements
- D025: DIFF_SKIP_FIELDS excludes id, topology_id, and all FK fields from comparison
- D026: Empty-string locations count as separate unknowns in redundancy analysis
- D027: Volumes with zero growth data excluded from capacity scoring but included in projections
- D028: Analysis exit code 1 on issues, 0 when clean (enables scripting)
- D029: Failure severity checks min_copies/min_locations before general degraded to correctly classify AT RISK
- D030: Optional clap subcommand with top-level args for combined dashboard mode
- D031: Decision titles use free-text (not slugs) -- supports natural language naming
- D032: No power_watts column added -- existing power_draw_watts covers same concept
- D033: RFC3339 timestamps used for closed_at to ensure consistent parsing in Decision::from_row
- D034: Template mode strips IDs to empty strings; import uses name-based fallback keys for FK resolution
- D035: Imported topologies start untagged with no parent_id (consistent with D022)
- D036: SyncRegimeWithContext lacks node IDs -- resolve volume_id to node_id via volumes parameter
- D037: Bidirectional link matching -- index both directions for link lookup
- D038: Latest price observation per item used for cost calculation
- D039: Direct link checking only for bandwidth (path-finding deferred to ANLZ-10)
- D040: Status problems section uses 6-month threshold for capacity warnings (shorter than analyze default of 12)
- D041: Prime outputs markdown text (not JSON) as agent bootstrap is read by LLMs
- D042: sp current sets tag via same pattern as topology tag command (clear existing current first per D020)

### Pending Todos

None.

### Roadmap Evolution

- Phase 7 added: CLI Polish and Correctness (post-v1 testing identified 4 priority issues)

### Blockers/Concerns

None.

## Session Continuity

Last session: 2026-02-08
Stopped at: Completed 07-01 (prime corrections + item-id linking)
Resume file: .planning/phases/07-cli-polish-and-correctness/07-02-PLAN.md

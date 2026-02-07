---
phase: 01-schema-and-core-types
plan: 01
subsystem: core-data
tags: [sqlite, schema, migration, models, rusqlite, topology]
dependency-graph:
  requires: []
  provides: [database-layer, topology-schema, entity-models, migration-system]
  affects: [01-02, 02-xx, 03-xx, 04-xx]
tech-stack:
  added: []
  removed: [ureq, xshell, serde_yaml, camino, fs-err]
  patterns: [pragma-user-version-migration, entity-new-insert-from_row-to_json, strictly-typed-columns]
key-files:
  created: []
  modified:
    - Cargo.toml
    - src/main.rs
    - src/core/mod.rs
    - src/core/db.rs
    - src/core/models.rs
    - src/core/events.rs
    - src/cli/mod.rs
    - src/domains/mod.rs
    - src/domains/storage/mod.rs
    - src/domains/storage/models.rs
  deleted:
    - src/lib.rs
    - src/pricing/mod.rs
    - src/pricing/bestbuy.rs
    - src/pricing/ebay.rs
    - src/pricing/fallback.rs
    - src/pricing/product.rs
    - src/pricing/url_parser.rs
    - src/domains/storage/analysis.rs
    - src/cli/analyze.rs
    - src/cli/config.rs
    - src/cli/decide.rs
    - src/cli/doctor.rs
    - src/cli/events.rs
    - src/cli/init.rs
    - src/cli/item.rs
    - src/cli/price.rs
    - src/cli/prime.rs
    - src/cli/sync.rs
decisions:
  - id: d001
    decision: "Rewrite Cargo.toml removing 5 unused dependencies (ureq, xshell, serde_yaml, camino, fs-err)"
    rationale: "Rewrite approach - these crates are not needed until Phase 6 (if ever)"
  - id: d002
    decision: "Use PRAGMA user_version for migration tracking instead of migration table"
    rationale: "Zero overhead, built into SQLite, recommended for embedded apps"
  - id: d003
    decision: "volumes.item_id is TEXT with NO FK constraint"
    rationale: "Items table does not exist in the rewrite; FK deferred to Phase 6"
  - id: d004
    decision: "Single migration v1 creates all 9 tables at once"
    rationale: "Fresh start OK per CONTEXT.md, simpler than per-table migrations"
  - id: d005
    decision: "Removed all old CLI commands and pricing module"
    rationale: "Clean rewrite - old code depends on removed crates; stub CLI for compilation"
metrics:
  duration: 5m 29s
  completed: 2026-02-07
---

# Phase 01 Plan 01: Schema and Core Data Layer Summary

**TLDR:** PRAGMA user_version migration system creating 9 tables (7 topology + events + undo_pointer) with strictly typed columns, plus 8 entity model structs with new/insert/from_row/to_json. 24 tests passing.

## Task Commits

| # | Task | Commit | Key Changes |
|---|------|--------|-------------|
| 1 | Clean Cargo.toml and entry point | `f8432c1` | Removed 5 deps, rewrote main.rs, stubbed CLI, deleted pricing/old CLI |
| 2 | Database layer with migration system | `8e28d8c` | PRAGMA user_version, 9 tables, CASCADE, indexes, 8 tests |
| 3 | Topology entity model structs | `a5bbc64` | 8 structs (7 entities + Event), 11 tests, roundtrip verified |

## What Was Built

### Database Layer (`src/core/db.rs` - 425 lines)
- `Database` struct wrapping `rusqlite::Connection`
- `open(path)` with parent dir creation, PRAGMAs (foreign_keys, WAL, synchronous), auto-migrate
- `open_memory()` for testing
- `transaction()` with auto commit/rollback
- `migrate()` using `PRAGMA user_version` -- reads current version, applies pending migrations
- `is_initialized()` checks for topologies table
- `CURRENT_VERSION = 1`

### Schema v1 (9 tables)
1. **topologies** - named configurations with parent_id for forking, is_active flag
2. **nodes** - compute devices with hardware profile (bays, interfaces, power)
3. **volumes** - storage units with capacity, filesystem, RAID, pool_type, item_id (TEXT, no FK)
4. **datasets** - logical data groups with criticality, min_copies, min_locations, max_rpo_hours
5. **placements** - junction table (dataset to volume) with role and priority
6. **links** - network connections with bandwidth, latency, metering, cost
7. **sync_regimes** - data movement definitions (dataset, source/target volume, schedule)
8. **events** - redesigned with sequence, before_state, after_state, summary, source
9. **undo_pointer** - single-row table tracking current sequence position (initialized to 0)

All topology children use `ON DELETE CASCADE`. 9 indexes created for query performance.

### Entity Models (`src/core/models.rs` - 1056 lines)
All 8 structs follow the pattern: `#[derive(Debug, Clone, Serialize, Deserialize)]`
- `new(...)` - create with UUID id and current timestamps
- `insert(&self, tx: &Transaction)` - INSERT using `params![]`
- `from_row(row: &Row)` - read from named columns
- `to_json(&self)` - serialize for event before/after state

### Test Coverage (24 tests)
- **db tests (8):** open_memory, migrate, tables_exist, foreign_keys_on, cascade_delete, is_initialized, undo_pointer, item_id_no_fk
- **model tests (11):** topology_new, topology_roundtrip, node_roundtrip, topology_to_json, cascade_node_volumes, volume_roundtrip, dataset_roundtrip, placement_roundtrip, link_roundtrip, sync_regime_roundtrip, event_roundtrip
- **specs tests (5):** capacity_parse, capacity_display, speed_parse, noise_parse, get_capacity_from_specs

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Removed old CLI and pricing modules**
- **Found during:** Task 1
- **Issue:** Old CLI files (10 modules) and pricing module (6 files) referenced removed dependencies (camino, serde_yaml, ureq, xshell). Code would not compile.
- **Fix:** Deleted all old CLI command implementations and pricing module, wrote minimal CLI stub with just Init command
- **Files deleted:** 18 files (src/cli/*.rs, src/pricing/*.rs, src/lib.rs, src/domains/storage/analysis.rs)
- **Commit:** `f8432c1`

**2. [Rule 3 - Blocking] Created stub events.rs**
- **Found during:** Task 2
- **Issue:** core/mod.rs declares `pub mod events;` but old events.rs references removed types
- **Fix:** Wrote minimal stub events.rs (placeholder for Plan 02)
- **Commit:** `f8432c1`

## Verification Results

All success criteria met:
- [x] Database opens, migrates to version 1, has all 9 tables
- [x] All topology entity structs exist with insert/from_row/to_json
- [x] PRAGMA foreign_keys = ON, CASCADE deletes verified
- [x] volumes.item_id is TEXT with NO FK constraint
- [x] No JSON metadata blob columns in new schema
- [x] No old tables (items, prices, configurations, decisions) created
- [x] cargo test passes: 24/24

## Decisions Made

| ID | Decision | Rationale |
|----|----------|-----------|
| D001 | Removed 5 crates from Cargo.toml | Not needed in rewrite; ureq/xshell return in Phase 6 if needed |
| D002 | PRAGMA user_version for migrations | Zero overhead, SQLite built-in, standard pattern |
| D003 | volumes.item_id TEXT no FK | Items table not in rewrite; FK deferred to Phase 6 |
| D004 | Single v1 migration for all tables | Fresh start OK, simpler than per-table |
| D005 | Deleted old CLI/pricing entirely | Clean rewrite, old code incompatible with reduced deps |

## Next Phase Readiness

Plan 01-02 needs:
- `src/core/events.rs` - currently a stub, Plan 02 implements the full event system with undo/redo
- `src/cli/mod.rs` - currently has only Init, Plan 02 adds all topology commands
- All model structs and db layer are ready for Plan 02 to build on

## Self-Check: PASSED

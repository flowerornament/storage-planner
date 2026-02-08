# Roadmap: Storage Planner

## Overview

Storage Planner transforms an existing purchase decision CLI into a topology-aware planning tool. The journey starts with database schema and core types (foundation), progresses through CLI scaffolding and basic commands (usable structure), adds versioning capabilities (exploration workflow), implements analysis functions (core value), integrates decision tracking (full workflow), and concludes with cost analysis and context features (AI agent optimization). Each phase delivers a coherent, verifiable capability that builds toward the core value: session continuity for AI-assisted purchase decisions.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [x] **Phase 1: Schema and Core Types** - Database foundation for topology modeling (REWRITE)
- [x] **Phase 2: CLI Scaffolding and Basic Commands** - Usable topology management
- [x] **Phase 3: Topology Versioning** - Fork, tag, and diff topologies
- [x] **Phase 4: Analysis Functions** - Redundancy, failure sim, RPO, capacity
- [x] **Phase 5: Decision Integration** - Link decisions to topologies
- [ ] **Phase 6: Cost and Context** - Cost analysis and AI context features

## Phase Details

### Phase 1: Schema and Core Types
**Goal**: Database tables and Rust types exist for all topology entities. Codebase rewritten from scratch (clean foundation, no legacy patterns).
**Depends on**: Nothing (first phase)
**Requirements**: INFRA-01, INFRA-02, INFRA-04, INFRA-05
**Success Criteria** (what must be TRUE):
  1. Running `sp` shows help with nested command structure (topology, node, volume, etc.)
  2. Database file `.sp/decisions.db` contains topology-related tables (topologies, nodes, volumes, datasets, placements, links, sync_regimes)
  3. Database has proper migration tracking via PRAGMA user_version
  4. Significant actions (create topology, add node) are logged to events table
**Plans**: 2 plans

Plans:
- [x] 01-01-PLAN.md -- Database layer with migration system, schema DDL, all topology model structs
- [x] 01-02-PLAN.md -- Event system with undo/redo engine, CLI scaffold with topology CRUD and placeholder commands

### Phase 2: CLI Scaffolding and Basic Commands
**Goal**: Users can create and populate topologies with nodes, volumes, datasets, and sync regimes
**Depends on**: Phase 1
**Requirements**: TOPO-01, TOPO-03, TOPO-04, CONT-01, CONT-02, CONT-03, CONT-04, CONT-05, CONT-06, CONT-07, CONT-08, CONT-09, CONT-10, CONT-11, CONT-12, CONT-13
**Success Criteria** (what must be TRUE):
  1. User can create a blank topology with name and description
  2. User can add nodes, volumes, and datasets to a topology
  3. User can place datasets on volumes and define sync regimes between volumes
  4. User can list topologies and show topology details (nodes, volumes, datasets, links, sync)
  5. All commands support --format=json for agent consumption
**Plans**: 4 plans

Plans:
- [x] 02-01-PLAN.md -- Entity resolver, CLI wiring, topology enhancements (update, --tree, --topology override)
- [x] 02-02-PLAN.md -- Node and volume CRUD (add, list, show, remove, update)
- [x] 02-03-PLAN.md -- Dataset and placement CRUD (add, list, show, remove, update/add/remove)
- [x] 02-04-PLAN.md -- Link and sync regime CRUD (add, list, show, remove)

### Phase 3: Topology Versioning
**Goal**: Users can fork topologies to explore alternatives and compare versions
**Depends on**: Phase 2
**Requirements**: TOPO-02, TOPO-05, TOPO-06, TOPO-07, TOPO-08, INFRA-03
**Success Criteria** (what must be TRUE):
  1. User can fork an existing topology (copies content, sets parent_id)
  2. User can tag topologies as current, exploring, or archived
  3. Only one topology can have the "current" tag at a time (enforced)
  4. User can diff two topologies to see what changed
  5. Global --format flag works on all commands (human, json)
**Plans**: 3 plans

Plans:
- [x] 03-01-PLAN.md -- Schema migration v2 (tag replaces is_active), tag/untag commands, list/show updates
- [x] 03-02-PLAN.md -- Fork command with deep copy and ID remapping for all 6 entity types
- [x] 03-03-PLAN.md -- Diff engine with field-level detail and entity filtering, lineage tree and log commands

### Phase 4: Analysis Functions
**Goal**: Users can analyze topologies for redundancy, failures, RPO compliance, and capacity
**Depends on**: Phase 3
**Requirements**: ANLZ-01, ANLZ-03, ANLZ-04, ANLZ-05
**Success Criteria** (what must be TRUE):
  1. User can check if dataset redundancy requirements (min_copies, min_locations) are met
  2. User can simulate a node failure and see which datasets lose copies
  3. User can check if sync schedules satisfy dataset max_rpo requirements
  4. User can project capacity growth and see months until volumes are full
**Plans**: 2 plans

Plans:
- [x] 04-01-PLAN.md -- Analysis engine with redundancy and capacity commands (result types, pure functions, CLI wiring)
- [x] 04-02-PLAN.md -- RPO compliance, failure simulation, and combined dashboard command

### Phase 5: Decision Integration
**Goal**: Users can track decisions with topology comparisons and constraint checking
**Depends on**: Phase 4
**Requirements**: DEC-01, DEC-02, DEC-03, DEC-04, DEC-05, DEC-06, DEC-07, DEC-08, DEC-09, DEC-10, DEC-11, ANLZ-02, ANLZ-08
**Success Criteria** (what must be TRUE):
  1. User can create, update, and close decisions with bd-like lifecycle
  2. User can add constraints (budget, noise) to decisions
  3. User can mark topologies as "under consideration" for a decision
  4. User can compare two topologies side-by-side including constraint analysis
  5. User can close a decision with chosen topology and rationale (or abandon)
**Plans**: 3 plans

Plans:
- [x] 05-01-PLAN.md -- Schema migration v3, decision model structs, event system registration, entity resolver, node field extensions
- [x] 05-02-PLAN.md -- Decision CLI module with CRUD commands and constraint/topology management
- [x] 05-03-PLAN.md -- Decision lifecycle (choose/abandon/reopen), constraint checking, topology comparison

### Phase 6: Cost and Context
**Goal**: Users can analyze costs and agents can get full context via sp prime
**Depends on**: Phase 5
**Requirements**: CAT-01, CAT-02, CAT-03, CAT-04, CAT-05, CAT-06, CAT-07, ANLZ-06, ANLZ-07, CTX-01, CTX-02, CTX-03, TOPO-09, TOPO-10, TOPO-11
**Success Criteria** (what must be TRUE):
  1. Catalog commands work (add item, show, list, search) - verify existing or implement
  2. User can add price observations with type (one-time, monthly, annual)
  3. User can analyze topology cost (one-time + recurring from linked catalog items)
  4. User can check bandwidth requirements (can links support sync regimes?)
  5. Running `sp status` shows current topology, open decisions, catalog stats
  6. Running `sp prime` outputs AI-optimized context dump for session continuity
  7. User can import/export topologies as YAML and view ASCII diagram
**Plans**: 5 plans

Plans:
- [ ] 06-01-PLAN.md -- Schema migration v4, CatalogItem/Price models, event system, entity resolver, serde_yaml_ng dep
- [ ] 06-02-PLAN.md -- Catalog CLI with item CRUD (add/show/list/search) and price management (add/list)
- [ ] 06-03-PLAN.md -- Bandwidth analysis and cost analysis with per-entity breakdown, summary, and TCO
- [ ] 06-04-PLAN.md -- Status dashboard, prime agent bootstrap, and current topology shortcut
- [ ] 06-05-PLAN.md -- YAML topology export/import and ASCII diagram command

## Progress

**Execution Order:**
Phases execute in numeric order: 1 -> 2 -> 3 -> 4 -> 5 -> 6

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Schema and Core Types | 2/2 | Complete | 2026-02-07 |
| 2. CLI Scaffolding and Basic Commands | 4/4 | Complete | 2026-02-07 |
| 3. Topology Versioning | 3/3 | Complete | 2026-02-07 |
| 4. Analysis Functions | 2/2 | Complete | 2026-02-07 |
| 5. Decision Integration | 3/3 | Complete | 2026-02-08 |
| 6. Cost and Context | 0/5 | Not started | - |

---
*Roadmap created: 2026-02-06*
*Depth: standard (5-8 phases)*
*Coverage: 58/58 v1 requirements mapped*

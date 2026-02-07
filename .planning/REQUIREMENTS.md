# Requirements: Storage Planner

**Defined:** 2026-02-05
**Core Value:** Session continuity for AI-assisted purchase decisions

## v1 Requirements

### Topologies (TOPO)

- [ ] **TOPO-01**: Create blank topology with name and description
- [ ] **TOPO-02**: Fork topology from existing (copies content, sets parent_id)
- [ ] **TOPO-03**: Show topology details (nodes, volumes, datasets, sync regimes)
- [ ] **TOPO-04**: List all topologies with status indicators
- [ ] **TOPO-05**: Tag topology (current, exploring, archived)
- [ ] **TOPO-06**: Untag topology
- [ ] **TOPO-07**: Enforce single "current" tag via database constraint
- [ ] **TOPO-08**: Diff two topologies showing changes
- [ ] **TOPO-09**: Map topology as visual ASCII diagram
- [ ] **TOPO-10**: Import topology from YAML file
- [ ] **TOPO-11**: Export topology to YAML file

### Topology Content (CONT)

- [ ] **CONT-01**: Add node with name, location, optional product reference
- [ ] **CONT-02**: Remove node from topology
- [ ] **CONT-03**: List nodes in topology
- [ ] **CONT-04**: Add volume to node with capacity, type, optional product
- [ ] **CONT-05**: Remove volume from topology
- [ ] **CONT-06**: Add dataset with size, criticality, requirements (min_copies, min_locations, max_rpo)
- [ ] **CONT-07**: Remove dataset from topology
- [ ] **CONT-08**: Place dataset on volume
- [ ] **CONT-09**: Unplace dataset from volume
- [ ] **CONT-10**: Add network link between nodes with bandwidth, latency, type
- [ ] **CONT-11**: Remove network link
- [ ] **CONT-12**: Add sync regime (source→target, method, schedule, direction)
- [ ] **CONT-13**: Remove sync regime

### Decisions (DEC)

- [ ] **DEC-01**: Create decision with title and optional parent (hierarchy)
- [ ] **DEC-02**: Show decision details including considered topologies
- [ ] **DEC-03**: List decisions with status filter
- [ ] **DEC-04**: Update decision title/description
- [ ] **DEC-05**: Add constraint to decision (budget, noise, etc.)
- [ ] **DEC-06**: Remove constraint from decision
- [ ] **DEC-07**: Consider topology for decision (add to comparison set)
- [ ] **DEC-08**: Unconsider topology (remove from comparison set)
- [ ] **DEC-09**: Close decision with chosen topology and reason
- [ ] **DEC-10**: Close decision without choice (abandoned/moot)
- [ ] **DEC-11**: Reopen closed decision

### Analysis (ANLZ)

- [ ] **ANLZ-01**: Analyze topology redundancy (dataset copies vs requirements)
- [ ] **ANLZ-02**: Analyze topology with decision constraints (budget, noise)
- [ ] **ANLZ-03**: Simulate node failure (what datasets lose copies?)
- [ ] **ANLZ-04**: Check RPO compliance (sync schedules vs dataset max_rpo)
- [ ] **ANLZ-05**: Project capacity (growth rate, months until full)
- [ ] **ANLZ-06**: Analyze bandwidth (can links support sync regimes?)
- [ ] **ANLZ-07**: Analyze cost (one-time + recurring from catalog)
- [ ] **ANLZ-08**: Compare two topologies side-by-side

### Catalog (CAT)

- [ ] **CAT-01**: Add item with name, category, specs, URL
- [ ] **CAT-02**: Show item details with price history
- [ ] **CAT-03**: List items with category filter
- [ ] **CAT-04**: Search items by query
- [ ] **CAT-05**: Add price observation (amount, source, condition, type)
- [ ] **CAT-06**: List price history for item
- [ ] **CAT-07**: Support price types (one-time, monthly, annual)

### Context (CTX)

- [ ] **CTX-01**: Show status overview (current topo, open decisions, catalog stats)
- [ ] **CTX-02**: Show AI-optimized context dump (sp prime)
- [ ] **CTX-03**: Show/set current topology shortcut

### Infrastructure (INFRA)

- [ ] **INFRA-01**: Database schema with all tables (topologies, nodes, volumes, datasets, etc.)
- [ ] **INFRA-02**: Schema migration tracking (PRAGMA user_version)
- [ ] **INFRA-03**: Global --format flag (human, json) on all commands
- [ ] **INFRA-04**: Nested CLI structure (App + Command enum)
- [ ] **INFRA-05**: Event logging for significant actions

## v2 Requirements

### Price Refresh

- **PRICE-01**: Refresh prices from Best Buy API
- **PRICE-02**: Refresh prices from eBay API
- **PRICE-03**: Staleness warnings in sp prime

### Advanced Analysis

- **ANLZ-10**: Bandwidth analysis with path finding
- **ANLZ-11**: TCO projection over N years
- **ANLZ-12**: What-if analysis (add/remove components)

### Import/Export

- **SYNC-01**: Export all data to JSONL for git sync
- **SYNC-02**: Import from JSONL

## Out of Scope

| Feature | Reason |
|---------|--------|
| GUI or web interface | CLI only, designed for AI agents |
| Real-time sync daemon | Tool is stateless between commands |
| Multi-user collaboration | Single-user local tool |
| Cloud storage | SQLite database, local only |
| Automatic purchasing | Provides recommendations, human executes |
| Interactive wizards | Hurt agent usability; simple commands preferred |
| Undo/redo stack | Complexity; use versioning instead |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| INFRA-01 | Phase 1 | Pending |
| INFRA-02 | Phase 1 | Pending |
| INFRA-04 | Phase 1 | Pending |
| INFRA-05 | Phase 1 | Pending |
| TOPO-01 | Phase 2 | Pending |
| TOPO-03 | Phase 2 | Pending |
| TOPO-04 | Phase 2 | Pending |
| CONT-01 | Phase 2 | Pending |
| CONT-02 | Phase 2 | Pending |
| CONT-03 | Phase 2 | Pending |
| CONT-04 | Phase 2 | Pending |
| CONT-05 | Phase 2 | Pending |
| CONT-06 | Phase 2 | Pending |
| CONT-07 | Phase 2 | Pending |
| CONT-08 | Phase 2 | Pending |
| CONT-09 | Phase 2 | Pending |
| CONT-10 | Phase 2 | Pending |
| CONT-11 | Phase 2 | Pending |
| CONT-12 | Phase 2 | Pending |
| CONT-13 | Phase 2 | Pending |
| TOPO-02 | Phase 3 | Pending |
| TOPO-05 | Phase 3 | Pending |
| TOPO-06 | Phase 3 | Pending |
| TOPO-07 | Phase 3 | Pending |
| TOPO-08 | Phase 3 | Pending |
| INFRA-03 | Phase 3 | Pending |
| ANLZ-01 | Phase 4 | Pending |
| ANLZ-03 | Phase 4 | Pending |
| ANLZ-04 | Phase 4 | Pending |
| ANLZ-05 | Phase 4 | Pending |
| DEC-01 | Phase 5 | Pending |
| DEC-02 | Phase 5 | Pending |
| DEC-03 | Phase 5 | Pending |
| DEC-04 | Phase 5 | Pending |
| DEC-05 | Phase 5 | Pending |
| DEC-06 | Phase 5 | Pending |
| DEC-07 | Phase 5 | Pending |
| DEC-08 | Phase 5 | Pending |
| DEC-09 | Phase 5 | Pending |
| DEC-10 | Phase 5 | Pending |
| DEC-11 | Phase 5 | Pending |
| ANLZ-02 | Phase 5 | Pending |
| ANLZ-08 | Phase 5 | Pending |
| CAT-05 | Phase 6 | Pending |
| CAT-07 | Phase 6 | Pending |
| ANLZ-06 | Phase 6 | Pending |
| ANLZ-07 | Phase 6 | Pending |
| CTX-01 | Phase 6 | Pending |
| CTX-02 | Phase 6 | Pending |
| CTX-03 | Phase 6 | Pending |
| TOPO-09 | Phase 6 | Pending |
| TOPO-10 | Phase 6 | Pending |
| TOPO-11 | Phase 6 | Pending |

**Coverage:**
- v1 requirements: 52 total
- Mapped to phases: 52
- Unmapped: 0 ✓

---
*Requirements defined: 2026-02-05*
*Last updated: 2026-02-05 after design exploration and research*

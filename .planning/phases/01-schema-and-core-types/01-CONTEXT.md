# Phase 1: Schema and Core Types - Context

**Gathered:** 2026-02-06
**Status:** Ready for planning

<domain>
## Phase Boundary

Database tables and Rust types for all topology entities: topologies, nodes, volumes, datasets, placements, links, and sync regimes. Includes migration tracking, event logging with undo/redo capability, and basic CLI help structure. Fresh database start is acceptable — no need to preserve existing .sp/decisions.db data.

</domain>

<decisions>
## Implementation Decisions

### Entity modeling
- Datasets are **independent entities** that get "placed" on volumes via a placement junction table — not children of a single volume. Supports multi-volume replication.
- Sync regimes operate at **dataset level** (per-dataset placement pair), not volume-to-volume. e.g., "photos on NAS → photos on backup drive, daily." Enables precise RPO analysis per dataset.
- Nodes carry a **full hardware profile**: name, role, physical location, available bays, interface types, power draw. Not just name+role.
- Volumes are **rich**: capacity (raw + usable), filesystem type, RAID level/pool type (ZFS mirror, RAID5, etc.), AND a foreign key to catalog items. A volume can reference the actual drive being considered for purchase.
- Links model **full network characteristics**: bandwidth, connection type (LAN/WAN/USB/Thunderbolt), latency, metered/unmetered, cost-per-GB. Supports bandwidth cost analysis in Phase 6.
- **Strictly typed columns** throughout — no JSON metadata blobs. Schema is self-documenting. Changes require migrations.

### Naming and identity
- Typically **few topologies** (2-5): one "current" plus a couple of forks/alternatives
- All naming and identity decisions at Claude's discretion, considering:
  - How users reference entities in CLI commands (UUIDs vs names vs numeric IDs)
  - Name scoping (global vs scoped to parent topology)
  - Active/default topology concept for implicit context
  - Volume reference style (path-style vs flags)
  - Naming conventions (slug-style vs free-form)
  - ID scheme consistency with existing codebase

### Migration strategy
- **Fresh start OK** — existing .sp/decisions.db data does not need preservation
- SQLite remains the database (local CLI tool, no server needed, portable file)
- Migration tracking method is flexible — PRAGMA user_version or migration table, Claude's discretion
- Schema organization (unified vs versioned steps) at Claude's discretion based on maintainability across 6 phases

### Event logging
- Events store both **structured JSON payload** and a **human-readable summary** — best of both worlds
- Events track **source**: user, agent, import, or migration — important for AI session continuity
- Events store **before/after state** for full undo/redo capability
- Event schema should be **redesigned from scratch** (not extending existing events table)
- **Full undo/redo in Phase 1**: `sp undo` and `sp redo` commands ship in this phase
- Multiple levels of undo — can undo repeatedly, redo after undo, standard undo/redo stack behavior

### Claude's Discretion
- Dataset properties (size, growth rate, min_copies, min_locations, max_rpo) — design based on what Phase 4 analysis functions need
- Placement table properties (pure junction vs role/priority fields) — based on sync regime and analysis needs
- Event logging granularity (major actions only vs all mutations) — based on what's useful for decision tracking
- All naming/identity decisions listed above

</decisions>

<specifics>
## Specific Ideas

- User wants the schema rich enough to support deep analysis: full hardware profiles on nodes, full network modeling on links, RAID-aware volumes, catalog-linked volumes
- Undo/redo is a first-class feature, not an afterthought — events are the backbone
- Few topologies expected (2-5), so design for clarity over scale
- AI agent awareness built in from the start (source tracking on events)

</specifics>

<deferred>
## Deferred Ideas

- PostgreSQL as an alternative backend — user mentioned openness to it, but SQLite fits the local CLI use case. Revisit only if scaling needs change.

</deferred>

---

*Phase: 01-schema-and-core-types*
*Context gathered: 2026-02-06*

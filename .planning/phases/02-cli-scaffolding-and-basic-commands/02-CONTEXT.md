# Phase 2: CLI Scaffolding and Basic Commands - Context

**Gathered:** 2026-02-06
**Status:** Ready for planning

<domain>
## Phase Boundary

Users can create and populate topologies with nodes, volumes, datasets, placements, links, and sync regimes through working CLI commands. All entity CRUD operates on the active topology by default. Commands support `--format=json` for agent consumption. Phase 1 placeholders become real implementations.

NOT in scope: forking/tagging topologies (Phase 3), analysis functions (Phase 4), decision tracking (Phase 5), cost analysis (Phase 6).

</domain>

<decisions>
## Implementation Decisions

### Entity referencing
- Entities referenced by **name or ID** everywhere — try name lookup first, fall back to UUID prefix match
- ID prefix matching: Claude's discretion on minimum length (likely 4-8 chars, error on ambiguity)
- All entity references resolve the same way — parent refs (--node, --dataset) use the same name-or-ID resolver
- Names are **slug-like only**: alphanumeric, hyphens, underscores. No spaces, no quoting needed in shell

### Disambiguation
- Volumes disambiguated via `--node` flag when names collide across nodes (not path syntax)
- Error if a volume name is ambiguous without `--node`, listing the options
- All commands default to active topology, with `--topology=name` override to target a different one without switching

### Cascade behavior
- Deleting an entity with dependents: **warn then cascade** — output lists what will be deleted, but proceeds without interactive prompt
- Undo is available for recovery

### Placement commands
- Claude's Discretion: decide between dedicated `sp placement` command or `sp dataset place` subcommand — pick what works best for agents and humans

### Link naming
- Claude's Discretion: auto-name from nodes (e.g., `mac-mini--nas`) vs user-provided names

### Create output
- Claude's Discretion: whether to show entity ID in text output or only in JSON mode

### Show/list output
- `sp topology show` displays **summary by default** (name, description, active, counts), with `--tree` or `--verbose` flag for full hierarchy (nodes -> volumes -> datasets)
- List commands: Claude's Discretion on format (compact one-liner vs table with headers)
- `sp node show` displays **node properties + its volumes** inline
- Size display: Claude's Discretion on auto-scaling (e.g., "4.0 TB", "500 GB")

### Capacity input format
- Claude's Discretion on all capacity/size parsing details:
  - Accept human units (4TB, 500GB) — likely also raw bytes if all digits
  - Binary vs decimal (TB vs TiB) — likely accept both, with TiB for binary
  - Bandwidth: likely accept both networking (10Gbps) and storage (1GB/s) conventions
  - Usable capacity: likely manual `--usable` flag, no RAID auto-calculation in Phase 2

### Update commands
- **Nodes, volumes, datasets**: support update-in-place (partial updates, only change specified fields)
- **Placements, links, sync regimes**: immutable — delete and recreate to change
- Renaming: Claude's Discretion (likely allowed since IDs are the real identifiers)
- Topology update: Claude's Discretion (likely yes for description changes)
- Cross-entity validation on update: Claude's Discretion (likely defer to Phase 4 analysis commands, no warnings in Phase 2)

### Claude's Discretion
- Placement command structure (dedicated vs subcommand)
- Link naming strategy
- Create output format (ID in text or JSON-only)
- List command format (one-liner vs table)
- Size display format
- Capacity/bandwidth parsing details
- ID prefix minimum length
- Rename support
- Topology update command
- Cross-entity validation warnings

</decisions>

<specifics>
## Specific Ideas

- Agent-friendliness is a first-class concern — `--format=json` on all commands, no interactive prompts, consistent resolver logic
- The `--topology` override avoids unnecessary `set-active` calls when agents work across topologies
- Slug-like naming chosen specifically so shell quoting is never needed
- Undo/redo already exists from Phase 1, so cascade-delete is safe

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 02-cli-scaffolding-and-basic-commands*
*Context gathered: 2026-02-06*

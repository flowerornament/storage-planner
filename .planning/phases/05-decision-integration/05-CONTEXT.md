# Phase 5: Decision Integration - Context

**Gathered:** 2026-02-07
**Status:** Ready for planning

<domain>
## Phase Boundary

Track purchase decisions with lifecycle management, link topologies as options under consideration, enforce constraints (budget, noise, power, space), and compare topologies side-by-side to reach a choice. Catalog integration and cost derivation are Phase 6 — this phase uses node-level metadata for constraint checking.

</domain>

<decisions>
## Implementation Decisions

### Decision lifecycle
- Four-state lifecycle: draft → open → decided → abandoned
- Draft state for decisions still being set up (adding constraints, considering topologies) before formally opening
- Decided = chose a topology; Abandoned = gave up / became moot
- DEC-11 reopen moves back to "open" status

### Decision hierarchy
- **Claude's Discretion**: Flat with optional parent_id is the simplest approach. No enforced tree behavior — just a foreign key for optional grouping.

### Reopen behavior
- **Claude's Discretion**: Pick the simpler approach for handling the previously chosen topology reference on reopen.

### Decision show command
- **Claude's Discretion**: Follow existing patterns in the codebase (e.g., how `topology show` works with inline details).

### Constraint system
- Supported constraint types: budget (max $), noise (max dB), power (max watts), rack units (max U)
- These are typed constraints, not arbitrary key-value pairs
- Constraints are attached to decisions and checked against considered topologies

### Constraint data source
- Add cost_estimate, noise_db, power_watts, rack_units fields to nodes (schema migration)
- Sum across topology nodes for totals when checking constraints
- Phase 6 will enrich these via catalog links, but Phase 5 uses direct node-level values
- User manually sets these values per node for now

### Constraint check output
- Pass/warn/fail with margin for each constraint
- PASS = within limit, WARN = within 10% of limit, FAIL = over limit
- Show actual value vs limit and how much headroom or overage

### Topology comparison
- Default shows analysis-only comparison (metrics side-by-side)
- --diff flag adds structural changes (builds on existing diff command)
- Comparison works on any two topologies — not scoped to a decision
- When run within a decision context, constraints are included in the comparison

### Comparison indicators
- **Claude's Discretion**: Per-metric advantage indicators or neutral data — pick what works best for CLI output.

### Comparison JSON format
- **Claude's Discretion**: Pick the format that best serves AI agent consumption (the tool is designed for agents).

### Close/choose workflow
- Closing a decision with a chosen topology records the choice but does NOT auto-tag the topology as "current" — tagging is a separate manual step
- User may not want to switch current topology immediately after deciding

### Rationale capture
- **Claude's Discretion**: Given the "session continuity" principle, requiring rationale for decided (but optional for abandoned) makes sense. Claude can pick the approach.

### Abandon reasons
- **Claude's Discretion**: Pick the simpler option (likely freeform only).

### Decision snapshot
- **Claude's Discretion**: Given the "session continuity" principle, snapshotting comparison data at close time preserves the historical record. Claude can decide whether the complexity is worth it.

</decisions>

<specifics>
## Specific Ideas

- Constraint checking should follow the same pass/warn/fail pattern already used by Phase 4 analysis commands (redundancy, RPO)
- The tool is designed for AI agent consumption — JSON output and exit codes matter
- "Budget + physical" constraints specifically chosen because they cover the main axes of hardware purchase decisions (cost, noise, power, space)
- Node-level fields (cost_estimate, noise_db, power_watts, rack_units) are the bridge to Phase 6 catalog integration

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 05-decision-integration*
*Context gathered: 2026-02-07*

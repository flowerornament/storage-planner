# Phase 4: Analysis Functions - Context

**Gathered:** 2026-02-07
**Status:** Ready for planning

<domain>
## Phase Boundary

CLI commands that evaluate a topology's health across four dimensions: redundancy coverage, failure resilience, RPO compliance, and capacity runway. Uses existing data model fields (min_copies, min_locations, max_rpo_hours, growth_rate_bytes_month, capacity_bytes, usable_bytes). This phase surfaces analysis FROM that data — it does not add new entity types or modify the schema.

</domain>

<decisions>
## Implementation Decisions

### Report output style
- Scored summary per analysis type (separate scores: Redundancy: X%, RPO: Y%, Capacity: Z%)
- Default output shows score + problems only; --verbose flag shows full per-dataset/volume breakdown
- Datasets/volumes without issues are hidden in default mode, shown in verbose
- Claude's Discretion: whether to include actionable fix suggestions per issue
- Claude's Discretion: all-clear output format (concise vs breakdown)
- Claude's Discretion: JSON output structure (include computed scores vs raw data)
- Claude's Discretion: exit codes (zero vs non-zero on issues)
- Claude's Discretion: color/symbol indicators for human output

### Command structure
- 'sp analyze' with no subcommand runs ALL analyses and gives combined report (score per type)
- Individual analyses available as subcommands (e.g., sp analyze redundancy, sp analyze rpo, etc.)
- Claude's Discretion: exact verb choice (analyze/check) and subcommand names — pick what fits existing CLI patterns
- Claude's Discretion: whether to default to current-tagged topology (consistent with existing commands)
- Claude's Discretion: whether --compare flag for side-by-side topology analysis fits this phase or belongs in Phase 5

### Failure simulation behavior
- Required argument: user specifies which node(s) to simulate failing (e.g., 'sp analyze failure nas-01')
- Multi-node failure supported: accept multiple node names to simulate simultaneous failure
- Report shows BOTH volume impact (which volumes lost, capacity gone) AND dataset impact (which datasets lose copies, which become unreachable)
- Claude's Discretion: severity tiers (degraded/at-risk/lost) vs flat listing

### Capacity projection format
- Headline metric: months-until-full per volume
- --verbose adds a timeline table showing projected usage at intervals (3, 6, 12 months)
- Datasets without growth_rate set are skipped and noted (no guessing)
- Ceiling uses usable_bytes if set, falls back to capacity_bytes
- Warning threshold: default 12 months, configurable via --warn-months=N flag
- Volumes approaching the threshold are highlighted in the report

### Claude's Discretion
- Exact verb for analysis commands (analyze vs check) based on CLI conventions
- Fix suggestion inclusion per analysis type
- JSON output structure and score inclusion
- Exit code behavior
- Color/symbol approach for terminal output
- Whether --compare belongs in Phase 4 or Phase 5
- Severity tier design for failure simulation
- Timeline table intervals for capacity projection
- All-clear output verbosity

</decisions>

<specifics>
## Specific Ideas

- "Analyze all" (no subcommand) should feel like a dashboard — quick health check of the whole topology
- Score-per-analysis approach means each dimension is independently assessed, not averaged into one number
- Failure sim should handle the "what if my NAS dies?" question directly — both infrastructure and data impact

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 04-analysis-functions*
*Context gathered: 2026-02-07*

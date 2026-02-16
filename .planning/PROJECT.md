# Storage Planner (sp)

## What This Is

A CLI tool that helps AI agents make structured purchase decisions, starting with storage hardware. Provides persistent state across sessions — topology modeling, versioned alternatives, analysis functions, decision tracking, and a catalog with pricing — so decisions don't fragment across context windows. Each new session runs `sp prime` and picks up where the last left off.

## Core Value

**Session continuity for AI-assisted decisions.** An agent can run `sp prime`, get full context of where a decision left off, and continue without re-deriving products, prices, requirements, or trade-offs.

## Current State

**Shipped:** v1.0 MVP (2026-02-16)
**Codebase:** 25,779 LOC Rust, 102 tests passing
**Tech stack:** Rust, SQLite, clap, serde_yaml_ng

v1.0 delivers the complete purchase decision workflow:
- Topology modeling (nodes, volumes, datasets, placements, links, sync regimes)
- Versioning (fork, tag, diff)
- Analysis (redundancy, failure sim, RPO, capacity, bandwidth, cost)
- Decision tracking (constraints, comparison, lifecycle)
- Catalog with price history
- AI context (`sp prime`, `sp status`, `sp current`)
- YAML import/export, ASCII diagrams

## Requirements

### Validated

- Topologies: TOPO-01 through TOPO-11 — v1.0
- Topology Content: CONT-01 through CONT-13 — v1.0
- Decisions: DEC-01 through DEC-11 — v1.0
- Analysis: ANLZ-01 through ANLZ-08 — v1.0
- Catalog: CAT-01 through CAT-07 — v1.0
- Context: CTX-01 through CTX-03 — v1.0
- Infrastructure: INFRA-01 through INFRA-05 — v1.0

### Active

(None — define in next milestone via `/gsd:new-milestone`)

### Out of Scope

- GUI or web interface — CLI only, designed for AI agents
- Real-time sync daemon — tool is stateless between commands
- Multi-user collaboration — single-user local tool
- Cloud storage — SQLite database, local only
- Automatic purchasing — provides recommendations, human executes

## Context

### The Problem

AI agents are good at reasoning but bad at: remembering context across sessions, tracking volatile data (prices change), and validating complex constraints. Each new session re-derives the same analysis.

### The Solution

Persistent structured state that any session can load: catalog with current prices, topologies modeling system state with versioning, decisions tracking what's being decided, and analysis functions that validate against requirements.

### The Test Case

Replace a Synology DS224+ NAS (32dB, slow) with external SSDs on Mac mini:
- SATA option: OWC Mercury Dual + 2x Samsung 870 EVO 4TB (~$559)
- NVMe option: OWC Express 4M2 + 2x Lexar NM790 4TB (~$587)
- Budget option: OWC Mercury Dual + 2x Samsung PM893 datacenter pulls (~$429)

Requirements: budget < $1000, noise = 0dB, capacity >= 8TB, maintain 3 copies of critical data.

## Constraints

- **Tech stack**: Rust (existing codebase), SQLite (existing database)
- **Interface**: CLI only, designed for AI agent consumption
- **Data model**: Append-only where practical (prices, decisions, events)
- **Complexity**: Simple over clever — agent needs to discover and use commands

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Topologies independent of decisions | "Current" topology doesn't belong to any decision | Good — clean separation |
| Tags instead of status field | "Current" is a tag, not a column; flexible, git-like | Good — partial unique index works |
| Requirements split: data vs decision | Dataset requirements in topology; decision constraints on decision | Good — clear boundaries |
| bd-like decisions | Simple create/close lifecycle, proven pattern | Good — natural workflow |
| Append-only prices | Price observations are immutable facts | Good — full history preserved |
| Clean rewrite over incremental | Deleted old CLI/pricing modules entirely | Good — no legacy debt |
| Event-sourced undo/redo | Generic handler using event_type suffix | Mixed — works for simple cases, edge cases with cascaded FKs |
| YAML for topology exchange | serde_yaml_ng for import/export | Good — human-readable, agent-friendly |
| Markdown for sp prime | Not JSON — agent bootstrap is read by LLMs | Good — natural for agents |
| Slug validation for names | Alphanumeric, hyphens, underscores only | Good — prevents quoting issues |

## Tech Debt (from v1.0)

- **Undo/redo edge cases** (5 items): Cascaded FK issues with delete+insert pattern, redo fails for multi-entity events
- **Prime guide accuracy** (5 items): Remaining command syntax errors in static guide
- **Analysis/UX** (5 items): Capacity overcounting, negative value acceptance, missing help text

See `.planning/milestones/v1.0-MILESTONE-AUDIT.md` for full details.

---
*Last updated: 2026-02-16 after v1.0 milestone*

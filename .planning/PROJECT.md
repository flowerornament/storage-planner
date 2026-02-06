# Storage Planner (sp)

## What This Is

A CLI tool that helps AI agents make structured purchase decisions, starting with storage hardware. The tool provides persistent state across sessions so decisions don't fragment—each new session can pick up where the last left off via `sp prime`.

The immediate use case is replacing a loud Synology NAS with silent external SSDs, but the patterns generalize to other purchase/configuration decisions.

## Core Value

**Session continuity for AI-assisted decisions.** An agent can run `sp prime`, get full context of where a decision left off, and continue without re-deriving products, prices, requirements, or trade-offs.

## Requirements

### Validated

(None yet — ship to validate)

### Active

#### Topologies
- [ ] Model system state as versioned graphs (nodes, volumes, datasets, sync regimes, links)
- [ ] Fork topologies to explore alternatives (parent_id lineage)
- [ ] Tag topologies (current, exploring, archived) with unique "current" enforcement
- [ ] Diff topologies to see what changed
- [ ] Import/export topologies as YAML

#### Topology Content
- [ ] Nodes with location and optional product reference
- [ ] Volumes with capacity, type, optional product reference
- [ ] Datasets with size, criticality, and requirements (min_copies, min_locations, max_rpo)
- [ ] Dataset placements (which datasets live on which volumes)
- [ ] Network links between nodes (bandwidth, latency, type)
- [ ] Sync regimes (source→target, method, schedule, direction)

#### Decisions
- [ ] bd-like lifecycle: create, update, close
- [ ] Hierarchy for complex decisions (parent_id)
- [ ] Track topologies under consideration (with staleness detection)
- [ ] Decision constraints (budget, noise, etc.) separate from data requirements
- [ ] Close with chosen topology and resolution rationale

#### Analysis
- [ ] Redundancy analysis (dataset copies vs requirements)
- [ ] Failure simulation (what happens if node X dies?)
- [ ] RPO compliance (sync schedules vs dataset max_rpo)
- [ ] Capacity projection (growth rate, months until full)
- [ ] Bandwidth analysis (can links support sync regimes?)
- [ ] Cost analysis (one-time + recurring, with TCO projection)
- [ ] Constraint checking (decision constraints like budget, noise)

#### Catalog
- [ ] Items with category, specs, URL
- [ ] Price observations (append-only, with source, condition)
- [ ] Price types (one-time, monthly, annual) for services
- [ ] Price refresh from APIs (Best Buy, eBay)
- [ ] Search catalog by query

#### Context
- [ ] `sp status` — overview of current state
- [ ] `sp prime` — AI-optimized context dump
- [ ] `sp current` — show/set current topology

### Out of Scope

- GUI or web interface — CLI only, designed for AI agents
- Real-time sync daemon — tool is stateless between commands
- Multi-user collaboration — single-user local tool
- Cloud storage of data — SQLite database, local only
- Automatic purchasing — provides recommendations, human executes

## Context

### The Problem

AI agents are good at reasoning but bad at:
- Remembering context across sessions (context window limits)
- Tracking volatile data (prices change, training data is stale)
- Validating complex constraints (does this topology meet redundancy requirements?)

Each new Claude session re-derives the same analysis: researches products, checks prices, evaluates trade-offs. The decision fragments across threads.

### The Solution

Persistent structured state that any session can load:
- **Catalog** with current prices (not training data)
- **Topologies** modeling system state with versioning
- **Decisions** tracking what's being decided and why
- **Analysis** functions that validate against requirements

The tool is a "pure function" — no hidden state, all state in the database, explicit inputs and outputs.

### Existing Code

The codebase has:
- Items, Prices, Configurations, Decisions, Events tables
- CLI commands for item/price/config management
- Best Buy and eBay API integrations for pricing
- Basic storage analysis module

What's new:
- Topology modeling (the graph of nodes, volumes, datasets, sync regimes)
- Richer analysis (redundancy, failure sim, RPO, bandwidth)
- Decision tracking decoupled from topologies
- Constraints on decisions (budget, noise)

### The Test Case

Replace a Synology DS224+ NAS (32dB, slow) with external SSDs on Mac mini:
- SATA option: OWC Mercury Dual + 2× Samsung 870 EVO 4TB (~$559)
- NVMe option: OWC Express 4M2 + 2× Lexar NM790 4TB (~$587)
- Budget option: OWC Mercury Dual + 2× Samsung PM893 datacenter pulls (~$429)

Requirements: budget < $1000, noise = 0dB, capacity >= 8TB, maintain 3 copies of critical data.

## Constraints

- **Tech stack**: Rust (existing codebase), SQLite (existing database)
- **Interface**: CLI only, designed for AI agent consumption
- **Data model**: Append-only where practical (prices, decisions, events)
- **Complexity**: Simple over clever—agent needs to discover and use commands

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Topologies independent of decisions | "Current" topology doesn't belong to any decision; topologies exist before/during/after decisions | — Pending |
| Tags instead of status field | "Current" is a tag, not a column; flexible, git-like | — Pending |
| Requirements split: data vs decision | Dataset requirements (min_copies) in topology; decision constraints (budget) on decision | — Pending |
| bd-like decisions | Simple create/close lifecycle, proven pattern, no complex verdict modeling | — Pending |
| Append-only prices | Price observations are immutable facts; never update, only add new | — Pending |

---
*Last updated: 2026-02-05 after design exploration*

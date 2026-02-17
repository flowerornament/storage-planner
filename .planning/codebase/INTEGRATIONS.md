# External Integrations

**Analysis Date:** 2026-01-31
**Updated:** 2026-02-16

## Architecture Decision

Pricing API integrations (Best Buy, eBay) have moved to Herald (`~/code/herald`). Herald owns the product catalog domain — PostgreSQL for storage, Oban for background price refresh, MCP tools for querying.

`sp` retains its local SQLite catalog for manual entry and offline use. Long-term, `sp` may query Herald's catalog via MCP.

See PROJECT.md "Architecture Decision: Catalog Lives in Herald" for rationale.

## Data Storage

**Database:**
- Type: SQLite 3
- Location: `.sp/decisions.db` (configurable via `SP_DIR` or `--dir` flag)
- Client: `rusqlite` 0.31 with bundled SQLite
- Features:
  - Foreign key constraints enabled
  - Write-Ahead Logging (WAL mode)

**Tables:**
- `catalog_items` — Products with name, category, specs (JSON), URL
- `prices` — Append-only price observations (item_id, amount_cents, source, timestamp)
- `decisions` — Purchase decision lifecycle tracking
- Topology tables: nodes, volumes, datasets, placements, links, sync_regimes
- `events` — Immutable audit log for all mutations

**File Storage:**
- `.sp/` directory, local filesystem only
- YAML exports via `sp export`

## Environment Configuration

| Variable | Purpose | Required |
|----------|---------|----------|
| `SP_DIR` | Path to database directory | No (defaults to `.sp/` in cwd) |

## CI/CD

- `justfile` with `check` target (fmt, lint, test)
- Nix flake for reproducible builds
- No GitHub Actions

---

*Integration audit: 2026-01-31*
*Revised: 2026-02-16 — pricing APIs moved to Herald*

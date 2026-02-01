# External Integrations

**Analysis Date:** 2026-01-31

## APIs & External Services

**Best Buy Products API:**
- Service: Best Buy developer products API for electronics price/availability lookup
- What it's used for: Fetch current prices and product specs for SSDs and storage devices
- SDK/Client: `ureq` 2.x (synchronous HTTP client)
- Auth: Simple API key in query parameter
- Env var: `SP_BESTBUY_API_KEY`
- Endpoint: `https://api.bestbuy.com/v1/products`
- Implementation: `src/pricing/bestbuy.rs`
- Query formats supported: Search by name, SKU, or UPC
- Response format: JSON with product SKU, name, manufacturer, UPC, sale price, regular price, details, URL, image, category

**eBay Browse API:**
- Service: eBay marketplace item search and price lookup
- What it's used for: Search for storage products across eBay marketplace, including used/refurbished options
- SDK/Client: `ureq` 2.x (synchronous HTTP client)
- Auth: OAuth2 Client Credentials flow
- Env vars:
  - `SP_EBAY_APP_ID` - OAuth2 application ID
  - `SP_EBAY_CERT_ID` - OAuth2 certificate ID
- Token endpoint: `https://api.ebay.com/identity/v1/oauth2/token`
- Browse API: `https://api.ebay.com/buy/browse/v1/item_summary/search`
- Implementation: `src/pricing/ebay.rs`
- Features:
  - OAuth2 token caching (`.sp/ebay_token.json`)
  - Automatic token refresh on expiry
  - Search by keyword with filtering
- Response format: JSON with item title, price (current/minimum), condition, listing status, shipping info

## Data Storage

**Database:**
- Type: SQLite 3
- Location: `.sp/decisions.db` (configurable via `SP_DIR` or `--dir` flag)
- Client: `rusqlite` 0.31 with bundled SQLite
- Features:
  - Foreign key constraints enabled
  - Write-Ahead Logging (WAL mode) for concurrency
  - Full-text search via FTS5 virtual table for items
  - Automatic FTS index maintenance via triggers

**Database Schema (`src/core/db.rs`):**
- `items` table: Catalog of purchasable products
  - Stores: id, name, category, brand, specs (JSON), tags (JSON array), metadata (JSON), timestamps
  - Indexes: category, archived status
  - Full-text search: id, name, category, brand, tags via FTS5

- `prices` table: Append-only price observations
  - Stores: id, item_id (FK), source ('ebay', 'bestbuy', 'amazon', 'manual'), price, currency, condition, URL, timestamp, metadata
  - Indexes: item_id, observed_at timestamp
  - Purpose: Preserve complete price history

- `configurations` table: Named compositions of items
  - Stores: id, name, domain ('storage', 'computing'), items (JSON array with quantities), domain_data (JSON), metadata, is_current flag, timestamps
  - Indexes: is_current status, archived
  - Purpose: Bundle items into system configurations

- `decisions` table: Recorded purchase choices (append-only)
  - Stores: id, purpose, status ('active', 'decided', 'abandoned'), options (JSON mapping names to config IDs), chosen_option, chosen_config_id (FK), rationale, decided_at timestamp, decided_by, metadata
  - Indexes: status
  - Purpose: Audit trail of decisions made

- `events` table: Immutable audit log (append-only)
  - Stores: id, event_type, entity_type ('item', 'price', 'configuration', 'decision'), entity_id, payload (JSON), timestamp, actor
  - Indexes: entity (type + id), timestamp, event_type
  - Purpose: Complete event log for all mutations

**File Storage:**
- Local filesystem only
- `.sp/` directory structure:
  - `decisions.db` - Main SQLite database
  - `ebay_token.json` - Cached eBay OAuth2 token (created on first eBay API call)
- No cloud storage integration
- YAML exports: Generated on-demand via `sp sync --export` command (read-only snapshots)

**Caching:**
- eBay OAuth2 token caching in `.sp/ebay_token.json`
  - Cached tokens validated before use
  - Automatic refresh when expired
  - No external cache service (filesystem-based)

## Authentication & Identity

**Auth Provider:**
- Custom implementations per API

**Best Buy:**
- Type: API key (simple authentication)
- Mechanism: Query parameter in GET requests
- Env var: `SP_BESTBUY_API_KEY`
- No session management required

**eBay:**
- Type: OAuth2 Client Credentials
- Mechanism: POST to token endpoint with credentials, returns access token
- Implementation: `src/pricing/ebay.rs` lines 48-112
- Token caching: `.sp/ebay_token.json` with expiry tracking
- Token refresh: Automatic via `chrono::Duration` comparison
- Env vars: `SP_EBAY_APP_ID`, `SP_EBAY_CERT_ID`

## Monitoring & Observability

**Error Tracking:**
- None integrated (error handling via `anyhow` crate locally)

**Logs:**
- Console-based output via `console` crate
- Styled output: Green checkmarks (✓), yellow warnings (!), colored text
- Structured logging: None (simple print statements in CLI commands)
- No persistent logging to files

## CI/CD & Deployment

**Hosting:**
- None specified (CLI tool, self-hosted)

**CI Pipeline:**
- `justfile` contains `check` target (runs fmt, lint, test)
- No GitHub Actions or external CI integration detected
- Expected pre-commit: `just check`

**Build Artifacts:**
- Single binary: `sp` built by Cargo
- Distribution: Self-contained (SQLite bundled)
- Release profile optimizations enabled (LTO, stripping)

## Environment Configuration

**Required Environment Variables:**

For full functionality:
- `SP_BESTBUY_API_KEY` - Best Buy API key (optional, fallback to agent mode if missing)
- `SP_EBAY_APP_ID` - eBay OAuth2 app ID (optional, fallback to agent mode if missing)
- `SP_EBAY_CERT_ID` - eBay OAuth2 certificate ID (optional, fallback to agent mode if missing)

For non-default database location:
- `SP_DIR` - Path to `.sp` directory (default: `.sp` in current working directory)

**Secrets Location:**
- Environment variables only (no `.env` file support detected)
- Best practice: Set via shell profile, CI/CD secrets, or environment loader
- OAuth2 tokens cached locally in `.sp/ebay_token.json` after first successful auth

## Webhooks & Callbacks

**Incoming:**
- None detected

**Outgoing:**
- None detected (CLI tool, not a server)

## API Fallback Behavior

**When APIs Unavailable:**

Both Best Buy and eBay APIs have graceful fallback to agent mode:
- Implementation: `src/pricing/fallback.rs`
- Triggers: Missing API keys, API errors, product not found, rate limiting
- Behavior:
  - Generates structured prompt for human agent
  - Provides JSON schema for manual data entry
  - Stores manual prices with `source='manual'` in database
  - CLI mode continues with user-provided input

**API Check:**
- `fetch_product()` in `src/pricing/mod.rs` tries Best Buy first, then eBay
- Returns first successful result or `None` if both fail
- `available_sources()` lists which APIs are configured

## Pricing API Priority Order

1. Best Buy (simpler API, good for electronics)
2. eBay (larger marketplace, includes used/refurbished)
3. Manual entry (fallback via agent prompts)

---

*Integration audit: 2026-01-31*

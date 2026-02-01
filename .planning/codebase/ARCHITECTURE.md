# Architecture

**Analysis Date:** 2026-01-31

## Pattern Overview

**Overall:** Layered architecture with domain-driven design and append-only event sourcing

**Key Characteristics:**
- Separation between domain-agnostic core (`core/`) and domain-specific extensions (`domains/`)
- All mutations recorded as append-only events for auditability
- SQLite as single source of truth; YAML exports are read-only snapshots
- CLI acts as the enforcement layer - all business logic flows through command validation
- Pluggable pricing integrations (Best Buy, eBay) with fallback to manual entry

## Layers

**CLI Layer (Command Enforcement):**
- Purpose: Enforce workflow invariants and validate user inputs before mutations
- Location: `src/cli/`
- Contains: Command implementations (`item.rs`, `price.rs`, `config.rs`, `decide.rs`, etc.)
- Depends on: Core models, database, domains, pricing integrations
- Used by: Main entry point (`src/main.rs`)
- Pattern: Each command is a subcommand with structured args (using `clap`)

**Core Layer (Domain-Agnostic Models):**
- Purpose: Provide fundamental abstractions that work across any purchase decision domain
- Location: `src/core/`
- Contains:
  - `models.rs`: Item, Price, Configuration, Decision, Event (core entities)
  - `db.rs`: SQLite connection management, transaction support, schema migrations
  - `events.rs`: Append-only event logging system
  - `specs.rs`: Parsing typed attributes (capacity, speed, noise, etc.)
- Depends on: SQLite via rusqlite, chrono for timestamps, uuid for IDs
- Used by: CLI layer, domains layer

**Domain Layer (Storage-Specific Analysis):**
- Purpose: Provide specialized models and analysis for specific categories (storage, compute, networking, etc.)
- Location: `src/domains/storage/`
- Contains:
  - `models.rs`: Node, Volume, Dataset, SyncRegime (storage topology concepts)
  - `analysis.rs`: Storage-specific calculations and recommendations
- Depends on: Core models, can extend Configuration with domain_data
- Used by: CLI layer when building/analyzing storage configurations

**Pricing Integration Layer:**
- Purpose: Fetch product information and prices from external APIs
- Location: `src/pricing/`
- Contains:
  - `bestbuy.rs`: Best Buy API integration
  - `ebay.rs`: eBay API integration (OAuth)
  - `url_parser.rs`: Extract retailer and product identifiers from URLs
  - `fallback.rs`: Graceful degradation when APIs unavailable
  - `product.rs`: Traits and common types (PriceFetcher, ProductFetcher)
- Depends on: ureq for HTTP, external retailer APIs
- Used by: `cli/item.rs` (URL-based add), `cli/price.rs` (fetch, refresh)

## Data Flow

**Adding an Item (URL-based):**

1. User runs `sp item add --url="https://bestbuy.com/site/..."`
2. `cli/item.rs::AddArgs` parses URL and initializes AddHandler
3. Handler calls `pricing/url_parser.rs::parse_url()` to extract retailer and identifiers
4. Handler calls `pricing::fetch_product(query)` to attempt to get specs and price
5. If API available: `pricing/bestbuy.rs` or `pricing/ebay.rs` fetches ProductInfo (specs, price)
6. If API unavailable: `pricing/fallback.rs` generates agent response with instructions
7. Handler creates `Item` model with parsed specs as JSON
8. Handler creates `Price` observation (if available) with source attribution
9. Both inserted via `Database::transaction()` → execute INSERT statements
10. Events recorded: `EventType::Created` for Item and `EventType::PriceObserved` for Price
11. Results displayed to user (or structured JSON for agent consumption)

**Recording a Price Observation:**

1. User runs `sp price add <item-id> --price=289 --condition=new --source=amazon`
2. `cli/price.rs::AddArgs` validates item exists
3. Handler creates `Price::new()` model with provided values
4. Transaction: Insert price, record PriceObserved event
5. Price history preserved (append-only, never overwritten)

**Building a Configuration:**

1. User runs `sp config create "My Setup" --domain=storage`
2. `cli/config.rs::CreateArgs` generates ID and creates `Configuration` struct
3. User runs `sp config add-item "My Setup" <item-id> --qty=2 --unit-price=289`
4. Handler loads configuration, loads item, appends `ConfigItem` to `items` vec
5. Transaction: Update configuration JSON field, record Updated event
6. Configuration can include domain_data (JSON) for storage-specific metadata

**Making a Decision:**

1. User runs `sp decide create --purpose="Storage upgrade"`
2. `cli/decide.rs` creates `Decision` with Active status
3. User runs `sp decide add-option opt1 --config="Config A"`
4. Handler maps option name to config ID in Decision.options HashMap
5. User runs `sp decide compare` - compares configurations (specs, cost, availability)
6. User runs `sp decide choose opt1 --rationale="Best value"`
7. Handler updates Decision: chosen_option, chosen_config_id, rationale, decided_at, status → Decided
8. Transaction: Insert Decision, record DecisionMade event
9. Optional: `sp decide deploy` sets Configuration.is_current=true, records ConfigDeployed event

**State Management:**

- Current state: Queried from database on each command (`Database::conn()`)
- Transactions: All mutations wrapped in `Database::transaction()` - auto-rollback on error
- Audit trail: Every mutation creates an Event (immutable, append-only)
- Exports: `sp sync` exports database to YAML in `export/` directory (read-only snapshots for review)
- No in-memory caching - database is the source of truth

## Key Abstractions

**Item:**
- Purpose: Purchasable product with specs and metadata
- Examples: `src/core/models.rs::Item` (struct with id, name, category, specs)
- Pattern: JSON specs field allows domain-specific attributes without schema changes
- Lifecycle: Created → Updated (metadata) → optionally Archived (soft delete)

**Price:**
- Purpose: Price observation at a point in time (immutable record)
- Examples: `src/core/models.rs::Price` (struct with item_id, price, source, condition, observed_at)
- Pattern: Append-only - never update, always insert new observation
- Sources: Enum + Custom (extensible without code changes)

**Configuration:**
- Purpose: Named composition of items forming a system
- Examples: `src/core/models.rs::Configuration` (struct with items vec, domain_data JSON)
- Pattern: Contains ConfigItem references (not full items) for lightweight composition
- Cost calculation: `Configuration::total_cost()` sums item prices * quantities

**Decision:**
- Purpose: Decision session with multiple options and chosen outcome
- Examples: `src/core/models.rs::Decision` (struct with options HashMap, rationale, status)
- Pattern: Options map name → config_id, allows side-by-side comparison
- Lifecycle: Active → Decided (with chosen option, rationale, timestamp)

**Event:**
- Purpose: Immutable audit log entry
- Examples: `src/core/models.rs::Event` (EventType + EntityType + payload)
- Pattern: JSON payload contains mutation details, searchable by entity_type and entity_id
- Invariant: No deletes, no updates - enables full replay/reconstruction

**Volume (Storage Domain):**
- Purpose: Storage unit attached to a node
- Examples: `src/domains/storage/models.rs::Volume` (struct with capacity_bytes, raid_level, datasets)
- Pattern: References item_id to link to catalog Item, embeds in Configuration.domain_data
- Use case: Building multi-volume storage topologies

## Entry Points

**CLI Entry:**
- Location: `src/main.rs`
- Triggers: User runs `sp <command> <subcommand> [args]`
- Responsibilities:
  1. Parse command line args via `Cli::parse()` (clap)
  2. Dispatch to appropriate command enum variant
  3. Load or initialize database at `.sp/decisions.db` (or `$SP_DIR`)
  4. Execute command logic, handle errors

**Database Entry:**
- Location: `src/core/db.rs::Database::open()` or `Database::open_memory()` (tests)
- Triggers: CLI initializes at startup (`sp init` or automatic on first command)
- Responsibilities:
  1. Create/open SQLite file at given path
  2. Enable WAL mode (better concurrency), foreign keys, synchronous mode
  3. Run migrations (schema initialization)
  4. Provide transaction interface for CLI commands

**Pricing Entry:**
- Location: `src/pricing/mod.rs::fetch_product()` or `parse_url()`
- Triggers: `sp item add --url=<url>` or `sp price fetch`
- Responsibilities:
  1. Parse URL to extract retailer and product identifiers
  2. Try available APIs in order (Best Buy → eBay)
  3. Return ProductInfo (specs, price) or None if unavailable
  4. Return structured response for agent fallback workflows

## Error Handling

**Strategy:** Layered error propagation with anyhow::Result

**Patterns:**

- **CLI Input Validation:**
  - `clap` validates required args and types at parse time
  - Commands manually validate business logic: "does item exist?", "is config empty?", etc.
  - Return `anyhow::bail!("reason")` for user-facing errors

- **Database Errors:**
  - Wrap rusqlite errors with context via `.with_context()` for readability
  - Transactions auto-rollback on any error - ensures atomicity
  - Schema validation at `Database::migrate()` time

- **API Errors:**
  - `PriceFetcher` returns `Result<Vec<PriceResult>>` - fails gracefully
  - No panic on missing API keys - treated as "not available"
  - Fallback handlers generate structured messages for agents to retry manually

- **Catastrophic Errors:**
  - Database corruption: Error from `Database::open()` propagates to main
  - Missing `.sp/` directory: Auto-created by `Database::open()`
  - Schema mismatch: Caught by `Database::is_initialized()` checks

## Cross-Cutting Concerns

**Logging:**
- Approach: Console output via `console::style()` crate
- No structured logging framework yet - simple stdout/stderr
- Errors formatted with context via anyhow

**Validation:**
- CLI layer: clap validates types and required fields
- Business logic: Commands verify invariants (e.g., item exists before adding price)
- Database: Foreign key constraints enforced via SQLite PRAGMA

**Authentication:**
- No user authentication (local CLI tool)
- Actor tracking: `current_actor()` reads `$USER` or `$USERNAME` env var
- Events record actor for audit trail (who made changes, when)

**Timestamps:**
- All timestamps in UTC via `chrono::Utc::now()`
- Stored as RFC3339 strings in SQLite for compatibility
- Parsed back to DateTime<Utc> when loaded from database

---

*Architecture analysis: 2026-01-31*

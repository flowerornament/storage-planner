# Codebase Concerns

**Analysis Date:** 2026-01-31

## Tech Debt

**Widespread unwrap() usage in serialization:**
- Issue: Multiple JSON serialization calls use `.unwrap()` without error handling, creating panic points in production
- Files: `src/core/models.rs` (lines 57-59, 81-82, 144, 379, 399, 519, 526), `src/cli/price.rs` (lines 886-887), `src/cli/analyze.rs` (line 294), `src/pricing/fallback.rs` (line 231)
- Impact: Any JSON serialization failure (e.g., non-serializable data in fields) causes CLI crash instead of graceful error handling
- Fix approach: Replace `.unwrap()` with proper error propagation using `?` or `.context()`. For deserialization, use `unwrap_or_default()` only for user-recoverable data; convert system failures to Result

**Incomplete eBay API integration:**
- Issue: eBay's `fetch_by_id()` method at `src/pricing/ebay.rs:257` uses wrong API endpoint format (line 266: `v1|{}|0` pattern appears incorrect)
- Files: `src/pricing/ebay.rs` (lines 262-268)
- Impact: Item lookups by ID fail silently or return wrong data. Comments acknowledge this is a simplification that needs proper endpoint
- Fix approach: Implement proper `/buy/browse/v1/item/{item_id}` endpoint. Add integration tests with actual eBay responses

**Custom base64 encoding instead of standard library:**
- Issue: Manual base64 encoding implementation in `src/pricing/ebay.rs:300-330` duplicates standard library functionality
- Files: `src/pricing/ebay.rs` (lines 300-330)
- Impact: Code maintenance burden, potential encoding bugs, security review overhead. Standard library is battle-tested
- Fix approach: Use `base64` crate (or standard library if added). Remove custom implementation

**Hacky JSON serialization in price comparison:**
- Issue: Comment explicitly notes "a bit hacky" approach at `src/cli/price.rs:884-887` - serializes to JSON then back to access fields
- Files: `src/cli/price.rs` (lines 884-887)
- Impact: Performance overhead (unnecessary serialization), type-safety lost, makes refactoring error-prone
- Fix approach: Define proper ItemPrice struct, derive Display traits, or use serde_json::Value::pointer() for field access

**Fallback field access with .as_ref().unwrap():**
- Issue: Best Buy fetcher uses `as_deref().unwrap_or("")` to provide empty default for API key instead of failing early
- Files: `src/pricing/bestbuy.rs` (lines 36, 46, 55)
- Impact: Silently produces invalid API URLs when key missing; errors appear downstream in HTTP call instead of at configuration time
- Fix approach: Make API key optional in constructor, fail in `is_available()` checks. Let API request fail with clear "missing API key" message

## Known Bugs

**Partial floating-point comparison without guards:**
- Symptoms: Code at `src/cli/analyze.rs:294` uses `.partial_cmp().unwrap()` on floats without checking for NaN
- Files: `src/cli/analyze.rs` (line 294)
- Trigger: Any NaN value in noise_levels causes panic when finding max
- Workaround: Ensure all noise data is validated before reaching this code; preprocess with `.filter(!f64::is_nan)`
- Fix: Use `max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))` or comparable fallback for NaN handling

**Silent JSON deserialization failures:**
- Symptoms: `from_row()` methods use `unwrap_or_default()` for JSON fields, losing data corruption indicators
- Files: `src/core/models.rs` (lines 81-82, 399), `src/pricing/ebay.rs` (line 201)
- Trigger: Corrupted metadata in database → silently becomes empty object instead of error
- Workaround: Validate database before deserializing; spot-check metadata in exports
- Impact: Data loss hidden from users; audit trail becomes unreliable

## Security Considerations

**API credentials in environment variables without validation:**
- Risk: Missing API keys silently produce empty strings used in URLs, or endpoints get called without auth headers
- Files: `src/pricing/bestbuy.rs` (lines 23-26, 36, 46, 55), `src/pricing/ebay.rs` (lines 28-33, 63-71)
- Current mitigation: `is_available()` checks before usage, but checks use same pattern as calls
- Recommendations:
  1. Validate API keys are non-empty at load time, fail startup if required keys missing
  2. Log authentication failures (rate limiting signals from API)
  3. Add request signing verification where APIs support it
  4. Test with intentionally bad credentials to verify error messages don't leak secrets

**No HTTPS enforcement for API calls:**
- Risk: Pricing API requests use HTTP URLs that could be intercepted or modified
- Files: `src/pricing/bestbuy.rs` (line 15: hardcoded HTTP), `src/pricing/ebay.rs` (lines 17-18: HTTPS), `src/pricing/url_parser.rs` (external URLs)
- Current mitigation: eBay uses HTTPS; Best Buy API endpoint is HTTPS in practice
- Recommendations: Add compile-time assertion that API URLs are HTTPS; test URL parsing for HTTP→HTTPS redirects

**No rate limiting or backoff strategy:**
- Risk: Repeated API calls could trigger rate limiting bans without client-side protection
- Files: `src/pricing/bestbuy.rs`, `src/pricing/ebay.rs` - no retry logic, no backoff
- Current mitigation: Price refresh is user-initiated, not automatic
- Recommendations: Add exponential backoff to `make_request()` methods, cache tokens for 60 minutes minimum

## Performance Bottlenecks

**Database queries without indexes:**
- Problem: No evidence of query optimization or index creation in `src/core/db.rs`
- Files: `src/core/db.rs` (schema migrations in SCHEMA constant not shown)
- Cause: SQLite schema may lack indexes on frequently queried fields (item_id, entity_id, entity_type)
- Improvement path: Add indexes on: items(category), prices(item_id, observed_at), events(entity_type, entity_id)

**Full JSON serialization/deserialization on every row load:**
- Problem: Every Item/Price/Event loaded from database deserializes JSON fields even when not needed
- Files: `src/core/models.rs` (from_row methods), affects all list/search operations
- Cause: Eager deserialization in From/row implementations
- Improvement path: Store parsed JSON in memory cache, lazy-deserialize on field access, or keep as String for display-only queries

**No query result limits in list operations:**
- Problem: `sp item list` could load entire database into memory
- Files: `src/cli/item.rs` - likely missing LIMIT clauses
- Cause: No pagination or result limiting visible in command definitions
- Improvement path: Add `--limit` and `--offset` parameters to list/search commands; implement cursor-based pagination

## Fragile Areas

**URL parsing for retailer identification:**
- Files: `src/pricing/url_parser.rs` (lines 44-185)
- Why fragile: Regex-free string matching brittle to URL format changes; no standardization of what retailers use
- Safe modification: Add comprehensive test cases for each retailer's URL formats (query params, alternate domains, ASIN-only formats); document expected patterns
- Test coverage: Unit tests exist but don't cover edge cases like:
  - URLs with extra path segments
  - Subdomains (amazon.co.uk, amazon.fr)
  - Mobile URLs (m.bestbuy.com)
  - URL shorteners

**Price extraction logic in CLI parsing:**
- Files: `src/cli/price.rs` (lines 884-918)
- Why fragile: Manual JSON traversal assumes consistent ItemPrice structure; no schema validation
- Safe modification: Define explicit struct, use serde for parsing, add field presence checks
- Test coverage: No visible tests for price formatting edge cases (missing prices, extreme values, currency mismatches)

**Event payload serialization:**
- Files: `src/core/models.rs` (line 379)
- Why fragile: `.unwrap()` on line 379 assumes payload always serializable; no validation of payload structure
- Safe modification: Add payload validation before insert; consider Schema validation framework (e.g., jsonschema)
- Test coverage: Events tested in `src/core/events.rs` but only with simple payloads

## Scaling Limits

**Single SQLite database file:**
- Current capacity: SQLite scales to ~100GB files, but CLI becomes slow with >1M rows without indexes
- Limit: Performance degrades significantly past ~500k price observations without proper indexing
- Scaling path: Add read replicas for analysis queries using `.sp/decisions.db-wal` WAL mode already enabled; implement archival to separate DB files by year

**No pagination in list/search operations:**
- Current capacity: Memory limits command output; 10k items × 1MB each = 10GB RAM
- Limit: UI becomes unusable with >1000 items in result set
- Scaling path: Implement cursor-based pagination; lazy-load price history on demand

**Token cache stored locally:**
- Current capacity: Single token cache file at `.sp/token.json` shared across all processes
- Limit: Concurrent CLI invocations could corrupt token cache
- Scaling path: Use file locking or atomic writes for token cache; consider moving to OS keychain

## Dependencies at Risk

**ureq without timeout configuration:**
- Risk: Hanging HTTP requests on network issues could freeze CLI
- Files: `src/pricing/bestbuy.rs` (line 64), `src/pricing/ebay.rs` (lines 77, 244, 268)
- Impact: CLI hangs indefinitely if retailer API slow or unreachable
- Migration plan: Set explicit timeouts on ureq calls (e.g., `.timeout(Duration::from_secs(30))`); add `--timeout` CLI parameter

**No vendoring of dependencies:**
- Risk: crates.io outage breaks builds; malicious version published
- Current: Cargo.lock present, so reproducible
- Recommendations: Use git dependencies for internal crates; evaluate cargo-vendor for offline builds

**chrono for date handling:**
- Risk: Potential future maintenance burden if chrono development slows
- Current: Good library, well-maintained
- Recommendations: Monitor for time-rs (time crate) alternatives; document date format expectations (RFC3339)

## Missing Critical Features

**No export/import for decisions:**
- Problem: Decisions created in CLI cannot be exported to portable format; backup/sharing requires raw DB file
- Blocks: Cannot integrate with external decision tracking systems; no portable config snapshots
- Approach: Add `sp decide export --format=json|yaml --decision=id` and import counterpart

**No comparison across configs:**
- Problem: Can build configs but cannot rank them systematically
- Blocks: Making final purchase decision requires manual review; no scoring/weighing system
- Approach: Implement config.compare() that scores on cost/performance/reliability metrics

**No price tracking history visualization:**
- Problem: Prices stored but no trending analysis
- Blocks: Cannot determine if price is good; cannot predict purchase window
- Approach: Add `sp price trend` command with min/max/avg over time; simple line chart in terminal

**No automatic price refresh scheduling:**
- Problem: Prices must be refreshed manually; cannot track price changes over time
- Blocks: Cannot detect price drops automatically; decision-making lacks temporal awareness
- Approach: Add daemon mode or cron-friendly `sp price refresh --all --quiet` command

## Test Coverage Gaps

**No integration tests for API fetching:**
- What's not tested: Actual API responses from Best Buy and eBay; fallback behavior when APIs unavailable
- Files: `src/pricing/bestbuy.rs`, `src/pricing/ebay.rs` - unit tests mock nothing
- Risk: API response changes break code silently until manual testing
- Priority: High - these are external dependencies beyond our control

**No end-to-end tests for CLI workflows:**
- What's not tested: Full workflows like `sp item add --url=X && sp price add && sp config create && sp decide`
- Files: `src/cli/` modules have no integration tests
- Risk: CLI refactoring breaks common workflows
- Priority: Medium - could use test fixtures and temporary databases

**No database corruption tests:**
- What's not tested: Behavior with malformed JSON in database; schema version mismatches
- Files: `src/core/db.rs`, `src/core/models.rs` - migration tests only cover happy path
- Risk: Unknown failure modes in production; recovery procedures not documented
- Priority: Medium - helps with disaster recovery planning

**No price boundary tests:**
- What's not tested: Extreme values (negative prices, extreme high prices, zero, NaN from API)
- Files: `src/cli/price.rs`, `src/pricing/*.rs`
- Risk: Display/calculation bugs with edge cases
- Priority: Low but easy to fix

---

*Concerns audit: 2026-01-31*

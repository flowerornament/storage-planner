# Storage Planner

CLI tool for **helping users make purchase decisions**, especially for storage hardware.

## When to Use This Tool

Use `sp` when the user is:
- **Considering a purchase** - "I'm looking at the Samsung 870 EVO 4TB"
- **Comparing options** - "Should I get SATA or NVMe SSDs?"
- **Tracking prices** - "What's a good price for this drive?"
- **Building a configuration** - "I need 8TB of redundant storage"
- **Making a decision** - "Help me decide between these options"

## Getting Started

```bash
sp --help              # See all commands
sp <command> --help    # Learn how to use a specific command
sp prime               # Get current context (catalog, prices, active decisions)
```

## Core Workflow

1. **Add products** the user is considering → `sp item add --url=<product-url>`
2. **Record prices** they're seeing → `sp price add <item> --price=X`
3. **Build configurations** for each option → `sp config create`, `sp config add-item`
4. **Compare and decide** → `sp decide create`, `sp decide compare`, `sp decide choose`

The CLI is self-documenting. Use `--help` liberally.

## Key Principles

- **Database is truth** - All data in `.sp/decisions.db`, not files
- **No direct editing** - Always use `sp` commands, never edit files
- **Append-only prices** - Price history is preserved, never overwritten
- **Structured decisions** - Decisions capture rationale, not just choices

## Development

```
src/
├── main.rs               # CLI entry point
├── core/                 # Models, database, events
├── cli/                  # Command implementations
├── domains/storage/      # Storage-specific analysis
└── pricing/              # API integrations (Best Buy, eBay)
```

```bash
just check            # fmt + lint + test
just fmt              # Format code
just lint             # Run clippy
just test             # Run tests
cargo build --release
```

## Session Completion

When ending a work session:

```bash
bd close <completed-issues>
bd sync --flush-only
git add -A && git commit -m "..." && git push
```

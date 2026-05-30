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

1. **Add products** to the catalog → `sp catalog add <name> --category=ssd`
2. **Record prices** → `sp catalog price add <item> --amount=X`
3. **Build topologies** → `sp topology create <name>`, `sp node add`, `sp volume add`
4. **Compare and decide** → `sp decision create`, `sp analyze compare`, `sp decision choose`

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
└── domains/storage/      # Storage-specific analysis
```

```bash
just check            # fmt + lint + test
just fmt              # Format code
just lint             # Run clippy
just test             # Run tests
cargo build --release
```

## Nix Packaging

Built via `flake.nix` (`rustPlatform.buildRustPackage`). Installed system-wide from `~/.nix-config/flake.nix`.

- Dev loop: `cargo run` / `just check` — no nix rebuild needed
- Verify packaging: `nix build .` then `./result/bin/sp --help`
- Deploy: push to GitHub, then in nix-config: `nix flake update storage-planner && nx rebuild`

## Release Flow

Release automation is local-first and tag-driven. Day-to-day work lands on `master`; release commits are ordinary commits on `master`; downstream flake consumers that want the latest published release should track `refs/heads/release`.

The `release` branch is generated state. `just release-tag` moves it to the new annotated version tag with `--force-with-lease` after the tag push.

Before bumping, verify shipped behavior is reflected in the docs agents and users read:

- `CHANGELOG.md` — entry for the target version, scaffolded by `release-bump`
- `README.md` — install instructions, command examples, and user-facing behavior
- `CLAUDE.md` / `AGENTS.md` — release/process changes, when agent workflow changed

Write docs as if they were always correct, without "added" or "updated" language.

Canonical sequence:

```bash
just release-bump 0.2.1
# Fill CHANGELOG.md and update docs for shipped user-facing behavior.
git add Cargo.toml Cargo.lock flake.nix CHANGELOG.md README.md CLAUDE.md
git commit -m "Release v0.2.1"
just release-verify
git push origin master
just release-tag 0.2.1
git ls-remote origin refs/heads/release 'refs/tags/v0.2.1^{}'
```

`just release-verify` intentionally requires a clean worktree. Commit the release-prep changes before running it so the Nix build sees the same git-tracked source that will be tagged.

`just release-verify` checks version alignment across `Cargo.toml`, `Cargo.lock`, and `flake.nix`; CHANGELOG readiness with no `TODO`/`TBD` placeholders; then runs `just check`, `just build`, `nix build .`, `nix run . -- --help`, and `./target/release/sp --help`.

`just release-tag` creates and pushes `vX.Y.Z`, then publishes `origin/release` at the same commit. It prompts before running because this is the public release step; use `just --yes release-tag X.Y.Z` only for explicit automation. The final `git ls-remote` check should show matching object IDs for `refs/heads/release` and the peeled tag.

## Task Tracking (bd)

```bash
# orient
bd show --current --short
bd query "status=in_progress"
bd ready --explain

# work
bd update <id> --claim
bd note <id> "context"
bd close <id> --suggest-next

# capture
bd todo add "quick thought"
bd create --title="..." --type=task --priority=2

# query
bd query "type=bug AND priority<=1 AND updated>7d"
bd search "keyword"
bd count "status=open"
bd graph --compact <id>

# state
bd kv set/get key [value]
bd find-duplicates
```

Full ref: `bd prime`

## Completion

Before ending a session:
1. Run `just check` if code changed.
2. Commit with a clear message.
3. `bd dolt push && git push`

Work is not complete until `git push` succeeds.

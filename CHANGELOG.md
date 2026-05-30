# Changelog

All notable changes to `storage-planner` are documented in this file.

## v1.0.1 - 2026-05-29

### Added

- Local-first, tag-driven release flow: `scripts/release.py` (bump, verify, tag)
  driven through grouped `just` recipes, a scaffolded `CHANGELOG.md`, and a
  `release` branch that downstream Nix consumers track via
  `?ref=refs/heads/release`.
- Nix flake for reproducible builds: `nix build .` produces `result/bin/sp` and
  `nix run . -- --help` runs the CLI without installing.
- Export/import round-trip test coverage for topology YAML.

### Changed

- Errors propagate to the caller instead of being silently discarded, including
  timestamp handling that previously fell back without surfacing failures.
- Consolidated duplicated loaders and bandwidth formatting, normalized the
  `analyze` command onto the standard subcommand pattern, and resolved a layer
  violation between modules.

### Fixed

- `compare` and constraint budget checks use catalog prices.


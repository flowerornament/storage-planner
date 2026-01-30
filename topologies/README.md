# Topologies Directory

This directory contains dated topology configurations for ongoing storage/backup analysis.

## Naming Convention

```
YYYY-MM-DD-<name>[-<variant>].yaml
```

Examples:
- `2025-01-29-mac-mini-m4-hub-option-a.yaml` - Initial Mac mini M4 hub plan, Option A
- `2025-01-29-mac-mini-m4-hub-option-b.yaml` - Same plan, Option B variant
- `2025-02-15-mac-mini-m4-hub-final.yaml` - Post-purchase actual configuration

## Workflow

1. **Planning phase**: Create dated files for different options being considered
2. **Decision phase**: Keep the chosen option, optionally archive rejected options
3. **Implementation phase**: Update the file as you purchase/configure hardware
4. **Ongoing**: Create new dated files when making significant changes

## Current Analysis

| Date | File | Description |
|------|------|-------------|
| 2025-01-29 | mac-mini-m4-hub-option-a.yaml | OWC ThunderBay 4 mini + 2x Crucial MX500 4TB RAID1 |

## Archive

Older/superseded topologies can be moved to `archive/` subdirectory if needed.

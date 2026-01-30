"""Path resolution helpers for CLI commands."""

from pathlib import Path

import typer


def resolve_config_path(config: Path | None) -> Path:
    """Resolve config path, defaulting to topology.yaml in current dir."""
    if config:
        return config
    default = Path("topology.yaml")
    if default.exists():
        return default
    raise typer.BadParameter(
        "No config file specified and topology.yaml not found in current directory"
    )


def resolve_catalog_path(catalog_dir: Path | None) -> Path:
    """Resolve catalog directory path."""
    if catalog_dir:
        return catalog_dir
    candidates = [
        Path("catalog"),
        Path.home() / ".config" / "storage-planner" / "catalog",
    ]
    for candidate in candidates:
        if candidate.exists():
            return candidate
    return Path("catalog")

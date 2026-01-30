"""Validate command for storage planner."""

from pathlib import Path
from typing import Optional

import typer

from storage_planner.loaders import load_topology, validate_topology_references, ValidationError
from storage_planner.analysis.completeness import (
    validate_completeness,
    format_completeness_report,
    IssueSeverity,
)
from storage_planner.output import console, print_error, print_success, print_warning, print_json
from storage_planner.cli.paths import resolve_config_path


def validate_cmd(
    config: Optional[Path] = typer.Argument(
        None,
        help="Path to topology YAML file (defaults to ./topology.yaml)",
    ),
    verbose: bool = typer.Option(False, "--verbose", "-v", help="Show detailed output"),
    strict: bool = typer.Option(
        False,
        "--strict",
        "-s",
        help="Check for complete explicit configuration (no implicit assumptions)",
    ),
    json_output: bool = typer.Option(False, "--json", help="Output JSON"),
) -> None:
    """Validate a topology configuration file.

    Checks YAML syntax, schema validity, and referential integrity.

    Use --strict to also check that all configuration is explicit and
    no default values or assumptions are being used.
    """
    try:
        # Load and validate schema
        config_path = resolve_config_path(config)
        topology = load_topology(config_path)
        if verbose and not json_output:
            console.print(f"[dim]Loaded topology: {topology.name}[/dim]")

        # Validate references
        ref_errors = validate_topology_references(topology)

        if ref_errors:
            if not json_output:
                print_warning(f"Found {len(ref_errors)} referential integrity issue(s):")
                for err in ref_errors:
                    console.print(f"  [yellow]•[/yellow] {err}")
            if json_output:
                print_json(
                    {
                        "valid": False,
                        "errors": ref_errors,
                        "completeness": None,
                    }
                )
            raise typer.Exit(1)

        # Completeness validation (strict mode or always show if errors)
        completeness_report = validate_completeness(topology)

        if json_output and completeness_report.has_errors:
            print_json(
                {
                    "valid": False,
                    "path": config_path,
                    "errors": [],
                    "completeness": {
                        "is_complete": False,
                        "issues": [
                            {
                                "severity": i.severity.value,
                                "location": i.location,
                                "field": i.field,
                                "message": i.message,
                                "suggestion": i.suggestion,
                            }
                            for i in completeness_report.issues
                        ],
                    },
                }
            )
            raise typer.Exit(1)

        if (strict or completeness_report.has_errors) and not json_output:
            if not completeness_report.is_complete:
                console.print()
                console.print(format_completeness_report(completeness_report))
                console.print()

                if completeness_report.has_errors:
                    print_error(
                        f"Topology has {len([i for i in completeness_report.issues if i.severity == IssueSeverity.ERROR])} "
                        "missing required configuration(s)"
                    )
                    raise typer.Exit(1)
                elif strict:
                    print_warning(
                        f"Topology has {len(completeness_report.issues)} implicit assumption(s) "
                        "(use explicit values for reproducible analysis)"
                    )

        if json_output:
            total_volumes = sum(len(n.volumes) for n in topology.nodes)
            print_json(
                {
                    "valid": True,
                    "path": config_path,
                    "counts": {
                        "nodes": len(topology.nodes),
                        "links": len(topology.links),
                        "datasets": len(topology.datasets),
                        "sync_regimes": len(topology.sync_regimes),
                        "volumes": total_volumes,
                    },
                    "completeness": {
                        "is_complete": completeness_report.is_complete,
                        "issues": [
                            {
                                "severity": i.severity.value,
                                "location": i.location,
                                "field": i.field,
                                "message": i.message,
                                "suggestion": i.suggestion,
                            }
                            for i in completeness_report.issues
                        ],
                    },
                }
            )
            return

        # Summary
        print_success(f"Valid: {config_path}")
        console.print(f"  Nodes: {len(topology.nodes)}")
        console.print(f"  Links: {len(topology.links)}")
        console.print(f"  Datasets: {len(topology.datasets)}")
        console.print(f"  Sync regimes: {len(topology.sync_regimes)}")
        total_volumes = sum(len(n.volumes) for n in topology.nodes)
        console.print(f"  Total volumes: {total_volumes}")

        if strict and completeness_report.is_complete:
            console.print(f"  [green]✓ All configuration is explicit[/green]")

    except ValidationError as e:
        print_error(e.message)
        for err in e.errors:
            console.print(f"  [red]•[/red] {err}")
        if json_output:
            print_json({"valid": False, "errors": [e.message] + e.errors})
        raise typer.Exit(1)

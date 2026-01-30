"""Validate command for storage planner."""

from pathlib import Path

import typer

from storage_planner.loaders import load_topology, validate_topology_references, ValidationError
from storage_planner.analysis.completeness import (
    validate_completeness,
    format_completeness_report,
    IssueSeverity,
)
from storage_planner.output import console, print_error, print_success, print_warning


def validate_cmd(
    config: Path = typer.Argument(
        ...,
        help="Path to topology YAML file",
        exists=True,
        readable=True,
    ),
    verbose: bool = typer.Option(False, "--verbose", "-v", help="Show detailed output"),
    strict: bool = typer.Option(
        False,
        "--strict",
        "-s",
        help="Check for complete explicit configuration (no implicit assumptions)",
    ),
) -> None:
    """Validate a topology configuration file.

    Checks YAML syntax, schema validity, and referential integrity.

    Use --strict to also check that all configuration is explicit and
    no default values or assumptions are being used.
    """
    try:
        # Load and validate schema
        topology = load_topology(config)
        if verbose:
            console.print(f"[dim]Loaded topology: {topology.name}[/dim]")

        # Validate references
        ref_errors = validate_topology_references(topology)

        if ref_errors:
            print_warning(f"Found {len(ref_errors)} referential integrity issue(s):")
            for err in ref_errors:
                console.print(f"  [yellow]•[/yellow] {err}")
            raise typer.Exit(1)

        # Completeness validation (strict mode or always show if errors)
        completeness_report = validate_completeness(topology)

        if strict or completeness_report.has_errors:
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

        # Summary
        print_success(f"Valid: {config}")
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
        raise typer.Exit(1)

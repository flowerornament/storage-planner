"""Analyze commands for storage planner."""

from pathlib import Path
from typing import Optional

import typer
from rich.table import Table

from storage_planner.loaders import load_topology, load_all_catalogs, ValidationError
from storage_planner.output import console, print_error, print_warning, print_success
from storage_planner.analysis import redundancy, bandwidth, rpo_rto, capacity

app = typer.Typer(no_args_is_help=True)


@app.command("all")
def analyze_all(
    config: Path = typer.Argument(
        ...,
        help="Path to topology YAML file",
        exists=True,
        readable=True,
    ),
    catalog_dir: Optional[Path] = typer.Option(
        None, "--catalog", "-c", help="Path to catalog directory"
    ),
) -> None:
    """Run full analysis on a topology (redundancy, RPO/RTO, bandwidth, capacity)."""
    try:
        topology = load_topology(config)
        console.print(f"[bold]Analysis: {topology.name}[/bold]\n")

        # Run all analyses
        _run_redundancy_analysis(topology)
        console.print()
        _run_rpo_rto_analysis(topology)
        console.print()
        _run_bandwidth_analysis(topology)
        console.print()
        _run_capacity_analysis(topology)

    except ValidationError as e:
        print_error(e.message)
        raise typer.Exit(1)


@app.command("redundancy")
def analyze_redundancy(
    config: Path = typer.Argument(
        ...,
        help="Path to topology YAML file",
        exists=True,
        readable=True,
    ),
) -> None:
    """Check data redundancy against requirements."""
    try:
        topology = load_topology(config)
        _run_redundancy_analysis(topology)
    except ValidationError as e:
        print_error(e.message)
        raise typer.Exit(1)


@app.command("bandwidth")
def analyze_bandwidth_cmd(
    config: Path = typer.Argument(
        ...,
        help="Path to topology YAML file",
        exists=True,
        readable=True,
    ),
) -> None:
    """Analyze bandwidth and identify bottlenecks."""
    try:
        topology = load_topology(config)
        _run_bandwidth_analysis(topology)
    except ValidationError as e:
        print_error(e.message)
        raise typer.Exit(1)


@app.command("rpo-rto")
def analyze_rpo_rto_cmd(
    config: Path = typer.Argument(
        ...,
        help="Path to topology YAML file",
        exists=True,
        readable=True,
    ),
) -> None:
    """Check RPO/RTO compliance for datasets."""
    try:
        topology = load_topology(config)
        _run_rpo_rto_analysis(topology)
    except ValidationError as e:
        print_error(e.message)
        raise typer.Exit(1)


@app.command("capacity")
def analyze_capacity_cmd(
    config: Path = typer.Argument(
        ...,
        help="Path to topology YAML file",
        exists=True,
        readable=True,
    ),
    months: int = typer.Option(12, "--months", "-m", help="Projection period in months"),
) -> None:
    """Project capacity needs over time."""
    try:
        topology = load_topology(config)
        _run_capacity_analysis(topology, months)
    except ValidationError as e:
        print_error(e.message)
        raise typer.Exit(1)


def _run_redundancy_analysis(topology) -> None:
    """Run redundancy analysis and print results."""
    results = redundancy.analyze_redundancy(topology)

    table = Table(title="Redundancy Analysis")
    table.add_column("Dataset")
    table.add_column("Criticality")
    table.add_column("Copies")
    table.add_column("Required")
    table.add_column("Locations")
    table.add_column("Req Loc")
    table.add_column("Status")

    for r in results:
        copies_str = str(r.actual_copies)
        if r.actual_copies < r.required_copies:
            copies_str = f"[red]{r.actual_copies}[/red]"

        locs_str = str(r.actual_locations)
        if r.actual_locations < r.required_locations:
            locs_str = f"[red]{r.actual_locations}[/red]"

        status = "[green]OK[/green]" if r.meets_requirements else "[red]FAIL[/red]"

        table.add_row(
            r.dataset_id,
            r.criticality.value,
            copies_str,
            str(r.required_copies),
            locs_str,
            str(r.required_locations),
            status,
        )

    console.print(table)

    # Summary
    failing = [r for r in results if not r.meets_requirements]
    if failing:
        print_warning(f"{len(failing)} dataset(s) do not meet redundancy requirements")
    else:
        print_success("All datasets meet redundancy requirements")


def _run_rpo_rto_analysis(topology) -> None:
    """Run RPO/RTO analysis and print results."""
    results = rpo_rto.analyze_rpo_rto(topology)

    table = Table(title="RPO/RTO Analysis")
    table.add_column("Dataset")
    table.add_column("Max RPO")
    table.add_column("Achieved RPO")
    table.add_column("Max RTO")
    table.add_column("Status")

    for r in results:
        achieved = r.achieved_rpo or "[dim]unknown[/dim]"
        if r.rpo_met is False:
            achieved = f"[red]{achieved}[/red]"
        elif r.rpo_met is True:
            achieved = f"[green]{achieved}[/green]"

        max_rpo = r.max_rpo or "[dim]none[/dim]"
        max_rto = r.max_rto or "[dim]none[/dim]"

        if r.rpo_met is None:
            status = "[yellow]?[/yellow]"
        elif r.rpo_met:
            status = "[green]OK[/green]"
        else:
            status = "[red]FAIL[/red]"

        table.add_row(r.dataset_id, max_rpo, achieved, max_rto, status)

    console.print(table)


def _run_bandwidth_analysis(topology) -> None:
    """Run bandwidth analysis and print results."""
    results = bandwidth.analyze_bandwidth(topology)

    if not results:
        console.print("[dim]No sync regimes to analyze[/dim]")
        return

    table = Table(title="Bandwidth Analysis")
    table.add_column("Sync Regime")
    table.add_column("Dataset Size")
    table.add_column("Link")
    table.add_column("Bandwidth")
    table.add_column("Est. Full Sync")
    table.add_column("Notes")

    for r in results:
        notes = ""
        if r.is_bottleneck:
            notes = "[yellow]Potential bottleneck[/yellow]"

        table.add_row(
            r.sync_regime_id,
            r.dataset_size,
            r.link_id or "[dim]unknown[/dim]",
            r.effective_bandwidth or "[dim]unknown[/dim]",
            r.estimated_sync_time or "[dim]unknown[/dim]",
            notes,
        )

    console.print(table)


def _run_capacity_analysis(topology, months: int = 12) -> None:
    """Run capacity analysis and print results."""
    results = capacity.analyze_capacity(topology, months)

    table = Table(title=f"Capacity Projection ({months} months)")
    table.add_column("Volume")
    table.add_column("Current Used")
    table.add_column("Capacity")
    table.add_column("Projected Used")
    table.add_column("Utilization")
    table.add_column("Status")

    for r in results:
        util_str = f"{r.projected_utilization_pct:.0f}%"
        if r.projected_utilization_pct > 90:
            util_str = f"[red]{util_str}[/red]"
        elif r.projected_utilization_pct > 75:
            util_str = f"[yellow]{util_str}[/yellow]"

        status = "[green]OK[/green]"
        if r.will_exceed_capacity:
            status = "[red]OVER[/red]"
        elif r.projected_utilization_pct > 90:
            status = "[yellow]WARN[/yellow]"

        table.add_row(
            r.volume_id,
            r.current_used,
            r.capacity,
            r.projected_used,
            util_str,
            status,
        )

    console.print(table)

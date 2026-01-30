"""Suggest commands for storage planner."""

from pathlib import Path
from typing import Optional

import typer
from rich.table import Table
from rich.panel import Panel

from storage_planner.loaders import load_topology, load_all_catalogs, ValidationError
from storage_planner.output import console, print_error, print_info
from storage_planner.analysis.redundancy import analyze_redundancy
from storage_planner.models import (
    Topology,
    HardwareCatalog,
    SoftwareCatalog,
    MarketPrices,
    Dataset,
    ChangeRate,
    SyncDirection,
)

app = typer.Typer(no_args_is_help=True)


@app.command("hardware")
def suggest_hardware(
    config: Path = typer.Argument(
        ...,
        help="Path to topology YAML file",
        exists=True,
        readable=True,
    ),
    catalog_dir: Optional[Path] = typer.Option(
        None, "--catalog", "-c", help="Path to catalog directory"
    ),
    budget: Optional[float] = typer.Option(
        None, "--budget", "-b", help="Maximum budget for recommendations"
    ),
) -> None:
    """Suggest hardware based on topology gaps.

    Analyzes the topology to find:
    - Volumes without assigned hardware
    - Redundancy gaps that could be filled with additional storage
    - Upgrade opportunities
    """
    try:
        topology = load_topology(config)

        hardware = HardwareCatalog()
        prices = MarketPrices()
        if catalog_dir and catalog_dir.exists():
            hardware, _, prices = load_all_catalogs(catalog_dir)

        console.print(f"[bold]Hardware Suggestions: {topology.name}[/bold]\n")

        suggestions_made = False

        # Check for volumes without product_id that could benefit from recommendations
        for node in topology.nodes:
            for volume in node.volumes:
                if not volume.product_id and not volume.purchase_cost:
                    suggestions_made = True
                    _suggest_for_volume(volume, node, hardware, prices, budget)

        # Check redundancy gaps
        redundancy_results = analyze_redundancy(topology)
        failing = [r for r in redundancy_results if not r.meets_requirements]

        if failing:
            suggestions_made = True
            console.print("\n[bold]Redundancy Gaps[/bold]")
            for r in failing:
                console.print(
                    f"\n  [yellow]{r.dataset_name}[/yellow]: needs {r.required_copies - r.actual_copies} "
                    f"more copies, {r.required_locations - r.actual_locations} more locations"
                )
                # Suggest storage to add
                _suggest_for_redundancy_gap(r, topology, hardware, prices, budget)

        if not suggestions_made:
            print_info("No hardware gaps identified. Topology looks complete.")

    except ValidationError as e:
        print_error(e.message)
        raise typer.Exit(1)


def _suggest_for_volume(volume, node, hardware, prices, budget):
    """Suggest hardware for a volume."""
    from storage_planner.analysis.utils import parse_size

    console.print(f"\n[bold]Volume: {volume.id}[/bold] on {node.name}")
    console.print(f"  Type: {volume.type.value}")
    console.print(f"  Capacity: {volume.raw_capacity}")

    # Find matching products
    capacity_bytes = parse_size(volume.raw_capacity)
    matching = []

    for product in hardware.products:
        if "capacity" in product.specs:
            prod_capacity = parse_size(product.specs["capacity"])
            if prod_capacity and capacity_bytes:
                # Match products within 50% of target capacity
                if prod_capacity >= capacity_bytes * 0.5 and prod_capacity <= capacity_bytes * 2:
                    price = product.retail_price or 0
                    mp = prices.get_best_price(product.id)
                    used_price = mp.price_mid if mp else None

                    if budget and price > budget and (not used_price or used_price > budget):
                        continue

                    matching.append((product, mp))

    if matching:
        console.print("\n  [bold]Suggested Products:[/bold]")
        for product, mp in matching[:5]:  # Top 5
            price_str = f"${product.retail_price:.0f}" if product.retail_price else "?"
            if mp:
                price_str += f" (used: ${mp.price_mid:.0f})"
            console.print(f"    • {product.name}: {price_str}")
            if product.specs.get("capacity"):
                console.print(f"      Capacity: {product.specs['capacity']}")
    else:
        console.print("  [dim]No matching products in catalog[/dim]")


def _suggest_for_redundancy_gap(result, topology, hardware, prices, budget):
    """Suggest hardware to fill a redundancy gap."""
    console.print("  [dim]Consider adding storage to another location[/dim]")


@app.command("software")
def suggest_software(
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
    """Suggest sync/backup software for each dataset.

    Matches dataset characteristics (change rate, criticality, RPO)
    against software capabilities defined in the catalog.
    """
    try:
        topology = load_topology(config)

        software = SoftwareCatalog()
        if catalog_dir and catalog_dir.exists():
            _, software, _ = load_all_catalogs(catalog_dir)

        if not software.software:
            print_error("No software definitions in catalog. Create catalog/software.yaml")
            raise typer.Exit(1)

        console.print(f"[bold]Software Suggestions: {topology.name}[/bold]\n")

        for dataset in topology.datasets:
            _suggest_software_for_dataset(dataset, topology, software)

    except ValidationError as e:
        print_error(e.message)
        raise typer.Exit(1)


def _suggest_software_for_dataset(dataset: Dataset, topology: Topology, software: SoftwareCatalog):
    """Suggest software for a dataset based on its characteristics."""
    console.print(f"[bold]{dataset.name}[/bold] ({dataset.id})")
    console.print(f"  Criticality: {dataset.criticality.value}")
    console.print(f"  Change rate: {dataset.change_rate.value}")
    if dataset.max_rpo:
        console.print(f"  Max RPO: {dataset.max_rpo}")
    if dataset.data_type:
        console.print(f"  Data type: {dataset.data_type}")

    # Score each software option
    scores: list[tuple[float, any, list[str]]] = []

    for sw in software.software:
        score = 0.0
        reasons: list[str] = []

        # Check change rate match
        if dataset.change_rate in sw.best_for.change_rate:
            score += 2.0
            reasons.append(f"Handles {dataset.change_rate.value} change rate")

        # Check data type match
        if dataset.data_type and dataset.data_type in sw.best_for.data_type:
            score += 2.0
            reasons.append(f"Designed for {dataset.data_type}")

        # Check criticality match
        if dataset.criticality.value in sw.best_for.criticality:
            score += 1.0
            reasons.append(f"Suited for {dataset.criticality.value} data")

        # Bonus for continuous sync with high change rate
        if dataset.change_rate in (ChangeRate.HIGH, ChangeRate.REALTIME):
            if "continuous" in sw.strengths:
                score += 1.5
                reasons.append("Supports continuous sync")

        # Bonus for versioning with critical data
        if dataset.criticality.value == "critical":
            if "versioning" in sw.strengths:
                score += 1.0
                reasons.append("Has versioning for recovery")

        # Bonus for deduplication with large datasets
        if "deduplication" in sw.strengths:
            score += 0.5
            reasons.append("Deduplication saves space")

        if score > 0:
            scores.append((score, sw, reasons))

    # Sort by score descending
    scores.sort(key=lambda x: -x[0])

    if scores:
        console.print("\n  [bold]Recommended:[/bold]")
        top_score, top_sw, top_reasons = scores[0]
        console.print(f"    [green]{top_sw.name}[/green] ({top_sw.type})")
        for reason in top_reasons[:3]:
            console.print(f"      • {reason}")

        if len(scores) > 1:
            _, alt_sw, _ = scores[1]
            console.print(f"\n    Alternative: {alt_sw.name}")
    else:
        console.print("  [dim]No specific recommendation - any general backup tool should work[/dim]")

    console.print()


@app.command("optimize")
def suggest_optimize(
    config: Path = typer.Argument(
        ...,
        help="Path to topology YAML file",
        exists=True,
        readable=True,
    ),
    minimize_devices: bool = typer.Option(
        False, "--minimize-devices", help="Focus on reducing always-on devices"
    ),
) -> None:
    """Suggest topology optimizations.

    Analyzes for:
    - Underutilized devices
    - Consolidation opportunities
    - Redundant sync paths
    """
    try:
        topology = load_topology(config)

        console.print(f"[bold]Optimization Analysis: {topology.name}[/bold]\n")

        # Count always-on devices
        always_on = [n for n in topology.nodes if n.power_profile and n.power_profile.value == "always_on"]
        console.print(f"Always-on devices: {len(always_on)}")
        for node in always_on:
            power = f"{node.power_watts_idle}W" if node.power_watts_idle else "unknown"
            console.print(f"  • {node.name} ({power})")

        if minimize_devices and len(always_on) > 1:
            console.print("\n[bold]Consolidation Opportunities:[/bold]")
            console.print("  Consider whether functions can be combined:")
            for node in always_on:
                vols = len(node.volumes)
                console.print(f"  • {node.name}: {vols} volume(s)")

        # Check for volumes not part of any sync regime
        synced_volumes: set[str] = set()
        for regime in topology.sync_regimes:
            synced_volumes.add(regime.source_volume)
            synced_volumes.update(regime.target_volumes)

        all_volumes = topology.get_all_volume_ids()
        unsynced = all_volumes - synced_volumes

        if unsynced:
            console.print("\n[bold]Volumes Not in Any Sync Regime:[/bold]")
            for vol_id in unsynced:
                vol_info = topology.get_volume(vol_id)
                if vol_info:
                    node, vol = vol_info
                    console.print(f"  • {vol_id} on {node.name}")

    except ValidationError as e:
        print_error(e.message)
        raise typer.Exit(1)

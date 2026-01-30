"""Suggest commands for storage planner."""

from pathlib import Path
from typing import Optional

import typer
from rich.table import Table
from rich.panel import Panel

from storage_planner.loaders import load_topology, load_all_catalogs, ValidationError
from storage_planner.output import console, print_error, print_info, print_json
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
from storage_planner.cli.paths import resolve_config_path

app = typer.Typer(no_args_is_help=True)


@app.command("hardware")
def suggest_hardware(
    config: Optional[Path] = typer.Argument(
        None,
        help="Path to topology YAML file (defaults to ./topology.yaml)",
    ),
    catalog_dir: Optional[Path] = typer.Option(
        None, "--catalog", "-c", help="Path to catalog directory"
    ),
    budget: Optional[float] = typer.Option(
        None, "--budget", "-b", help="Maximum budget for recommendations"
    ),
    json_output: bool = typer.Option(False, "--json", help="Output JSON"),
) -> None:
    """Suggest hardware based on topology gaps.

    Analyzes the topology to find:
    - Volumes without assigned hardware
    - Redundancy gaps that could be filled with additional storage
    - Upgrade opportunities
    """
    try:
        config_path = resolve_config_path(config)
        topology = load_topology(config_path)

        hardware = HardwareCatalog()
        prices = MarketPrices()
        if catalog_dir and catalog_dir.exists():
            hardware, _, prices = load_all_catalogs(catalog_dir)

        if not json_output:
            console.print(f"[bold]Hardware Suggestions: {topology.name}[/bold]\n")

        suggestions_made = False
        volume_suggestions: list[dict] = []
        redundancy_suggestions: list[dict] = []

        # Check for volumes without product_id that could benefit from recommendations
        for node in topology.nodes:
            for volume in node.volumes:
                if not volume.product_id and not volume.purchase_cost:
                    suggestions_made = True
                    volume_suggestions.append(
                        _suggest_for_volume(volume, node, hardware, prices, budget, json_output)
                    )

        # Check redundancy gaps
        redundancy_results = analyze_redundancy(topology)
        failing = [r for r in redundancy_results if not r.meets_requirements]

        if failing:
            suggestions_made = True
            if not json_output:
                console.print("\n[bold]Redundancy Gaps[/bold]")
            for r in failing:
                if not json_output:
                    console.print(
                        f"\n  [yellow]{r.dataset_name}[/yellow]: needs {r.required_copies - r.actual_copies} "
                        f"more copies, {r.required_locations - r.actual_locations} more locations"
                    )
                # Suggest storage to add
                redundancy_suggestions.append(
                    _suggest_for_redundancy_gap(r, topology, hardware, prices, budget, json_output)
                )

        if json_output:
            print_json(
                {
                    "topology": {"name": topology.name, "path": config_path},
                    "volume_suggestions": volume_suggestions,
                    "redundancy_suggestions": redundancy_suggestions,
                }
            )
            return

        if not suggestions_made:
            print_info("No hardware gaps identified. Topology looks complete.")

    except ValidationError as e:
        print_error(e.message)
        raise typer.Exit(1)


def _suggest_for_volume(volume, node, hardware, prices, budget, json_output: bool = False):
    """Suggest hardware for a volume."""
    from storage_planner.analysis.utils import parse_size

    if not json_output:
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

    suggestions = []
    if matching:
        for product, mp in matching[:5]:  # Top 5
            suggestions.append(
                {
                    "product_id": product.id,
                    "name": product.name,
                    "retail_price": product.retail_price,
                    "used_price_mid": mp.price_mid if mp else None,
                    "capacity": product.specs.get("capacity"),
                }
            )
        if not json_output:
            console.print("\n  [bold]Suggested Products:[/bold]")
            for product, mp in matching[:5]:  # Top 5
                price_str = f"${product.retail_price:.0f}" if product.retail_price else "?"
                if mp:
                    price_str += f" (used: ${mp.price_mid:.0f})"
                console.print(f"    • {product.name}: {price_str}")
                if product.specs.get("capacity"):
                    console.print(f"      Capacity: {product.specs['capacity']}")
    else:
        if not json_output:
            console.print("  [dim]No matching products in catalog[/dim]")

    return {
        "volume_id": volume.id,
        "node_id": node.id,
        "volume_type": volume.type.value,
        "raw_capacity": volume.raw_capacity,
        "suggestions": suggestions,
    }


def _suggest_for_redundancy_gap(result, topology, hardware, prices, budget, json_output: bool = False):
    """Suggest hardware to fill a redundancy gap."""
    from storage_planner.analysis.utils import parse_size

    dataset = topology.get_dataset(result.dataset_id)
    dataset_size = parse_size(dataset.current_size) if dataset else None

    suggestions = []
    for product in hardware.products:
        if product.discontinued:
            continue
        capacity_str = product.specs.get("capacity") if product.specs else None
        capacity_bytes = parse_size(capacity_str) if capacity_str else None
        if dataset_size and capacity_bytes and capacity_bytes < dataset_size:
            continue

        score = 0.0
        reasons = []

        if dataset_size and capacity_bytes:
            score += 2.0
            reasons.append("Meets capacity requirement")

        if dataset and dataset.data_type:
            if any(dataset.data_type in uc.lower() for uc in product.use_cases):
                score += 1.0
                reasons.append(f"Use case match: {dataset.data_type}")

        if dataset and dataset.criticality.value == "critical":
            if product.category.value in ("nas", "enclosure"):
                score += 0.5
                reasons.append("Suited for redundancy targets")

        if topology.constraints.max_noise_db_home is not None and product.noise_db is not None:
            if product.noise_db <= topology.constraints.max_noise_db_home:
                score += 0.5
                reasons.append("Meets noise constraint")

        price = product.retail_price or 0
        mp = prices.get_best_price(product.id)
        used_price = mp.price_mid if mp else None
        if budget and price > budget and (not used_price or used_price > budget):
            continue

        suggestions.append(
            {
                "product_id": product.id,
                "name": product.name,
                "retail_price": product.retail_price,
                "used_price_mid": used_price,
                "capacity": capacity_str,
                "score": score,
                "reasons": reasons[:3],
            }
        )

    suggestions.sort(
        key=lambda s: (-s["score"], s["retail_price"] if s["retail_price"] is not None else 1e9)
    )
    top = suggestions[:5]

    if not json_output:
        if top:
            console.print("  [bold]Suggested Products:[/bold]")
            for item in top:
                price_str = f"${item['retail_price']:.0f}" if item["retail_price"] else "?"
                if item["used_price_mid"]:
                    price_str += f" (used: ${item['used_price_mid']:.0f})"
                console.print(f"    • {item['name']}: {price_str}")
                if item.get("capacity"):
                    console.print(f"      Capacity: {item['capacity']}")
                if item.get("reasons"):
                    console.print(f"      Why: {', '.join(item['reasons'])}")
        else:
            console.print("  [dim]No matching products in catalog[/dim]")

    return {
        "dataset_id": result.dataset_id,
        "dataset_name": result.dataset_name,
        "missing_copies": max(result.required_copies - result.actual_copies, 0),
        "missing_locations": max(result.required_locations - result.actual_locations, 0),
        "suggestions": top,
    }


@app.command("software")
def suggest_software(
    config: Optional[Path] = typer.Argument(
        None,
        help="Path to topology YAML file (defaults to ./topology.yaml)",
    ),
    catalog_dir: Optional[Path] = typer.Option(
        None, "--catalog", "-c", help="Path to catalog directory"
    ),
    json_output: bool = typer.Option(False, "--json", help="Output JSON"),
) -> None:
    """Suggest sync/backup software for each dataset.

    Matches dataset characteristics (change rate, criticality, RPO)
    against software capabilities defined in the catalog.
    """
    try:
        config_path = resolve_config_path(config)
        topology = load_topology(config_path)

        software = SoftwareCatalog()
        if catalog_dir and catalog_dir.exists():
            _, software, _ = load_all_catalogs(catalog_dir)

        if not software.software:
            print_error("No software definitions in catalog. Create catalog/software.yaml")
            raise typer.Exit(1)

        if not json_output:
            console.print(f"[bold]Software Suggestions: {topology.name}[/bold]\n")

        suggestions = []
        for dataset in topology.datasets:
            suggestions.append(_suggest_software_for_dataset(dataset, topology, software, json_output))

        if json_output:
            print_json(
                {
                    "topology": {"name": topology.name, "path": config_path},
                    "suggestions": suggestions,
                }
            )
            return

    except ValidationError as e:
        print_error(e.message)
        raise typer.Exit(1)


def _suggest_software_for_dataset(
    dataset: Dataset,
    topology: Topology,
    software: SoftwareCatalog,
    json_output: bool = False,
):
    """Suggest software for a dataset based on its characteristics."""
    if not json_output:
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
        top_score, top_sw, top_reasons = scores[0]
        alt_sw = scores[1][1] if len(scores) > 1 else None
        if not json_output:
            console.print("\n  [bold]Recommended:[/bold]")
            console.print(f"    [green]{top_sw.name}[/green] ({top_sw.type})")
            for reason in top_reasons[:3]:
                console.print(f"      • {reason}")

            if alt_sw:
                console.print(f"\n    Alternative: {alt_sw.name}")
        return {
            "dataset_id": dataset.id,
            "dataset_name": dataset.name,
            "recommended": {
                "id": top_sw.id,
                "name": top_sw.name,
                "type": top_sw.type,
                "reasons": top_reasons[:3],
            },
            "alternative": {
                "id": alt_sw.id,
                "name": alt_sw.name,
                "type": alt_sw.type,
            }
            if alt_sw
            else None,
        }
    else:
        if not json_output:
            console.print("  [dim]No specific recommendation - any general backup tool should work[/dim]")
        return {
            "dataset_id": dataset.id,
            "dataset_name": dataset.name,
            "recommended": None,
            "alternative": None,
        }

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

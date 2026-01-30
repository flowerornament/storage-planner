"""Catalog commands for storage planner."""

from pathlib import Path
from typing import Optional

import typer
from rich.table import Table
from rich.panel import Panel

from storage_planner.loaders import load_all_catalogs, ValidationError
from storage_planner.output import console, print_error
from storage_planner.models import ProductCategory

app = typer.Typer(no_args_is_help=True)


def _resolve_catalog_dir(catalog_dir: Optional[Path]) -> Path:
    """Resolve catalog directory."""
    if catalog_dir:
        return catalog_dir
    default = Path("catalog")
    if default.exists():
        return default
    raise typer.BadParameter("No catalog directory specified and ./catalog not found")


@app.command("list")
def list_cmd(
    category: Optional[str] = typer.Argument(None, help="Category to filter by (ssd, hdd, enclosure)"),
    catalog_dir: Optional[Path] = typer.Option(None, "--catalog", "-c", help="Catalog directory"),
    tag: Optional[list[str]] = typer.Option(None, "--tag", "-t", help="Filter by tag (can repeat)"),
    use_case: Optional[str] = typer.Option(None, "--use-case", "-u", help="Filter by use case"),
    include_discontinued: bool = typer.Option(False, "--include-discontinued", help="Include discontinued products"),
) -> None:
    """List products in the catalog."""
    try:
        cat_dir = _resolve_catalog_dir(catalog_dir)
        hardware, software, prices = load_all_catalogs(cat_dir)

        # Start with all products
        products = hardware.products

        # Filter by category if specified
        if category:
            try:
                cat_enum = ProductCategory(category.lower())
                products = [p for p in products if p.category == cat_enum]
            except ValueError:
                print_error(f"Unknown category: {category}")
                console.print(f"Valid categories: {', '.join(c.value for c in ProductCategory)}")
                raise typer.Exit(1)

        # Filter by tags
        if tag:
            tag_set = [t.lower() for t in tag]
            products = [
                p for p in products
                if any(t.lower() in tag_set for t in p.tags)
            ]

        # Filter by use case
        if use_case:
            use_case_lower = use_case.lower()
            products = [
                p for p in products
                if any(use_case_lower in uc.lower() for uc in p.use_cases)
            ]

        # Filter discontinued
        if not include_discontinued:
            products = [p for p in products if not p.discontinued]

        if not products:
            console.print("[dim]No products found[/dim]")
            return

        table = Table(title="Hardware Catalog")
        table.add_column("ID")
        table.add_column("Name")
        table.add_column("Category")
        table.add_column("Retail")
        table.add_column("Used (mid)")
        table.add_column("Tags")

        for product in products:
            used_price = ""
            mp = prices.get_best_price(product.id)
            if mp:
                used_price = f"${mp.price_mid:.0f}"

            tags_str = ", ".join(product.tags[:3])
            if len(product.tags) > 3:
                tags_str += "..."

            table.add_row(
                product.id,
                product.name,
                product.category.value,
                f"${product.retail_price:.0f}" if product.retail_price else "-",
                used_price or "-",
                tags_str or "-",
            )

        console.print(table)

    except ValidationError as e:
        print_error(e.message)
        raise typer.Exit(1)


@app.command("show")
def show_cmd(
    product_id: str = typer.Argument(..., help="Product ID to show"),
    catalog_dir: Optional[Path] = typer.Option(None, "--catalog", "-c", help="Catalog directory"),
) -> None:
    """Show detailed product information."""
    try:
        cat_dir = _resolve_catalog_dir(catalog_dir)
        hardware, _, prices = load_all_catalogs(cat_dir)

        product = hardware.get_product(product_id)
        if not product:
            print_error(f"Product not found: {product_id}")
            raise typer.Exit(1)

        # Basic info
        console.print(Panel(f"[bold]{product.name}[/bold]", subtitle=product.brand))
        console.print(f"  ID: {product.id}")
        console.print(f"  Category: {product.category.value}")
        if product.model:
            console.print(f"  Model: {product.model}")

        # Specs
        if product.specs:
            console.print("\n[bold]Specifications[/bold]")
            for key, value in product.specs.items():
                console.print(f"  {key}: {value}")

        # Pricing
        console.print("\n[bold]Pricing[/bold]")
        if product.retail_price:
            console.print(f"  Retail: ${product.retail_price:.2f}")
            if product.retail_url:
                console.print(f"  URL: {product.retail_url}")

        market_prices = prices.get_for_product(product_id)
        if market_prices:
            console.print("\n  [bold]Used Market[/bold]")
            for mp in market_prices:
                console.print(f"    {mp.source}: ${mp.price_low}-${mp.price_high} (mid: ${mp.price_mid})")
                console.print(f"      Updated: {mp.last_updated}, Sample: {mp.sample_size}")
                if mp.notes:
                    console.print(f"      Notes: {mp.notes}")

        # Additional info
        if product.noise_db is not None:
            console.print(f"\n  Noise: {product.noise_db} dB")
        if product.aesthetic_notes:
            console.print(f"  Aesthetics: {product.aesthetic_notes}")
        if product.notes:
            console.print(f"\n  Notes: {product.notes}")

    except ValidationError as e:
        print_error(e.message)
        raise typer.Exit(1)


@app.command("search")
def search_cmd(
    query: str = typer.Argument(..., help="Search query"),
    catalog_dir: Optional[Path] = typer.Option(None, "--catalog", "-c", help="Catalog directory"),
) -> None:
    """Search products by name, brand, or notes."""
    try:
        cat_dir = _resolve_catalog_dir(catalog_dir)
        hardware, _, prices = load_all_catalogs(cat_dir)

        results = hardware.search(query)
        if not results:
            console.print(f"[dim]No products matching '{query}'[/dim]")
            return

        table = Table(title=f"Search Results: '{query}'")
        table.add_column("ID")
        table.add_column("Name")
        table.add_column("Category")
        table.add_column("Retail")

        for product in results:
            table.add_row(
                product.id,
                product.name,
                product.category.value,
                f"${product.retail_price:.0f}" if product.retail_price else "-",
            )

        console.print(table)

    except ValidationError as e:
        print_error(e.message)
        raise typer.Exit(1)


@app.command("compare")
def compare_cmd(
    product_ids: list[str] = typer.Argument(..., help="Product IDs to compare"),
    catalog_dir: Optional[Path] = typer.Option(None, "--catalog", "-c", help="Catalog directory"),
) -> None:
    """Compare products side-by-side."""
    try:
        cat_dir = _resolve_catalog_dir(catalog_dir)
        hardware, _, prices = load_all_catalogs(cat_dir)

        products = []
        for pid in product_ids:
            product = hardware.get_product(pid)
            if not product:
                print_error(f"Product not found: {pid}")
                raise typer.Exit(1)
            products.append(product)

        # Collect all spec keys
        all_specs: set[str] = set()
        for p in products:
            all_specs.update(p.specs.keys())

        # Build comparison table
        table = Table(title="Product Comparison")
        table.add_column("Attribute")
        for p in products:
            table.add_column(p.name[:20])

        # Basic attributes
        table.add_row("Brand", *[p.brand for p in products])
        table.add_row("Category", *[p.category.value for p in products])
        table.add_row(
            "Retail",
            *[f"${p.retail_price:.0f}" if p.retail_price else "-" for p in products],
        )

        # Used prices
        used_prices = []
        for p in products:
            mp = prices.get_best_price(p.id)
            if mp:
                used_prices.append(f"${mp.price_mid:.0f}")
            else:
                used_prices.append("-")
        table.add_row("Used (mid)", *used_prices)

        # Specs
        for spec_key in sorted(all_specs):
            values = [str(p.specs.get(spec_key, "-")) for p in products]
            table.add_row(spec_key, *values)

        # Noise
        table.add_row(
            "Noise (dB)",
            *[str(p.noise_db) if p.noise_db is not None else "-" for p in products],
        )

        console.print(table)

    except ValidationError as e:
        print_error(e.message)
        raise typer.Exit(1)


@app.command("software")
def software_cmd(
    catalog_dir: Optional[Path] = typer.Option(None, "--catalog", "-c", help="Catalog directory"),
) -> None:
    """List software definitions in the catalog."""
    try:
        cat_dir = _resolve_catalog_dir(catalog_dir)
        _, software, _ = load_all_catalogs(cat_dir)

        if not software.software:
            console.print("[dim]No software defined[/dim]")
            return

        table = Table(title="Software Catalog")
        table.add_column("ID")
        table.add_column("Name")
        table.add_column("Type")
        table.add_column("Platforms")
        table.add_column("Strengths")

        for sw in software.software:
            strengths = ", ".join(sw.strengths[:3])
            if len(sw.strengths) > 3:
                strengths += "..."

            table.add_row(
                sw.id,
                sw.name,
                sw.type,
                ", ".join(sw.platforms),
                strengths,
            )

        console.print(table)

    except ValidationError as e:
        print_error(e.message)
        raise typer.Exit(1)


@app.command("summary")
def summary_cmd(
    catalog_dir: Optional[Path] = typer.Option(None, "--catalog", "-c", help="Catalog directory"),
) -> None:
    """Show catalog summary - what's cached and available."""
    try:
        cat_dir = _resolve_catalog_dir(catalog_dir)
        hardware, software, prices = load_all_catalogs(cat_dir)

        summary = hardware.summary()

        console.print("[bold]Catalog Summary[/bold]\n")

        # Product counts
        console.print(f"[bold]Products:[/bold] {summary['active_products']} active, {summary['discontinued']} discontinued")

        # By category
        if summary["by_category"]:
            console.print("\n[bold]By Category:[/bold]")
            for cat, count in sorted(summary["by_category"].items()):
                console.print(f"  {cat}: {count}")

        # Tags
        if summary["tags"]:
            console.print("\n[bold]Available Tags:[/bold]")
            console.print(f"  {', '.join(summary['tags'])}")

        # Use cases
        if summary["use_cases"]:
            console.print("\n[bold]Available Use Cases:[/bold]")
            console.print(f"  {', '.join(summary['use_cases'])}")

        # Software
        console.print(f"\n[bold]Software Definitions:[/bold] {len(software.software)}")

        # Market prices
        priced_products = len(set(p.product_id for p in prices.prices))
        console.print(f"[bold]Products with Market Prices:[/bold] {priced_products}")

        # Staleness check
        if prices.prices:
            from datetime import datetime, timedelta
            today = datetime.now().date()
            stale_count = 0
            for mp in prices.prices:
                try:
                    price_date = datetime.strptime(mp.last_updated, "%Y-%m-%d").date()
                    if (today - price_date).days > 30:
                        stale_count += 1
                except ValueError:
                    pass
            if stale_count > 0:
                console.print(f"[yellow]Warning: {stale_count} market price(s) older than 30 days[/yellow]")

    except ValidationError as e:
        print_error(e.message)
        raise typer.Exit(1)

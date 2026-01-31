"""Console output utilities using Rich."""

from rich.console import Console
from rich.panel import Panel
from rich.table import Table

console = Console()


def print_error(message: str) -> None:
    """Print an error message."""
    console.print(f"[red]Error:[/red] {message}")


def print_warning(message: str) -> None:
    """Print a warning message."""
    console.print(f"[yellow]Warning:[/yellow] {message}")


def print_success(message: str) -> None:
    """Print a success message."""
    console.print(f"[green]{message}[/green]")


def print_info(message: str) -> None:
    """Print an info message."""
    console.print(f"[blue]{message}[/blue]")


def create_table(title: str, columns: list[str]) -> Table:
    """Create a Rich table with common styling."""
    table = Table(title=title, show_header=True, header_style="bold")
    for col in columns:
        table.add_column(col)
    return table


def create_panel(content: str, title: str, style: str = "blue") -> Panel:
    """Create a Rich panel."""
    return Panel(content, title=title, border_style=style)

"""Catalog models for hardware and software."""

from typing import Any

from pydantic import BaseModel, Field

from storage_planner.models.enums import ChangeRate, ProductCategory, SyncDirection


class DriveSpecs(BaseModel):
    """Specifications for a storage drive."""

    capacity: str  # e.g., "8TB"
    interface: str  # e.g., "SATA", "NVMe", "USB-C"
    form_factor: str  # e.g., "2.5in", "M.2", "3.5in"
    read_speed: str | None = None
    write_speed: str | None = None
    tbw: str | None = None  # Total bytes written endurance
    warranty_years: int | None = None
    nand_type: str | None = None  # e.g., "QLC", "TLC", "MLC"


class EnclosureSpecs(BaseModel):
    """Specifications for a drive enclosure."""

    bays: int
    interface: str  # e.g., "USB-C", "Thunderbolt 4"
    max_capacity_per_bay: str | None = None
    form_factor: str | None = None  # e.g., "2.5in", "3.5in", "M.2"
    stackable: bool = False
    m4_mini_compatible: bool = True
    raid_support: list[str] = Field(default_factory=list)  # e.g., ["JBOD", "RAID0", "RAID1"]
    power_delivery_watts: int | None = None


class Product(BaseModel):
    """A hardware product in the catalog."""

    id: str
    name: str
    brand: str
    model: str | None = None
    category: ProductCategory
    specs: dict[str, Any] = Field(default_factory=dict)  # Flexible specs
    retail_price: float | None = None
    retail_url: str | None = None
    noise_db: float | None = None
    aesthetic_notes: str | None = None
    notes: str | None = None

    # Research/caching fields
    tags: list[str] = Field(default_factory=list)  # e.g., ["quiet", "high-capacity", "budget"]
    use_cases: list[str] = Field(default_factory=list)  # e.g., ["time-machine-target", "nas-storage"]
    pros: list[str] = Field(default_factory=list)
    cons: list[str] = Field(default_factory=list)
    discontinued: bool = False
    last_verified: str | None = None  # ISO date when info was last checked

    def get_drive_specs(self) -> DriveSpecs | None:
        """Parse specs as drive specifications."""
        if self.category in (ProductCategory.SSD, ProductCategory.HDD):
            return DriveSpecs(**self.specs)
        return None

    def get_enclosure_specs(self) -> EnclosureSpecs | None:
        """Parse specs as enclosure specifications."""
        if self.category == ProductCategory.ENCLOSURE:
            return EnclosureSpecs(**self.specs)
        return None


class MarketPrice(BaseModel):
    """Used market price data for a product."""

    product_id: str
    source: str  # e.g., "ebay", "reddit-hardwareswap", "facebook"
    price_low: float
    price_mid: float
    price_high: float
    last_updated: str  # ISO date
    sample_size: int = 1
    notes: str | None = None


class SoftwareBestFor(BaseModel):
    """Conditions under which software is recommended."""

    change_rate: list[ChangeRate] = Field(default_factory=list)
    direction: SyncDirection | None = None
    criticality: list[str] = Field(default_factory=list)
    data_type: list[str] = Field(default_factory=list)
    target: str | None = None  # e.g., "local-network", "remote-server", "cloud"
    max_rpo: str | None = None  # Recommended when RPO is this or stricter


class Software(BaseModel):
    """A sync/backup software definition."""

    id: str
    name: str
    type: str  # "sync", "backup", "replication"
    strengths: list[str] = Field(default_factory=list)
    weaknesses: list[str] = Field(default_factory=list)
    best_for: SoftwareBestFor = Field(default_factory=SoftwareBestFor)
    platforms: list[str] = Field(default_factory=list)  # e.g., ["macos", "linux", "windows"]
    url: str | None = None
    notes: str | None = None


class HardwareCatalog(BaseModel):
    """Complete hardware catalog."""

    products: list[Product] = Field(default_factory=list)

    def get_product(self, product_id: str) -> Product | None:
        """Get a product by ID."""
        for product in self.products:
            if product.id == product_id:
                return product
        return None

    def get_by_category(self, category: ProductCategory) -> list[Product]:
        """Get all products in a category."""
        return [p for p in self.products if p.category == category]

    def search(self, query: str) -> list[Product]:
        """Search products by name, brand, or notes."""
        query = query.lower()
        results = []
        for product in self.products:
            if (
                query in product.name.lower()
                or query in product.brand.lower()
                or (product.notes and query in product.notes.lower())
            ):
                results.append(product)
        return results

    def filter_by_tags(
        self,
        tags: list[str],
        match_all: bool = False,
        exclude_discontinued: bool = True,
    ) -> list[Product]:
        """Filter products by tags.

        Args:
            tags: Tags to match
            match_all: If True, product must have all tags. If False, any tag matches.
            exclude_discontinued: Skip discontinued products
        """
        results = []
        tags_lower = [t.lower() for t in tags]
        for product in self.products:
            if exclude_discontinued and product.discontinued:
                continue
            product_tags = [t.lower() for t in product.tags]
            if match_all:
                if all(t in product_tags for t in tags_lower):
                    results.append(product)
            else:
                if any(t in product_tags for t in tags_lower):
                    results.append(product)
        return results

    def filter_by_use_case(
        self,
        use_case: str,
        exclude_discontinued: bool = True,
    ) -> list[Product]:
        """Get products suitable for a specific use case."""
        use_case_lower = use_case.lower()
        results = []
        for product in self.products:
            if exclude_discontinued and product.discontinued:
                continue
            if any(use_case_lower in uc.lower() for uc in product.use_cases):
                results.append(product)
        return results

    def get_active_products(self) -> list[Product]:
        """Get all non-discontinued products."""
        return [p for p in self.products if not p.discontinued]

    def summary(self) -> dict[str, Any]:
        """Get a summary of catalog contents."""
        active = self.get_active_products()
        by_category: dict[str, int] = {}
        all_tags: set[str] = set()
        all_use_cases: set[str] = set()

        for p in active:
            cat = p.category.value
            by_category[cat] = by_category.get(cat, 0) + 1
            all_tags.update(p.tags)
            all_use_cases.update(p.use_cases)

        return {
            "total_products": len(self.products),
            "active_products": len(active),
            "discontinued": len(self.products) - len(active),
            "by_category": by_category,
            "tags": sorted(all_tags),
            "use_cases": sorted(all_use_cases),
        }


class MarketPrices(BaseModel):
    """Collection of market price data."""

    prices: list[MarketPrice] = Field(default_factory=list)

    def get_for_product(self, product_id: str) -> list[MarketPrice]:
        """Get all market prices for a product."""
        return [p for p in self.prices if p.product_id == product_id]

    def get_best_price(self, product_id: str) -> MarketPrice | None:
        """Get the most recent market price for a product."""
        prices = self.get_for_product(product_id)
        if not prices:
            return None
        return max(prices, key=lambda p: p.last_updated)


class SoftwareCatalog(BaseModel):
    """Collection of software definitions."""

    software: list[Software] = Field(default_factory=list)

    def get_software(self, software_id: str) -> Software | None:
        """Get software by ID."""
        for sw in self.software:
            if sw.id == software_id:
                return sw
        return None

    def get_by_type(self, sw_type: str) -> list[Software]:
        """Get all software of a type."""
        return [sw for sw in self.software if sw.type == sw_type]

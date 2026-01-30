"""Utility functions for analysis."""

import re
from typing import Optional


def parse_size(size_str: str) -> Optional[int]:
    """Parse a size string like '8TB', '500GB', '50MB' to bytes.

    Returns None if parsing fails.
    """
    if not size_str:
        return None

    size_str = size_str.strip().upper()
    match = re.match(r"^([\d.]+)\s*([KMGTP]?B?)$", size_str)
    if not match:
        return None

    value = float(match.group(1))
    unit = match.group(2)

    multipliers = {
        "": 1,
        "B": 1,
        "KB": 1024,
        "K": 1024,
        "MB": 1024**2,
        "M": 1024**2,
        "GB": 1024**3,
        "G": 1024**3,
        "TB": 1024**4,
        "T": 1024**4,
        "PB": 1024**5,
        "P": 1024**5,
    }

    return int(value * multipliers.get(unit, 1))


def format_size(bytes_val: int) -> str:
    """Format bytes as human-readable string."""
    if bytes_val < 1024:
        return f"{bytes_val}B"
    elif bytes_val < 1024**2:
        return f"{bytes_val / 1024:.1f}KB"
    elif bytes_val < 1024**3:
        return f"{bytes_val / 1024**2:.1f}MB"
    elif bytes_val < 1024**4:
        return f"{bytes_val / 1024**3:.1f}GB"
    else:
        return f"{bytes_val / 1024**4:.1f}TB"


def parse_bandwidth(bw_str: str) -> Optional[int]:
    """Parse a bandwidth string like '10Gbps', '500Mbps', '100MB/s' to bits per second.

    Returns None if parsing fails.
    """
    if not bw_str:
        return None

    bw_str = bw_str.strip()
    has_slash = "/s" in bw_str.lower()

    match = re.match(
        r"^([\d.]+)\s*([KMGT]?)\s*([bB])(?:PS|/S)$",
        bw_str,
        re.IGNORECASE,
    )
    if not match:
        return None

    value = float(match.group(1))
    unit = match.group(2).upper()
    bit_or_byte = match.group(3)

    multipliers = {
        "": 1,
        "K": 1000,
        "M": 1000**2,
        "G": 1000**3,
        "T": 1000**4,
    }

    bps = value * multipliers.get(unit, 1)
    # Treat explicit "/s" with uppercase B as bytes per second; otherwise default to bits.
    if bit_or_byte == "B" and has_slash:
        bps *= 8

    return int(bps)


def format_bandwidth(bps: int) -> str:
    """Format bits per second as human-readable string."""
    if bps < 1000:
        return f"{bps}bps"
    elif bps < 1000**2:
        return f"{bps / 1000:.1f}Kbps"
    elif bps < 1000**3:
        return f"{bps / 1000**2:.1f}Mbps"
    elif bps < 1000**4:
        return f"{bps / 1000**3:.1f}Gbps"
    else:
        return f"{bps / 1000**4:.1f}Tbps"


def parse_duration(duration_str: str) -> Optional[int]:
    """Parse a duration string like '1h', '30m', '7d' to seconds.

    Returns None if parsing fails.
    """
    if not duration_str:
        return None

    duration_str = duration_str.strip().lower()
    match = re.match(r"^([\d.]+)\s*([smhdw])$", duration_str)
    if not match:
        return None

    value = float(match.group(1))
    unit = match.group(2)

    multipliers = {
        "s": 1,
        "m": 60,
        "h": 3600,
        "d": 86400,
        "w": 604800,
    }

    return int(value * multipliers.get(unit, 1))


def format_duration(seconds: int) -> str:
    """Format seconds as human-readable duration."""
    if seconds < 60:
        return f"{seconds}s"
    elif seconds < 3600:
        return f"{seconds // 60}m"
    elif seconds < 86400:
        return f"{seconds // 3600}h"
    else:
        return f"{seconds // 86400}d"


def parse_growth_rate(rate_str: str) -> Optional[tuple[float, str, str]]:
    """Parse a growth rate string like '1GB/month', '10%/year'.

    Returns (value, period, kind) where kind is "percent" or "absolute".
    """
    if not rate_str:
        return None

    rate_str = rate_str.strip().lower()

    # Percentage format: 10%/year
    match = re.match(r"^([\d.]+)%/(\w+)$", rate_str)
    if match:
        return (float(match.group(1)), match.group(2), "percent")

    # Absolute format: 1GB/month
    match = re.match(r"^([\d.]+)\s*([kmgtp]?b)/(\w+)$", rate_str)
    if match:
        size_bytes = parse_size(match.group(1) + match.group(2))
        if size_bytes:
            return (float(size_bytes), match.group(3), "absolute")

    return None

"""Tests for analysis utility functions."""

import pytest

from storage_planner.analysis.utils import (
    parse_size,
    format_size,
    parse_bandwidth,
    format_bandwidth,
    parse_duration,
    format_duration,
    parse_growth_rate,
)


class TestParseSize:
    """Test size parsing."""

    def test_bytes(self):
        assert parse_size("100B") == 100
        assert parse_size("100") == 100

    def test_kilobytes(self):
        assert parse_size("1KB") == 1024
        assert parse_size("1K") == 1024

    def test_megabytes(self):
        assert parse_size("1MB") == 1024 * 1024
        assert parse_size("512MB") == 512 * 1024 * 1024

    def test_gigabytes(self):
        assert parse_size("1GB") == 1024**3
        assert parse_size("8GB") == 8 * 1024**3

    def test_terabytes(self):
        assert parse_size("1TB") == 1024**4
        assert parse_size("4TB") == 4 * 1024**4

    def test_petabytes(self):
        assert parse_size("1PB") == 1024**5

    def test_decimal_values(self):
        assert parse_size("1.5TB") == int(1.5 * 1024**4)

    def test_case_insensitive(self):
        assert parse_size("1tb") == parse_size("1TB")
        assert parse_size("500gb") == parse_size("500GB")

    def test_whitespace(self):
        assert parse_size(" 1TB ") == 1024**4
        assert parse_size("1 TB") == 1024**4

    def test_invalid(self):
        assert parse_size("") is None
        assert parse_size("invalid") is None
        assert parse_size("TB") is None


class TestFormatSize:
    """Test size formatting."""

    def test_bytes(self):
        assert format_size(100) == "100B"

    def test_kilobytes(self):
        assert format_size(1024) == "1.0KB"
        assert format_size(2048) == "2.0KB"

    def test_megabytes(self):
        assert format_size(1024**2) == "1.0MB"

    def test_gigabytes(self):
        assert format_size(1024**3) == "1.0GB"
        assert format_size(int(1.5 * 1024**3)) == "1.5GB"

    def test_terabytes(self):
        assert format_size(1024**4) == "1.0TB"
        assert format_size(8 * 1024**4) == "8.0TB"


class TestParseBandwidth:
    """Test bandwidth parsing."""

    def test_bits_per_second(self):
        assert parse_bandwidth("1000bps") == 1000
        assert parse_bandwidth("1000BPS") == 1000

    def test_kilobits(self):
        assert parse_bandwidth("1Kbps") == 1000

    def test_megabits(self):
        assert parse_bandwidth("100Mbps") == 100_000_000

    def test_gigabits(self):
        assert parse_bandwidth("1Gbps") == 1_000_000_000
        assert parse_bandwidth("10Gbps") == 10_000_000_000

    def test_terabits(self):
        assert parse_bandwidth("1Tbps") == 1_000_000_000_000

    def test_alternative_format(self):
        assert parse_bandwidth("1Gb/s") == 1_000_000_000

    def test_bytes_per_second(self):
        assert parse_bandwidth("100MB/s") == 800_000_000
        assert parse_bandwidth("1GB/s") == 8_000_000_000

    def test_invalid(self):
        assert parse_bandwidth("") is None
        assert parse_bandwidth("invalid") is None
        assert parse_bandwidth("100bytes") is None


class TestFormatBandwidth:
    """Test bandwidth formatting."""

    def test_bits(self):
        assert format_bandwidth(500) == "500bps"

    def test_kilobits(self):
        assert format_bandwidth(1_000) == "1.0Kbps"

    def test_megabits(self):
        assert format_bandwidth(100_000_000) == "100.0Mbps"

    def test_gigabits(self):
        assert format_bandwidth(1_000_000_000) == "1.0Gbps"


class TestParseDuration:
    """Test duration parsing."""

    def test_seconds(self):
        assert parse_duration("30s") == 30
        assert parse_duration("1s") == 1

    def test_minutes(self):
        assert parse_duration("5m") == 300
        assert parse_duration("1m") == 60

    def test_hours(self):
        assert parse_duration("1h") == 3600
        assert parse_duration("24h") == 86400

    def test_days(self):
        assert parse_duration("1d") == 86400
        assert parse_duration("7d") == 604800

    def test_weeks(self):
        assert parse_duration("1w") == 604800

    def test_decimal(self):
        assert parse_duration("1.5h") == 5400

    def test_invalid(self):
        assert parse_duration("") is None
        assert parse_duration("invalid") is None
        assert parse_duration("1y") is None  # years not supported


class TestFormatDuration:
    """Test duration formatting."""

    def test_seconds(self):
        assert format_duration(30) == "30s"

    def test_minutes(self):
        assert format_duration(60) == "1m"
        assert format_duration(300) == "5m"

    def test_hours(self):
        assert format_duration(3600) == "1h"
        assert format_duration(7200) == "2h"

    def test_days(self):
        assert format_duration(86400) == "1d"
        assert format_duration(172800) == "2d"


class TestParseGrowthRate:
    """Test growth rate parsing."""

    def test_percentage_monthly(self):
        result = parse_growth_rate("10%/month")
        assert result == (10.0, "month", "percent")

    def test_percentage_yearly(self):
        result = parse_growth_rate("25%/year")
        assert result == (25.0, "year", "percent")

    def test_absolute_monthly(self):
        result = parse_growth_rate("1GB/month")
        assert result is not None
        value, period, kind = result
        assert value == 1024**3
        assert period == "month"
        assert kind == "absolute"

    def test_absolute_yearly(self):
        result = parse_growth_rate("10TB/year")
        assert result is not None
        value, period, kind = result
        assert value == 10 * 1024**4
        assert period == "year"
        assert kind == "absolute"

    def test_invalid(self):
        assert parse_growth_rate("") is None
        assert parse_growth_rate("invalid") is None

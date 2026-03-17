//! Spec parsing utilities
//!
//! Parse typed attributes like capacity ("4TB"), speed ("560MB/s"), noise ("32dB").

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::fmt;

/// A capacity value with unit
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Capacity {
    pub bytes: u64,
}

impl Capacity {
    pub const KB: u64 = 1_000;
    pub const MB: u64 = 1_000_000;
    pub const GB: u64 = 1_000_000_000;
    pub const TB: u64 = 1_000_000_000_000;
    pub const PB: u64 = 1_000_000_000_000_000;

    // Binary units
    pub const KIB: u64 = 1_024;
    pub const MIB: u64 = 1_024 * 1_024;
    pub const GIB: u64 = 1_024 * 1_024 * 1_024;
    pub const TIB: u64 = 1_024 * 1_024 * 1_024 * 1_024;

    pub fn from_bytes(bytes: u64) -> Self {
        Self { bytes }
    }

    pub fn parse(s: &str) -> Result<Self> {
        let s = s.trim().to_uppercase();

        // Try to extract number and unit
        let (num_str, unit) = s
            .find(|c: char| c.is_alphabetic())
            .map(|i| s.split_at(i))
            .unwrap_or((&s, "B"));

        let num: f64 = num_str
            .trim()
            .parse()
            .map_err(|_| anyhow!("Invalid number: {}", num_str))?;

        let multiplier = match unit.trim() {
            "B" | "" => 1,
            "KB" | "K" => Self::KB,
            "MB" | "M" => Self::MB,
            "GB" | "G" => Self::GB,
            "TB" | "T" => Self::TB,
            "PB" | "P" => Self::PB,
            "KIB" | "KI" => Self::KIB,
            "MIB" | "MI" => Self::MIB,
            "GIB" | "GI" => Self::GIB,
            "TIB" | "TI" => Self::TIB,
            _ => return Err(anyhow!("Unknown capacity unit: {}", unit)),
        };

        Ok(Self {
            bytes: (num * multiplier as f64) as u64,
        })
    }

    pub fn as_tb(&self) -> f64 {
        self.bytes as f64 / Self::TB as f64
    }

    pub fn as_gb(&self) -> f64 {
        self.bytes as f64 / Self::GB as f64
    }
}

impl fmt::Display for Capacity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.bytes >= Self::TB {
            write!(f, "{:.1}TB", self.as_tb())
        } else if self.bytes >= Self::GB {
            write!(f, "{:.1}GB", self.as_gb())
        } else if self.bytes >= Self::MB {
            write!(f, "{:.1}MB", self.bytes as f64 / Self::MB as f64)
        } else {
            write!(f, "{}B", self.bytes)
        }
    }
}

/// A speed value (bytes per second)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Speed {
    pub bytes_per_sec: u64,
}

impl Speed {
    pub fn parse(s: &str) -> Result<Self> {
        let s = s.trim().to_uppercase();

        // Remove "/S" suffix if present
        let s = s.trim_end_matches("/S").trim_end_matches("/SEC");

        // Parse using capacity logic
        let capacity = Capacity::parse(s)?;
        Ok(Self {
            bytes_per_sec: capacity.bytes,
        })
    }

    pub fn as_mbps(&self) -> f64 {
        self.bytes_per_sec as f64 / Capacity::MB as f64
    }

    pub fn as_gbps(&self) -> f64 {
        self.bytes_per_sec as f64 / Capacity::GB as f64
    }
}

impl fmt::Display for Speed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.bytes_per_sec >= Capacity::GB {
            write!(f, "{:.1}GB/s", self.as_gbps())
        } else {
            write!(f, "{:.0}MB/s", self.as_mbps())
        }
    }
}

/// Format a bandwidth value in bytes/sec to a human-readable string.
pub fn format_bandwidth(bytes_per_sec: i64) -> String {
    let bps = bytes_per_sec as f64;
    if bps >= Capacity::GB as f64 {
        format!("{:.1} GB/s", bps / Capacity::GB as f64)
    } else if bps >= Capacity::MB as f64 {
        format!("{:.1} MB/s", bps / Capacity::MB as f64)
    } else if bps >= Capacity::KB as f64 {
        format!("{:.1} KB/s", bps / Capacity::KB as f64)
    } else {
        format!("{} B/s", bytes_per_sec)
    }
}

/// A noise level in decibels
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NoiseLevel {
    pub db: f64,
}

#[allow(dead_code)]
impl NoiseLevel {
    pub fn parse(s: &str) -> Result<Self> {
        let s = s.trim().to_uppercase();
        let num_str = s.trim_end_matches("DB").trim_end_matches("DBA").trim();
        let db: f64 = num_str
            .parse()
            .map_err(|_| anyhow!("Invalid noise level: {}", s))?;
        Ok(Self { db })
    }

    /// Check if this noise level is within a limit
    pub fn within(&self, limit: f64) -> bool {
        self.db <= limit
    }
}

impl fmt::Display for NoiseLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.0}dB", self.db)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capacity_parse() {
        assert_eq!(Capacity::parse("4TB").unwrap().bytes, 4 * Capacity::TB);
        assert_eq!(Capacity::parse("500GB").unwrap().bytes, 500 * Capacity::GB);
        assert_eq!(
            Capacity::parse("1.5TB").unwrap().bytes,
            (1.5 * Capacity::TB as f64) as u64
        );
        assert_eq!(Capacity::parse("4 TB").unwrap().bytes, 4 * Capacity::TB);
        assert_eq!(Capacity::parse("4tb").unwrap().bytes, 4 * Capacity::TB);
    }

    #[test]
    fn test_capacity_display() {
        assert_eq!(Capacity::from_bytes(4 * Capacity::TB).to_string(), "4.0TB");
        assert_eq!(
            Capacity::from_bytes(500 * Capacity::GB).to_string(),
            "500.0GB"
        );
    }

    #[test]
    fn test_speed_parse() {
        assert_eq!(
            Speed::parse("560MB/s").unwrap().bytes_per_sec,
            560 * Capacity::MB
        );
        assert_eq!(
            Speed::parse("7GB/s").unwrap().bytes_per_sec,
            7 * Capacity::GB
        );
        assert_eq!(
            Speed::parse("560MB").unwrap().bytes_per_sec,
            560 * Capacity::MB
        );
    }

    #[test]
    fn test_noise_parse() {
        assert_eq!(NoiseLevel::parse("32dB").unwrap().db, 32.0);
        assert_eq!(NoiseLevel::parse("0db").unwrap().db, 0.0);
        assert!(NoiseLevel { db: 25.0 }.within(30.0));
        assert!(!NoiseLevel { db: 35.0 }.within(30.0));
    }
}

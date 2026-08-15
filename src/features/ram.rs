use std::sync::Arc;

use tokio::fs::read_to_string;
use tokio::time::{interval, Duration};

use crate::config::RamConfig;
use crate::features::feature_trait::{_publish_update, FeatureState};
use crate::FeatureTrait;
use crate::StatusBar;

pub struct Ram {
    status_bar: Arc<StatusBar>,
    config: RamConfig,
    state: FeatureState,
}

#[derive(Debug)]
struct RamSnapshot {
    total: u128,
    used: u128,
}

#[async_trait::async_trait]
impl FeatureTrait for Ram {
    /// Default RAM state.
    fn new(status_bar: Arc<StatusBar>) -> Self {
        Self {
            status_bar,
            config: RamConfig::default(),
            state: FeatureState::default(),
        }
    }

    /// Refresh RAM pressure with a fixed internal cadence.
    async fn pull(&mut self) {
        let mut interval = interval(Duration::from_secs(1));
        interval.tick().await;

        loop {
            let sample = _read_ram_snapshot()
                .await
                .map(|snapshot| self._format_output(&snapshot));
            let update = self.state.update("ram", sample);

            _publish_update(update, &self.status_bar.ram, &self.status_bar.redraw).await;

            interval.tick().await;
        }
    }
}

impl Ram {
    /// Swap feature settings.
    pub fn set_config(&mut self, config: RamConfig) {
        self.config = config;
    }

    /// Render used bytes and used percent from one snapshot.
    fn _format_output(&self, snapshot: &RamSnapshot) -> String {
        format!(
            "{}{} · {:.0}%",
            self.config.glyph,
            _format_bytes(snapshot.used),
            _used_percent(snapshot)
        )
    }
}

/// Read a consistent RAM snapshot from `/proc/meminfo`.
async fn _read_ram_snapshot() -> Result<RamSnapshot, String> {
    let meminfo = read_to_string("/proc/meminfo")
        .await
        .map_err(|err| format!("Failed to read /proc/meminfo: {err}"))?;

    _from_meminfo(meminfo.as_str())
}

/// Parse the RAM fields used by the current status calculation.
fn _from_meminfo(meminfo: &str) -> Result<RamSnapshot, String> {
    let mut total = None;
    let mut available = None;
    let mut free = 0;
    let mut buffers = 0;
    let mut cached = 0;
    let mut reclaimable = 0;

    for line in meminfo.lines() {
        let Some((field, value)) = line.split_once(':') else {
            continue;
        };

        match field {
            "MemTotal" => total = Some(_from_meminfo_value(field, value)?),
            "MemAvailable" => available = Some(_from_meminfo_value(field, value)?),
            "MemFree" => free = _from_meminfo_value(field, value)?,
            "Buffers" => buffers = _from_meminfo_value(field, value)?,
            "Cached" => cached = _from_meminfo_value(field, value)?,
            "SReclaimable" => reclaimable = _from_meminfo_value(field, value)?,
            _ => continue,
        }
    }

    let total = total.ok_or_else(|| "MemTotal is missing from /proc/meminfo".to_string())?;

    if total == 0 {
        return Err("MemTotal must be greater than zero".to_string());
    }

    let available = match available {
        Some(available) => available,
        None => free
            .saturating_add(buffers)
            .saturating_add(cached)
            .saturating_add(reclaimable),
    };

    let used = total.saturating_sub(available);

    Ok(RamSnapshot { total, used })
}

/// Parse one named `meminfo` value from kibibytes to bytes.
fn _from_meminfo_value(field: &str, value: &str) -> Result<u128, String> {
    let mut parts = value.split_whitespace();
    let raw = parts.next().unwrap_or("");
    let number = raw
        .parse::<u128>()
        .map_err(|_| format!("Invalid {field} value: {raw}"))?;
    let unit = parts.next().unwrap_or("");

    if unit != "kB" {
        return Err(format!("Invalid {field} unit: {unit}"));
    }

    number
        .checked_mul(1024)
        .ok_or_else(|| format!("{field} value is too large"))
}

/// Format bytes with one binary unit digit.
fn _format_bytes(bytes: u128) -> String {
    const STEP: f64 = 1024.0;

    if bytes < 1024 {
        return format!("{bytes}B");
    }

    let units = ["K", "M", "G", "T", "P"];
    let mut value = bytes as f64;
    let mut unit = "B";

    for next_unit in units {
        value /= STEP;
        unit = next_unit;

        if value < STEP {
            break;
        }
    }

    format!("{value:.1}{unit}")
}

/// Keep the percentage aligned with the rendered used value.
fn _used_percent(snapshot: &RamSnapshot) -> f64 {
    (snapshot.used as f64 / snapshot.total as f64) * 100.0
}

#[cfg(test)]
mod tests {
    use super::{_from_meminfo, _from_meminfo_value, _used_percent, RamSnapshot};

    /// Parse a complete meminfo fixture without reading the host.
    #[test]
    fn parse_meminfo_fixture() {
        let snapshot = _from_meminfo(include_str!("../../tests/fixtures/proc/meminfo.txt"))
            .unwrap();

        assert_eq!(snapshot.total, 1_024_000_000);
        assert_eq!(snapshot.used, 384_000_000);
    }

    /// Fall back to free, buffer, cache, and reclaimable slab bytes.
    #[test]
    fn parse_legacy_meminfo_fixture() {
        let snapshot = _from_meminfo(include_str!(
            "../../tests/fixtures/proc/meminfo_legacy.txt",
        )).unwrap();

        assert_eq!(snapshot.total, 1_024_000_000);
        assert_eq!(snapshot.used, 583_680_000);
    }

    /// Reject meminfo with a malformed total field.
    #[test]
    fn reject_meminfo_without_numeric_total() {
        let snapshot = _from_meminfo("MemTotal: unknown kB\nMemFree: 20 kB\n");

        assert_eq!(
            snapshot.unwrap_err(),
            "Invalid MemTotal value: unknown".to_string(),
        );
    }

    /// Require the total field before calculating a percentage.
    #[test]
    fn reject_missing_memtotal() {
        let snapshot = _from_meminfo("MemFree: 20 kB\n");

        assert_eq!(
            snapshot.unwrap_err(),
            "MemTotal is missing from /proc/meminfo".to_string(),
        );
    }

    /// Report a malformed optional field instead of treating it as zero.
    #[test]
    fn reject_malformed_available_value() {
        let snapshot = _from_meminfo(
            "MemTotal: 100 kB\nMemAvailable: unknown kB\n",
        );

        assert_eq!(
            snapshot.unwrap_err(),
            "Invalid MemAvailable value: unknown".to_string(),
        );
    }

    /// Parse the kernel-style unit suffix from meminfo.
    #[test]
    fn parse_meminfo_kib_line() {
        let bytes = _from_meminfo_value("MemTotal", "32794828 kB").unwrap();

        assert_eq!(bytes, 33_581_903_872);
    }

    /// Saturate inconsistent kernel values instead of exceeding 100%.
    #[test]
    fn saturate_available_above_total() {
        let snapshot = _from_meminfo(
            "MemTotal: 100 kB\nMemAvailable: 120 kB\n",
        ).unwrap();

        assert_eq!(snapshot.used, 0);
        assert_eq!(_used_percent(&snapshot), 0.0);
    }

    /// Bound the old-kernel fallback when reclaimable fields exceed total.
    #[test]
    fn saturate_legacy_available_above_total() {
        let snapshot = _from_meminfo(
            "MemTotal: 100 kB\nMemFree: 60 kB\nCached: 60 kB\n",
        ).unwrap();

        assert_eq!(snapshot.used, 0);
        assert_eq!(_used_percent(&snapshot), 0.0);
    }

    /// Keep the percentage tied to the same used value.
    #[test]
    fn used_percent_matches_snapshot() {
        let snapshot = RamSnapshot {
            total: 100,
            used: 39,
        };

        assert_eq!(_used_percent(&snapshot), 39.0);
    }
}

use std::sync::Arc;

use tokio::fs::read_to_string;
use tokio::time::{interval, Duration};

use crate::config::RamConfig;
use crate::FeatureTrait;
use crate::StatusBar;

pub struct Ram {
    status_bar: Arc<StatusBar>,
    config: RamConfig,
}

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
        }
    }

    /// Refresh RAM pressure with a fixed internal cadence.
    async fn pull(&mut self) {
        let mut interval = interval(Duration::from_secs(1));

        loop {
            let output = match _read_ram_snapshot().await {
                Ok(snapshot) => self._format_output(&snapshot),
                Err(_) => String::new(),
            };

            *self.status_bar.ram.write().await = output;
            self.status_bar.redraw.notify_one();

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
    let mut total = 0;
    let mut free = 0;
    let mut buffers = 0;
    let mut cached = 0;

    for line in meminfo.lines() {
        if line.starts_with("MemTotal:") {
            total = _parse_meminfo_bytes(line);
            continue;
        }

        if line.starts_with("MemFree:") {
            free = _parse_meminfo_bytes(line);
            continue;
        }

        if line.starts_with("Buffers:") {
            buffers = _parse_meminfo_bytes(line);
            continue;
        }

        if line.starts_with("Cached:") {
            cached = _parse_meminfo_bytes(line);
        }
    }

    if total == 0 {
        return Err("MemTotal is missing from /proc/meminfo".to_string());
    }

    let reclaimable = free + buffers + cached;
    let used = total.saturating_sub(reclaimable);

    Ok(RamSnapshot { total, used })
}

/// Parse a `meminfo` byte field from one line.
fn _parse_meminfo_bytes(line: &str) -> u128 {
    let value = match line.split_once(':') {
        Some((_, value)) => value.trim(),
        None => "0 kB",
    };

    value
        .split_whitespace()
        .next()
        .and_then(|value| value.parse::<u128>().ok())
        .unwrap_or(0)
        .saturating_mul(1024)
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
    use super::{_from_meminfo, _parse_meminfo_bytes, _used_percent, RamSnapshot};

    /// Parse a complete meminfo fixture without reading the host.
    #[test]
    fn parse_meminfo_fixture() {
        let snapshot = _from_meminfo(include_str!("../../tests/fixtures/proc/meminfo.txt"))
            .unwrap();

        assert_eq!(snapshot.total, 1_024_000_000);
        assert_eq!(snapshot.used, 614_400_000);
    }

    /// Reject meminfo with a malformed total field.
    #[test]
    fn reject_meminfo_without_numeric_total() {
        let snapshot = _from_meminfo("MemTotal: unknown kB\nMemFree: 20 kB\n");

        assert!(snapshot.is_err());
    }

    /// Parse the kernel-style unit suffix from meminfo.
    #[test]
    fn parse_meminfo_kib_line() {
        let bytes = _parse_meminfo_bytes("MemTotal:       32794828 kB");

        assert_eq!(bytes, 33_581_903_872);
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

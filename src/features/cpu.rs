use std::fs::{read_dir, read_to_string as read_to_string_sync};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::fs::read_to_string;
use tokio::time::{interval, Duration};

use crate::config::CpuConfig;
use crate::features::feature_trait::{_publish_update, FeatureState};
use crate::FeatureTrait;
use crate::StatusBar;

const CPU_STAT_PATH: &str = "/proc/stat";
const HWMON_PATH: &str = "/sys/class/hwmon";
const THERMAL_PATH: &str = "/sys/class/thermal";

pub struct Cpu {
    status_bar: Arc<StatusBar>,
    previous_total: CpuSample,
    previous_cores: Vec<CpuSample>,
    config: CpuConfig,
    state: FeatureState,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct CpuSample {
    total: u64,
    idle: u64,
}

struct CpuSnapshot {
    total: CpuSample,
    cores: Vec<CpuSample>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct TemperatureCandidate {
    score: i32,
    millidegrees: i64,
}

#[async_trait::async_trait]
impl FeatureTrait for Cpu {
    /// Default CPU state.
    fn new(status_bar: Arc<StatusBar>) -> Self {
        Self {
            status_bar,
            previous_total: CpuSample::default(),
            previous_cores: vec![],
            config: CpuConfig::default(),
            state: FeatureState::default(),
        }
    }

    /// Refresh CPU load on a fixed internal cadence.
    async fn pull(&mut self) {
        let mut interval = interval(Duration::from_secs(1));
        interval.tick().await;

        loop {
            let sample = self._render_sample().await;
            let update = self.state.update("cpu", sample);

            _publish_update(update, &self.status_bar.cpu, &self.status_bar.redraw).await;

            interval.tick().await;
        }
    }
}

impl Cpu {
    /// Swap feature settings.
    pub fn set_config(&mut self, config: CpuConfig) {
        self.config = config;
    }

    /// Sample CPU state and format one compact status line.
    async fn _render_sample(&mut self) -> Result<String, String> {
        let snapshot = _read_cpu_snapshot().await?;
        let total_usage = _usage_percent(snapshot.total, self.previous_total);
        let core_usages = _core_usages(snapshot.cores.as_slice(), self.previous_cores.as_slice());
        let sparkline = _render_sparkline(core_usages.as_slice(), self.config.sparkline_width);
        let temperature = _read_cpu_temperature();

        self.previous_total = snapshot.total;
        self.previous_cores = snapshot.cores;

        Ok(self._format_output(total_usage, sparkline.as_str(), temperature))
    }

    /// Keep the CPU block terse and consistent.
    fn _format_output(&self, total_usage: f64, sparkline: &str, temperature: Option<i64>) -> String {
        let mut output = format!("{}{}%", self.config.glyph, total_usage.round() as u64);

        if !sparkline.is_empty() {
            output.push(' ');
            output.push_str(sparkline);
        }

        if let Some(temperature) = temperature {
            output.push(' ');
            output.push_str(format!("{temperature}°").as_str());
        }

        output
    }
}

/// Read the current CPU counters from `/proc/stat`.
async fn _read_cpu_snapshot() -> Result<CpuSnapshot, String> {
    let proc_stat = read_to_string(CPU_STAT_PATH)
        .await
        .map_err(|err| format!("Failed to read {CPU_STAT_PATH}: {err}"))?;

    _parse_cpu_snapshot(proc_stat.as_str())
}

/// Parse the aggregate and per-core counters from procfs.
fn _parse_cpu_snapshot(proc_stat: &str) -> Result<CpuSnapshot, String> {
    let mut total = None;
    let mut cores = vec![];

    for line in proc_stat.lines() {
        let Some((name, sample)) = _parse_cpu_line(line) else {
            continue;
        };

        if name == "cpu" {
            total = Some(sample);
            continue;
        }

        cores.push(sample);
    }

    match total {
        Some(total) => Ok(CpuSnapshot { total, cores }),
        None => Err("Missing aggregate cpu line in /proc/stat".to_string()),
    }
}

/// Parse one procfs CPU counter line.
fn _parse_cpu_line(line: &str) -> Option<(&str, CpuSample)> {
    let mut fields = line.split_whitespace();
    let name = fields.next()?;

    if !name.starts_with("cpu") {
        return None;
    }

    let mut total = 0;
    let mut idle = 0;
    let mut index = 0;

    for field in fields {
        let value = field.parse::<u64>().ok()?;
        total += value;

        if index == 3 || index == 4 {
            idle += value;
        }

        index += 1;
    }

    if index < 4 {
        return None;
    }

    Some((name, CpuSample { total, idle }))
}

/// Turn two CPU samples into a percentage.
fn _usage_percent(current: CpuSample, previous: CpuSample) -> f64 {
    let total_delta = current.total.saturating_sub(previous.total);

    if total_delta == 0 {
        return 0.0;
    }

    let idle_delta = current.idle.saturating_sub(previous.idle);
    let busy_delta = total_delta.saturating_sub(idle_delta);
    let usage = (busy_delta as f64 / total_delta as f64) * 100.0;

    usage.clamp(0.0, 100.0)
}

/// Keep per-core deltas aligned with the stored history.
fn _core_usages(current: &[CpuSample], previous: &[CpuSample]) -> Vec<f64> {
    let mut usages = Vec::with_capacity(current.len());
    let mut index = 0;

    while index < current.len() {
        let previous_sample = match previous.get(index) {
            Some(sample) => *sample,
            None => CpuSample::default(),
        };

        usages.push(_usage_percent(current[index], previous_sample));
        index += 1;
    }

    usages
}

/// Render either one glyph per core or a grouped sparkline.
fn _render_sparkline(usages: &[f64], width: usize) -> String {
    if usages.is_empty() {
        return String::new();
    }

    let groups = _compress_usages(usages, width);
    let mut output = String::with_capacity(groups.len());

    for usage in groups {
        output.push(_sparkline_glyph(usage));
    }

    output
}

/// Average core usage buckets when the user wants a shorter sparkline.
fn _compress_usages(usages: &[f64], width: usize) -> Vec<f64> {
    if width == 0 || usages.len() <= width {
        return usages.to_vec();
    }

    let mut groups = Vec::with_capacity(width);
    let mut bucket = 0;

    while bucket < width {
        let start = bucket * usages.len() / width;
        let end = (bucket + 1) * usages.len() / width;
        let mut total = 0.0;
        let mut count = 0;
        let mut index = start;

        while index < end {
            total += usages[index];
            count += 1;
            index += 1;
        }

        if count == 0 {
            groups.push(0.0);
        } else {
            groups.push(total / count as f64);
        }

        bucket += 1;
    }

    groups
}

/// Map one usage bucket onto the bar font sparkline.
fn _sparkline_glyph(usage: f64) -> char {
    let sparkline = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█', '▉'];
    let scaled = (usage.clamp(0.0, 100.0) / 100.0) * 8.0;
    let index = scaled.round() as usize;

    sparkline[index.min(8)]
}

/// Probe sysfs for a CPU temperature without external tools.
fn _read_cpu_temperature() -> Option<i64> {
    if let Some(candidate) = _read_hwmon_temperature() {
        return Some(_round_millidegrees(candidate.millidegrees));
    }

    _read_thermal_temperature().map(|candidate| _round_millidegrees(candidate.millidegrees))
}

/// Prefer hwmon labels that clearly belong to the CPU package.
fn _read_hwmon_temperature() -> Option<TemperatureCandidate> {
    let mut best = None;

    for directory in _read_directory_paths(HWMON_PATH) {
        let name = _read_trimmed_file(directory.join("name").as_path()).unwrap_or_default();

        for file in _read_directory_paths(directory.as_path()) {
            let Some(file_name) = _file_name(file.as_path()) else {
                continue;
            };
            let Some(prefix) = _temperature_prefix(file_name.as_str()) else {
                continue;
            };

            let value = match _read_temperature_value(file.as_path()) {
                Some(value) => value,
                None => continue,
            };
            let label_path = directory.join(format!("{prefix}_label"));
            let label = _read_trimmed_file(label_path.as_path()).unwrap_or_default();
            let score = _score_hwmon_candidate(name.as_str(), label.as_str());

            if score <= 0 {
                continue;
            }

            let candidate = TemperatureCandidate {
                score,
                millidegrees: value,
            };
            best = _better_candidate(best, candidate);
        }
    }

    best
}

/// Fall back to thermal zones when hwmon labels are missing.
fn _read_thermal_temperature() -> Option<TemperatureCandidate> {
    let mut best = None;

    for directory in _read_directory_paths(THERMAL_PATH) {
        let zone_type = match _read_trimmed_file(directory.join("type").as_path()) {
            Some(zone_type) => zone_type,
            None => continue,
        };
        let value = match _read_temperature_value(directory.join("temp").as_path()) {
            Some(value) => value,
            None => continue,
        };
        let score = _score_thermal_candidate(zone_type.as_str());

        if score <= 0 {
            continue;
        }

        let candidate = TemperatureCandidate {
            score,
            millidegrees: value,
        };
        best = _better_candidate(best, candidate);
    }

    best
}

/// Keep only the strongest temperature match.
fn _better_candidate(
    current: Option<TemperatureCandidate>,
    candidate: TemperatureCandidate,
) -> Option<TemperatureCandidate> {
    match current {
        Some(current) if current.score >= candidate.score => Some(current),
        _ => Some(candidate),
    }
}

/// Collect directory children without bubbling filesystem noise into the bar.
fn _read_directory_paths(path: impl AsRef<Path>) -> Vec<PathBuf> {
    let mut paths = vec![];
    let directory = match read_dir(path) {
        Ok(directory) => directory,
        Err(_) => return paths,
    };

    for entry in directory {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };

        paths.push(entry.path());
    }

    paths
}

/// Read a small sysfs value as trimmed text.
fn _read_trimmed_file(path: &Path) -> Option<String> {
    match read_to_string_sync(path) {
        Ok(value) => Some(value.trim().to_string()),
        Err(_) => None,
    }
}

/// Extract the final path component as UTF-8.
fn _file_name(path: &Path) -> Option<String> {
    path.file_name().map(|file_name| file_name.to_string_lossy().to_string())
}

/// Match `temp*_input` files and return the shared prefix.
fn _temperature_prefix(file_name: &str) -> Option<String> {
    if !file_name.starts_with("temp") || !file_name.ends_with("_input") {
        return None;
    }

    Some(file_name.trim_end_matches("_input").to_string())
}

/// Read one millidegree sysfs temperature entry.
fn _read_temperature_value(path: &Path) -> Option<i64> {
    let value = _read_trimmed_file(path)?;

    _from_temperature_value(value.as_str())
}

/// Parse one millidegree sysfs value.
fn _from_temperature_value(value: &str) -> Option<i64> {
    let value = value.trim().parse::<i64>().ok()?;

    if !_is_sane_temperature(value) {
        return None;
    }

    Some(value)
}

/// Reject sensors that are obviously broken or not in Celsius.
fn _is_sane_temperature(millidegrees: i64) -> bool {
    millidegrees > 0 && millidegrees < 150_000
}

/// Score hwmon metadata by how strongly it points at the CPU package.
fn _score_hwmon_candidate(name: &str, label: &str) -> i32 {
    _score_hwmon_name(name) + _score_hwmon_label(label)
}

/// Prefer hwmon devices that are known CPU temperature sources.
fn _score_hwmon_name(name: &str) -> i32 {
    let name = name.to_ascii_lowercase();

    if name.contains("coretemp") || name.contains("k10temp") || name.contains("zenpower") {
        return 20;
    }

    if name.contains("cpu") || name.contains("package") || name.contains("pkg") {
        return 12;
    }

    if _contains_any(name.as_str(), &["acpitz", "nvme", "iwlwifi", "pch", "amdgpu", "battery"]) {
        return -10;
    }

    0
}

/// Prefer labels that point at the package rather than board sensors.
fn _score_hwmon_label(label: &str) -> i32 {
    let label = label.to_ascii_lowercase();

    if label.contains("package id 0") {
        return 24;
    }

    if label.contains("package") || label.contains("tctl") || label.contains("tdie") {
        return 20;
    }

    if label.contains("cpu") {
        return 16;
    }

    if label.contains("core") {
        return 8;
    }

    0
}

/// Prefer thermal zones that explicitly expose CPU package state.
fn _score_thermal_candidate(zone_type: &str) -> i32 {
    let zone_type = zone_type.to_ascii_lowercase();

    if zone_type.contains("x86_pkg_temp") {
        return 24;
    }

    if zone_type.contains("cpu") || zone_type.contains("package") {
        return 18;
    }

    if zone_type.contains("soc") {
        return 8;
    }

    if zone_type.contains("acpitz") || zone_type.contains("pch") {
        return -10;
    }

    0
}

/// Format whole-degree Celsius for a narrow status block.
fn _round_millidegrees(millidegrees: i64) -> i64 {
    ((millidegrees as f64) / 1000.0).round() as i64
}

/// Detect common substrings without callback-heavy iterator chains.
fn _contains_any(text: &str, tokens: &[&str]) -> bool {
    for token in tokens {
        if text.contains(token) {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::{
        _compress_usages, _from_temperature_value, _parse_cpu_snapshot, _score_hwmon_candidate,
        _score_thermal_candidate, _usage_percent, CpuSample,
    };

    /// Parse a sysfs temperature fixture without reading host sensors.
    #[test]
    fn parse_temperature_fixture() {
        let value = _from_temperature_value(include_str!(
            "../../tests/fixtures/sys/class/hwmon/temp1_input",
        ));

        assert_eq!(value, Some(47_250));
    }

    /// Parse aggregate and per-core counters from procfs text.
    #[test]
    fn parse_cpu_snapshot_from_proc_stat() {
        let snapshot = _parse_cpu_snapshot(include_str!("../../tests/fixtures/proc/stat.txt"))
            .unwrap();

        assert_eq!(snapshot.total, CpuSample { total: 200, idle: 50 });
        assert_eq!(snapshot.cores.len(), 2);
        assert_eq!(snapshot.cores[0], CpuSample { total: 100, idle: 25 });
    }

    /// Reject procfs text without an aggregate CPU row.
    #[test]
    fn reject_snapshot_without_aggregate_cpu() {
        let snapshot = _parse_cpu_snapshot("cpu0 50 10 15 20 5 0 0 0 0 0\n");

        assert!(snapshot.is_err());
    }

    /// Keep usage calculations tied to delta counters.
    #[test]
    fn calculate_usage_from_two_samples() {
        let current = CpuSample { total: 200, idle: 90 };
        let previous = CpuSample { total: 100, idle: 50 };

        assert_eq!(_usage_percent(current, previous), 60.0);
    }

    /// Keep the sparkline width stable on high-core machines.
    #[test]
    fn compress_core_usages_to_eight_buckets() {
        let compressed = _compress_usages(
            &[10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0, 100.0],
            8,
        );

        assert_eq!(compressed.len(), 8);
        assert_eq!(compressed[0], 10.0);
        assert_eq!(compressed[7], 95.0);
    }

    /// Prefer explicit CPU package sensors over generic devices.
    #[test]
    fn score_cpu_hwmon_labels_above_noise() {
        assert!(
            _score_hwmon_candidate("k10temp", "Tctl")
                > _score_hwmon_candidate("nvme", "Composite")
        );
    }

    /// Prefer thermal zones that clearly belong to the CPU.
    #[test]
    fn score_cpu_thermal_zones_above_board_zones() {
        assert!(_score_thermal_candidate("x86_pkg_temp") > _score_thermal_candidate("acpitz"));
    }
}

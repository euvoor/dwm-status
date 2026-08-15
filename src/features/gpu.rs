use std::sync::Arc;

use tokio::process::Command;
use tokio::time::{interval, Duration};

use crate::config::GpuConfig;
use crate::features::feature_trait::{_publish_update, FeatureState};
use crate::FeatureTrait;
use crate::StatusBar;

const GPU_QUERY: &str =
    "utilization.gpu,memory.used,memory.total,temperature.gpu,fan.speed";

pub struct Gpu {
    status_bar: Arc<StatusBar>,
    config: GpuConfig,
    state: FeatureState,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct GpuTelemetry {
    usage_percent: Option<u64>,
    vram_percent: Option<u64>,
    temperature_celsius: Option<i64>,
    fan_percent: Option<u64>,
}

#[async_trait::async_trait]
impl FeatureTrait for Gpu {
    /// Default GPU state.
    fn new(status_bar: Arc<StatusBar>) -> Self {
        Self {
            status_bar,
            config: GpuConfig::default(),
            state: FeatureState::default(),
        }
    }

    /// Refresh GPU telemetry on a fixed internal cadence.
    async fn pull(&mut self) {
        let mut interval = interval(Duration::from_secs(1));
        interval.tick().await;

        loop {
            let sample = self._render_sample().await;
            let update = self.state.update("gpu", sample);

            _publish_update(update, &self.status_bar.gpu, &self.status_bar.redraw).await;

            interval.tick().await;
        }
    }
}

impl Gpu {
    /// Swap feature settings.
    pub fn set_config(&mut self, config: GpuConfig) {
        self.config = config;
    }

    /// Read one telemetry sample and format the status block.
    async fn _render_sample(&self) -> Result<String, String> {
        let telemetry = _read_gpu_telemetry().await?;

        Ok(self._format_output(telemetry))
    }

    /// Keep the GPU block compact while preserving the key signals.
    fn _format_output(&self, telemetry: GpuTelemetry) -> String {
        let Some(usage_percent) = telemetry.usage_percent else {
            return String::new();
        };
        let mut segments = vec![format!("{usage_percent}%")];

        if let Some(vram_percent) = telemetry.vram_percent {
            segments.push(format!("V: {vram_percent}%"));
        }

        if let Some(temperature_celsius) = telemetry.temperature_celsius {
            segments.push(format!("T: {temperature_celsius}°"));
        }

        if let Some(fan_percent) = telemetry.fan_percent {
            segments.push(format!("F: {fan_percent}%"));
        }

        format!("{}{}", self.config.glyph, segments.join(" · "))
    }
}

/// Query one NVIDIA GPU telemetry line through the stable CSV interface.
async fn _read_gpu_telemetry() -> Result<GpuTelemetry, String> {
    let output = Command::new("nvidia-smi")
        .arg(format!("--query-gpu={GPU_QUERY}"))
        .arg("--format=csv,noheader,nounits")
        .output()
        .await
        .map_err(|err| format!("Failed to run nvidia-smi: {err}"))?;

    if !output.status.success() {
        return Err(format!("nvidia-smi exited with status {}", output.status));
    }

    let stdout = String::from_utf8(output.stdout)
        .map_err(|err| format!("Invalid nvidia-smi output: {err}"))?;

    _from_gpu_telemetry(stdout.as_str())
}

/// Parse the first reported GPU from the CSV query output.
fn _from_gpu_telemetry(stdout: &str) -> Result<GpuTelemetry, String> {
    let line = match _first_gpu_line(stdout) {
        Some(line) => line,
        None => return Err("nvidia-smi returned no GPU rows".to_string()),
    };
    let fields = _from_csv_fields(line);

    if fields.len() < 5 {
        return Err(format!("Incomplete GPU telemetry row: {line}"));
    }

    let memory_used = _from_unsigned_value(fields[1].as_str());
    let memory_total = _from_unsigned_value(fields[2].as_str());

    Ok(GpuTelemetry {
        usage_percent: _from_unsigned_value(fields[0].as_str()),
        vram_percent: _vram_percent(memory_used, memory_total),
        temperature_celsius: _from_temperature(fields[3].as_str()),
        fan_percent: _from_unsigned_value(fields[4].as_str()),
    })
}

/// Skip blank lines and use the first visible GPU row.
fn _first_gpu_line(stdout: &str) -> Option<&str> {
    for line in stdout.lines() {
        if line.trim().is_empty() {
            continue;
        }

        return Some(line.trim());
    }

    None
}

/// Split the simple NVIDIA CSV row without bringing in a parser crate.
fn _from_csv_fields(line: &str) -> Vec<String> {
    let mut fields = vec![];

    for field in line.split(',') {
        fields.push(field.trim().to_string());
    }

    fields
}

/// Parse an unsigned NVIDIA field when the driver exposes one.
fn _from_unsigned_value(value: &str) -> Option<u64> {
    if _is_missing_field(value) {
        return None;
    }

    value.parse::<u64>().ok()
}

/// Parse a Celsius field when the driver exposes one.
fn _from_temperature(value: &str) -> Option<i64> {
    if _is_missing_field(value) {
        return None;
    }

    value.parse::<i64>().ok()
}

/// Convert used and total frame-buffer memory to whole-percent occupancy.
fn _vram_percent(used: Option<u64>, total: Option<u64>) -> Option<u64> {
    let used = used? as u128;
    let total = total? as u128;

    if total == 0 {
        return None;
    }

    let rounded = (used * 100 + total / 2) / total;

    Some(rounded.min(100) as u64)
}

/// Treat unsupported telemetry values as absent instead of noisy text.
fn _is_missing_field(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();

    normalized.is_empty()
        || normalized == "n/a"
        || normalized == "[not supported]"
        || normalized == "not supported"
}

#[cfg(test)]
mod tests {
    use super::{_from_gpu_telemetry, _vram_percent, Gpu, GpuTelemetry, GPU_QUERY};
    use crate::config::GpuConfig;
    use crate::FeatureTrait;
    use crate::StatusBar;
    use std::sync::Arc;

    /// Parse the compact CSV query row from nvidia-smi.
    #[test]
    fn parse_first_gpu_from_multirow_fixture() {
        let telemetry = _from_gpu_telemetry(include_str!("../../tests/fixtures/nvidia-smi.csv"))
            .unwrap();

        assert_eq!(
            telemetry,
            GpuTelemetry {
                usage_percent: Some(23),
                vram_percent: Some(18),
                temperature_celsius: Some(47),
                fan_percent: Some(32),
            }
        );
    }

    /// Accept partial telemetry when a field is not supported.
    #[test]
    fn parse_gpu_csv_row_with_missing_fan() {
        let telemetry = _from_gpu_telemetry("23, 1475, 8192, 47, N/A\n").unwrap();

        assert_eq!(telemetry.fan_percent, None);
        assert_eq!(telemetry.usage_percent, Some(23));
    }

    /// Query occupancy inputs instead of memory-bus utilization.
    #[test]
    fn query_used_and_total_vram() {
        assert_eq!(
            GPU_QUERY,
            "utilization.gpu,memory.used,memory.total,temperature.gpu,fan.speed",
        );
    }

    /// Omit VRAM occupancy when either capacity input is unavailable.
    #[test]
    fn omit_unsupported_vram_values() {
        let missing_used = _from_gpu_telemetry("23, N/A, 8192, 47, 32\n").unwrap();
        let missing_total = _from_gpu_telemetry("23, 1475, N/A, 47, 32\n").unwrap();

        assert_eq!(missing_used.vram_percent, None);
        assert_eq!(missing_total.vram_percent, None);
    }

    /// Treat malformed and zero capacities as unavailable.
    #[test]
    fn omit_invalid_vram_values() {
        let malformed = _from_gpu_telemetry("23, bad, 8192, 47, 32\n").unwrap();
        let zero_total = _from_gpu_telemetry("23, 10, 0, 47, 32\n").unwrap();

        assert_eq!(malformed.vram_percent, None);
        assert_eq!(zero_total.vram_percent, None);
    }

    /// Round occupancy to the nearest whole percent and cap it at 100.
    #[test]
    fn round_and_bound_vram_percent() {
        assert_eq!(_vram_percent(Some(1475), Some(8192)), Some(18));
        assert_eq!(_vram_percent(Some(3), Some(8)), Some(38));
        assert_eq!(_vram_percent(Some(12), Some(10)), Some(100));
    }

    /// Keep the rendered GPU block terse and ordered.
    #[test]
    fn format_gpu_output() {
        let status_bar = Arc::new(StatusBar::new());
        let mut gpu = Gpu::new(status_bar);

        gpu.set_config(GpuConfig {
            glyph: "gpu ".to_string(),
        });

        let output = gpu._format_output(GpuTelemetry {
            usage_percent: Some(23),
            vram_percent: Some(18),
            temperature_celsius: Some(47),
            fan_percent: Some(32),
        });

        assert_eq!(output, "gpu 23% · V: 18% · T: 47° · F: 32%");
    }
}

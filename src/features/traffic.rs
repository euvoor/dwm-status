use std::sync::Arc;
use std::time::Instant;

use tokio::time::{interval, Duration};

use crate::config::TrafficConfig;
use crate::network::{read_dev_stats, read_primary_interface};
use crate::FeatureTrait;
use crate::StatusBar;

pub struct Traffic {
    status_bar: Arc<StatusBar>,
    previous_sample: Option<TrafficSample>,
    config: TrafficConfig,
}

#[derive(Clone, Debug)]
struct TrafficSample {
    iface: String,
    recv_bytes: u128,
    trans_bytes: u128,
    observed_at: Instant,
}

#[async_trait::async_trait]
impl FeatureTrait for Traffic {
    /// Default traffic state.
    fn new(status_bar: Arc<StatusBar>) -> Self {
        Self {
            status_bar,
            previous_sample: None,
            config: TrafficConfig::default(),
        }
    }

    /// Refresh traffic rates on a fixed internal cadence.
    async fn pull(&mut self) {
        let mut interval = interval(Duration::from_secs(1));
        interval.tick().await;

        loop {
            let output = self._render_sample().await.unwrap_or_default();

            *self.status_bar.traffic.write().await = output;
            self.status_bar.redraw.notify_one();

            interval.tick().await;
        }
    }
}

impl Traffic {
    /// Swap feature settings.
    pub fn set_config(&mut self, config: TrafficConfig) {
        self.config = config;
    }

    /// Sample the routed interface and render RX/TX rates.
    async fn _render_sample(&mut self) -> Result<String, String> {
        let iface = match read_primary_interface().await? {
            Some(iface) => iface.name,
            None => {
                self.previous_sample = None;
                return Ok(String::new());
            }
        };
        let dev_stats = read_dev_stats().await?;
        let stats = match dev_stats.get(&iface) {
            Some(stats) => stats,
            None => {
                self.previous_sample = None;
                return Ok(String::new());
            }
        };
        let current = TrafficSample {
            iface,
            recv_bytes: stats.recv_bytes,
            trans_bytes: stats.trans_bytes,
            observed_at: Instant::now(),
        };
        let (recv_rate, trans_rate) = self._rates_for(current.clone());

        self.previous_sample = Some(current);

        Ok(self._format_output(recv_rate, trans_rate))
    }

    /// Reset rates cleanly when the primary interface changes.
    fn _rates_for(&self, current: TrafficSample) -> (f64, f64) {
        let Some(previous) = &self.previous_sample else {
            return (0.0, 0.0);
        };

        if previous.iface != current.iface {
            return (0.0, 0.0);
        }

        let elapsed = current.observed_at.duration_since(previous.observed_at).as_secs_f64();

        if elapsed <= 0.0 {
            return (0.0, 0.0);
        }

        let recv_delta = current.recv_bytes.saturating_sub(previous.recv_bytes) as f64;
        let trans_delta = current.trans_bytes.saturating_sub(previous.trans_bytes) as f64;

        (recv_delta / elapsed, trans_delta / elapsed)
    }

    /// Keep the traffic block terse and glanceable.
    fn _format_output(&self, recv_rate: f64, trans_rate: f64) -> String {
        format!(
            "{}↓{} ↑{}",
            self.config.glyph,
            _format_rate(recv_rate),
            _format_rate(trans_rate),
        )
    }
}

/// Format a byte-per-second rate with compact binary units.
fn _format_rate(rate: f64) -> String {
    const STEP: f64 = 1024.0;

    if rate < STEP {
        return format!("{:.0}B", rate.max(0.0));
    }

    let units = ["K", "M", "G", "T", "P"];
    let mut value = rate;
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

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{_format_rate, Traffic, TrafficSample};
    use crate::config::TrafficConfig;
    use crate::FeatureTrait;
    use crate::StatusBar;
    use std::sync::Arc;

    /// Keep sub-kibibyte rates integer and compact.
    #[test]
    fn format_small_rate() {
        assert_eq!(_format_rate(999.0), "999B");
    }

    /// Scale binary units without the longer `MiB` style.
    #[test]
    fn format_large_rate() {
        assert_eq!(_format_rate(1_572_864.0), "1.5M");
    }

    /// Reset the visible rate when traffic switches interfaces.
    #[test]
    fn reset_rates_when_interface_changes() {
        let status_bar = Arc::new(StatusBar::new());
        let mut traffic = Traffic::new(status_bar);
        let observed_at = Instant::now();

        traffic.set_config(TrafficConfig {
            glyph: "net ".to_string(),
        });
        traffic.previous_sample = Some(TrafficSample {
            iface: "eth0".to_string(),
            recv_bytes: 1_000,
            trans_bytes: 2_000,
            observed_at,
        });

        let rates = traffic._rates_for(TrafficSample {
            iface: "wlan0".to_string(),
            recv_bytes: 2_000,
            trans_bytes: 3_000,
            observed_at: observed_at + Duration::from_secs(1),
        });

        assert_eq!(rates, (0.0, 0.0));
    }

    /// Clamp rates to zero when kernel counters reset.
    #[test]
    fn reset_rates_when_counters_decrease() {
        let status_bar = Arc::new(StatusBar::new());
        let mut traffic = Traffic::new(status_bar);
        let observed_at = Instant::now();

        traffic.previous_sample = Some(TrafficSample {
            iface: "eth0".to_string(),
            recv_bytes: 2_000,
            trans_bytes: 3_000,
            observed_at,
        });

        let rates = traffic._rates_for(TrafficSample {
            iface: "eth0".to_string(),
            recv_bytes: 1_000,
            trans_bytes: 2_000,
            observed_at: observed_at + Duration::from_secs(1),
        });

        assert_eq!(rates, (0.0, 0.0));
    }
}

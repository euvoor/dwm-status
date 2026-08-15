use std::sync::Arc;

use tokio::time::{interval, Duration, MissedTickBehavior};

use crate::config::{ConnectivityConfig, ConnectivityFormat};
use crate::features::feature_trait::{_publish_update, FeatureState};
use crate::network::{read_connectivity_snapshot, spawn_connectivity_events, ConnectivitySnapshot};
use crate::FeatureTrait;
use crate::StatusBar;

pub struct Connectivity {
    status_bar: Arc<StatusBar>,
    config: ConnectivityConfig,
    state: FeatureState,
}

#[async_trait::async_trait]
impl FeatureTrait for Connectivity {
    /// Default passive state.
    fn new(status_bar: Arc<StatusBar>) -> Self {
        Self {
            status_bar,
            config: ConnectivityConfig::default(),
            state: FeatureState::default(),
        }
    }

    /// Publish passive link state.
    async fn pull(&mut self) {
        let events = match spawn_connectivity_events() {
            Ok(events) => Some(events),
            Err(err) => {
                eprintln!("Feature connectivity event stream failed: {err}; using timed resync");
                None
            }
        };
        let mut refresh = interval(Duration::from_secs(self.config.idle));
        refresh.set_missed_tick_behavior(MissedTickBehavior::Skip);

        self._publish_snapshot().await;

        loop {
            if let Some(events) = &events {
                tokio::select! {
                    _ = events.notified() => {}
                    _ = refresh.tick() => {}
                }
            } else {
                refresh.tick().await;
            }

            self._publish_snapshot().await;
        }
    }
}

impl Connectivity {
    /// Swap feature settings.
    pub fn set_config(&mut self, config: ConnectivityConfig) {
        self.config = config;
    }

    /// Push the latest passive snapshot into the shared slot.
    async fn _publish_snapshot(&mut self) {
        let sample = read_connectivity_snapshot()
            .await
            .map(|snapshot| self._format_output(&snapshot));
        let update = self.state.update("connectivity", sample);

        _publish_update(
            update,
            &self.status_bar.connectivity,
            &self.status_bar.redraw,
        ).await;
    }

    /// Select the configured layout.
    fn _format_output(&self, snapshot: &ConnectivitySnapshot) -> String {
        let output = match self.config.format {
            ConnectivityFormat::Compact => self._to_compact_output(snapshot),
            ConnectivityFormat::Full => self._to_full_output(snapshot),
        };

        format!("{}{}", self.config.glyph, output)
    }

    /// Dense tokens for a narrow bar.
    fn _to_compact_output(&self, snapshot: &ConnectivitySnapshot) -> String {
        let mut output = vec![];

        if let Some(iface) = &snapshot.primary_iface {
            let mut head = vec![];

            if self.config.show_kind {
                head.push(iface.kind.compact_label().to_string());
            }

            if self.config.show_iface {
                head.push(iface.name.clone());
            }

            if ! head.is_empty() {
                output.push(head.join(":"));
            }

            if ! iface.link_up {
                output.push("down".to_string());
            }
        } else {
            output.push("no-iface".to_string());
        }

        if self.config.show_route {
            output.push(if snapshot.has_default_route {
                "gw".to_string()
            } else {
                "no-gw".to_string()
            });
        }

        if self.config.show_dns {
            output.push(snapshot.dns_state.compact_label().to_string());
        }

        output.join(" ")
    }

    /// Labeled fields for wider bars.
    fn _to_full_output(&self, snapshot: &ConnectivitySnapshot) -> String {
        let mut output = vec![];

        if let Some(iface) = &snapshot.primary_iface {
            if self.config.show_kind {
                output.push(format!("kind:{}", iface.kind.full_label()));
            }

            if self.config.show_iface {
                output.push(format!("iface:{}", iface.name));
            }

            output.push(format!("link:{}", if iface.link_up { "up" } else { "down" }));
        } else {
            output.push("iface:none".to_string());
        }

        if self.config.show_route {
            output.push(format!("route:{}", if snapshot.has_default_route { "yes" } else { "no" }));
        }

        if self.config.show_dns {
            output.push(format!("dns:{}", snapshot.dns_state.full_label()));
        }

        output.join(" ")
    }
}

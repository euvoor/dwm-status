use std::sync::Arc;

use tokio::time::{sleep, Duration};

use crate::config::ConnectivityConfig;
use crate::network::{read_connectivity_snapshot, ConnectivitySnapshot};
use crate::FeatureTrait;
use crate::StatusBar;

pub struct Connectivity {
    status_bar: Arc<StatusBar>,
    config: ConnectivityConfig,
}

#[async_trait::async_trait]
impl FeatureTrait for Connectivity {
    /// Default passive state.
    fn new(status_bar: Arc<StatusBar>) -> Self {
        Self {
            status_bar,
            config: ConnectivityConfig::default(),
        }
    }

    /// Publish passive link state.
    async fn pull(&mut self) {
        loop {
            let output = match read_connectivity_snapshot().await {
                Ok(snapshot) => self._format_output(&snapshot),
                Err(_) => format!("{}no-net", self.config.prefix),
            };

            *self.status_bar.connectivity.write().await = output;
            self.status_bar.redraw.notify_one();

            sleep(Duration::from_secs(self.config.idle)).await;
        }
    }
}

impl Connectivity {
    /// Swap feature settings.
    pub fn set_config(&mut self, config: ConnectivityConfig) {
        self.config = config;
    }

    /// Select the configured layout.
    fn _format_output(&self, snapshot: &ConnectivitySnapshot) -> String {
        let output = match self.config.format.as_str() {
            "full" => self._to_full_output(snapshot),
            _ => self._to_compact_output(snapshot),
        };

        format!("{}{}", self.config.prefix, output)
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

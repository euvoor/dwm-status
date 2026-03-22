use crate::config::NetStatsConfig;
use crate::network::{interface_kind, read_dev_stats};
use crate::FeatureTrait;
use crate::StatusBar;
use byte_unit::{Byte, UnitType};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

pub struct NetStats {
    status_bar: Arc<StatusBar>,
    config: NetStatsConfig,
}

#[async_trait::async_trait]
impl FeatureTrait for NetStats {
    fn new(status_bar: Arc<StatusBar>) -> Self {
        Self {
            status_bar,
            config: NetStatsConfig::default(),
        }
    }

    async fn pull(&mut self) {
        let mut prev_stats = HashMap::new();

        loop {
            let dev = match read_dev_stats().await {
                Ok(dev) => dev,
                Err(_) => {
                    *self.status_bar.net_stats.write().await = String::new();
                    sleep(Duration::from_secs(self.config.idle)).await;
                    continue;
                }
            };
            let mut output = vec![];

            for iface in &self.config.ifaces {
                let stats = match dev.get(iface) {
                    Some(stats) => stats,
                    None => continue,
                };
                let prev = prev_stats.entry(iface.clone()).or_insert((stats.recv_bytes, stats.trans_bytes));
                let recv_stat = Byte::from_u128(stats.recv_bytes.saturating_sub(prev.0))
                    .unwrap()
                    .get_appropriate_unit(UnitType::Binary);
                let trans_stat = Byte::from_u128(stats.trans_bytes.saturating_sub(prev.1))
                    .unwrap()
                    .get_appropriate_unit(UnitType::Binary);

                *prev = (stats.recv_bytes, stats.trans_bytes);

                output.push(format!(
                    "{}: {}/{}",
                    interface_kind(iface).net_stats_label(),
                    recv_stat,
                    trans_stat,
                ));
            }

            if !output.is_empty() {
                output = output
                    .iter()
                    .map(|a| format!("({})", a))
                    .collect::<Vec<String>>();
            }

            let output = format!("{}{}", self.config.prefix, output.join(" "));

            *self.status_bar.net_stats.write().await = output;

            sleep(Duration::from_secs(self.config.idle)).await;
        }
    }
}

impl NetStats {
    pub fn set_config(&mut self, config: NetStatsConfig) {
        self.config = config;
    }
}

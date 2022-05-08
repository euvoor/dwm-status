use std::sync::Arc;
use crate::FeatureTrait;
use tokio::sync::{ mpsc, Mutex };
use tokio::time::{ sleep, Duration };
use chrono::offset::Utc;
use crate::StatusBar;
use tokio::fs::read_to_string;
use std::collections::HashMap;
use byte_unit::Byte;
use std::path::Path;
use crate::config::NetStatsConfig;

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

        for iface in &self.config.ifaces {
            prev_stats.insert(iface, (0u128, 0u128));
        }

        loop {
            let dev = read_to_string("/proc/net/dev").await.unwrap();
            let mut output = vec![];

            dev.split('\n')
                .for_each(|line| {
                    if line.contains(':') {
                        let line = line.split_once(':').unwrap();

                        for iface in &self.config.ifaces {
                            if *iface == line.0.trim() {
                                let prev = prev_stats.get(iface).unwrap();
                                let mut stats = line.1.split_whitespace();
                                let recv = stats.next().unwrap().parse::<u128>().unwrap();
                                let trans = stats.skip(7).take(1).map(|n| n.parse::<u128>().unwrap()).collect::<Vec<u128>>()[0];

                                let recv_stat = Byte::from_bytes(recv - prev.0).get_appropriate_unit(true);
                                let trans_stat = Byte::from_bytes(trans - prev.1).get_appropriate_unit(true);

                                prev_stats.insert(iface, (recv, trans));

                                let mut ifacestr = format!("{}/{}", recv_stat, trans_stat);

                                if Path::new(&format!("/sys/class/net/{}/wireless", iface)).exists() {
                                    ifacestr = format!("W: {}", ifacestr);
                                } else {
                                    ifacestr = format!("E: {}", ifacestr);
                                }

                                output.push(ifacestr);
                            }
                        }
                    }
                });

            if ! output.is_empty() {
                output = output.iter().map(|a| format!("({})", a)).collect::<Vec<String>>();
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

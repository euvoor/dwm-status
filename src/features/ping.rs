use std::sync::Arc;
use crate::FeatureTrait;
use tokio::sync::{ Mutex, mpsc };
use std::io::prelude::*;
use tokio::fs::read_to_string;
use byte_unit::Byte;
use tokio::time::{ sleep, Duration };
use std::process::Command;
use crate::StatusBar;
use crate::Vpn;
use crate::config::PingConfig;

pub struct Ping {
    status_bar: Arc<StatusBar>,
    config: PingConfig,
}

#[async_trait::async_trait]
impl FeatureTrait for Ping {
    fn new(status_bar: Arc<StatusBar>) -> Self {
        Self {
            status_bar,
            config: PingConfig::default(),
        }
    }

    async fn pull(&mut self) {
        loop {
            let mut server = "8.8.8.8".to_string();

            if let Some(vpn) = Vpn::is_connected() {
                if vpn {
                    let resolv = read_to_string("/etc/resolv.conf").await.unwrap();

                    resolv.split('\n')
                        .for_each(|line| {
                            if line.trim().starts_with("nameserver") {
                                server = line.split_whitespace().last().unwrap().to_string();
                            }
                        });
                }
            }

            let ping = Command::new("ping")
                .args(&["-n", "-c1", &server])
                .output()
                .unwrap();

            let output = String::from_utf8(ping.stdout).unwrap();
            let mut ping = String::new();

            output.split('\n')
                .for_each(|line| {
                    if line.contains("time=") {
                        let time: Vec<&str> = line.split_whitespace()
                            .skip(6)
                            .take(2)
                            .collect();

                        let value = time[0].split('=').last().unwrap();

                        ping = format!("{} {}", value, time[1]);
                    }
                });


            if ping.len() == 0 {
                ping = self.config.no_internet.clone();
            }

            let output = format!("{}{}", self.config.prefix, ping);

            *self.status_bar.ping.write().await = output;

            sleep(Duration::from_secs(self.config.idle)).await;
        }
    }
}

impl Ping {
    pub fn set_config(&mut self, config: PingConfig) {
        self.config = config;
    }
}

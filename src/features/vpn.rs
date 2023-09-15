use std::sync::Arc;
use crate::FeatureTrait;
use tokio::sync::{ mpsc, Mutex };
use tokio::time::{ sleep, Duration };
use chrono::offset::Utc;
use std::process::Command;
use crate::StatusBar;
use crate::config::VpnConfig;

#[derive(Default, Debug)]
pub struct VpnStatus {
    pub relay: String,
    pub ipv4: String,
    pub location: String,
    pub position: String,
}

pub struct Vpn {
    status_bar: Arc<StatusBar>,
    config: VpnConfig,
}

#[async_trait::async_trait]
impl FeatureTrait for Vpn {
    fn new(status_bar: Arc<StatusBar>) -> Self {
        Self {
            status_bar,
            config: VpnConfig::default(),
        }
    }

    async fn pull(&mut self) {
        return;
        loop {
            let is_connected = Vpn::is_connected();

            if is_connected.is_none() {
                break
            }

            let mut output = String::new();

            if is_connected.unwrap() {
                output = format!("{}", self.config.prefix);

                if let Some(vpn_status) = Vpn::connected_to() {
                    output = format!(
                        "{}{} {}",
                        self.config.prefix,
                        vpn_status.relay,
                        vpn_status.ipv4,
                    );
                }
            }

            *self.status_bar.vpn.write().await = output;

            sleep(Duration::from_secs(self.config.idle)).await;
        }
    }
}

impl Vpn {
    pub fn set_config(&mut self, config: VpnConfig) {
        self.config = config;
    }

    pub fn is_connected() -> Option<bool> {
        match Command::new("mullvad").arg("status").output() {
            Ok(vpn) => {
                let vpn = String::from_utf8(vpn.stdout).unwrap();
                Some(! vpn.contains("Disconnected"))
            },
            Err(err) => {
                dbg!("mullvad", err);
                None
            }
        }
    }

    pub fn connected_to() -> Option<VpnStatus> {
        match Command::new("mullvad").args(&["status", "-l"]).output() {
            Ok(vpn) => {
                let vpn = String::from_utf8(vpn.stdout).unwrap();
                let mut vpn_status = VpnStatus { ..Default::default() };

                vpn.split('\n').for_each(|line| {
                    if line.starts_with("Connected to") {
                        let mut cols = line.split_whitespace();
                        cols.next().unwrap();
                        cols.next().unwrap();

                        vpn_status.relay = cols.next().unwrap().to_string();

                        cols.next().unwrap();

                        vpn_status.location = cols.next().unwrap().to_string();
                        vpn_status.location.push_str(" ");
                        vpn_status.location.push_str(cols.next().unwrap());
                    }

                    if line.starts_with("IPv4:") {
                        vpn_status.ipv4 = line.split_whitespace().last().unwrap().to_string();
                    }

                    if line.starts_with("Position:") {
                        vpn_status.position = line.split_whitespace().last().unwrap().to_string();
                    }
                });

                Some(vpn_status)
            },
            Err(err) => {
                dbg!("mullvad status -l", err);
                None
            }
        }
    }
}

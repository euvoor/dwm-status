use std::sync::Arc;
use crate::FeatureTrait;
use tokio::sync::{ mpsc, Mutex };
use tokio::time::{ sleep, Duration };
use chrono::offset::Utc;
use std::process::Command;
use crate::StatusBar;

pub struct Vpn {
    status_bar: Arc<StatusBar>,
    prefix: &'static str,
    idle: Duration,
}

#[async_trait::async_trait]
impl FeatureTrait for Vpn {
    fn new(
        status_bar: Arc<StatusBar>,
        prefix: &'static str,
        idle: Duration,
    ) -> Self {
        Self {
            status_bar,
            prefix,
            idle,
        }
    }

    async fn pull(&mut self) {
        loop {
            match Command::new("mullvad").arg("status").output() {
                Ok(vpn) => {
                    let vpn = String::from_utf8(vpn.stdout).unwrap();
                    let mut output = String::new();

                    if ! vpn.contains("Tunnel status: Disconnected") {
                        output = format!("{}☂", self.prefix);
                    }

                    *self.status_bar.vpn.write().await = output;

                    sleep(self.idle).await;
                },
                Err(_) => {
                    eprintln!("'mullvad' command not found!");
                    break
                }
            }
        }
    }
}

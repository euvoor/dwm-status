use std::sync::Arc;
use crate::FeatureTrait;
use tokio::sync::{ mpsc, Mutex };
use tokio::time::{ sleep, Duration };
use chrono::offset::Utc;
use std::process::Command;

pub struct Vpn {
    prefix: &'static str,
    position: u8,
    tx: mpsc::Sender<(u8, String)>,
}

#[async_trait::async_trait]
impl FeatureTrait for Vpn {
    fn new(position: u8, prefix: &'static str, tx: mpsc::Sender<(u8, String)>) -> Self {
        Self {
            prefix,
            position,
            tx,
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

                    self.tx.send((self.position, output)).await.unwrap();

                    sleep(Duration::from_secs(10)).await;
                },
                Err(_) => {
                    eprintln!("'mullvad' command not found!");
                    break
                }
            }
        }
    }
}

use std::sync::Arc;
use crate::FeatureTrait;
use tokio::sync::{ Mutex, mpsc };
use std::io::prelude::*;
use tokio::fs::read_to_string;
use byte_unit::Byte;
use tokio::time::{ sleep, Duration };
use std::process::Command;
use crate::StatusBar;

pub struct Ping {
    status_bar: Arc<StatusBar>,
    prefix: &'static str,
    idle: Duration,
}

#[async_trait::async_trait]
impl FeatureTrait for Ping {
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
            let ping = Command::new("ping")
                .args(&["-n", "-c1", "1.1.1.1"])
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

            let output = format!("{}{}", self.prefix, ping);

            *self.status_bar.ping.write().await = output;

            sleep(self.idle).await;
        }
    }
}

use std::sync::Arc;
use crate::FeatureTrait;
use tokio::sync::{ Mutex, mpsc };
use std::io::prelude::*;
use tokio::fs::read_to_string;
use byte_unit::Byte;
use tokio::time::{ sleep, Duration };
use std::process::Command;

pub struct Ping {
    prefix: &'static str,
    position: u8,
    tx: mpsc::Sender<(u8, String)>,
    next: usize,
}

#[async_trait::async_trait]
impl FeatureTrait for Ping {
    fn new(position: u8, prefix: &'static str, tx: mpsc::Sender<(u8, String)>) -> Self {
        Self {
            prefix,
            position,
            tx,
            next: 0,
        }
    }

    async fn pull(&mut self) {
        let servers = [
            "1.1.1.1",
            // // Google
            // "8.8.8.8",
            // "8.8.4.4",
            // // Quad9
            // "9.9.9.9",
            // "149.112.112.112",
            // // OpenDNS
            // "208.67.222.222",
            // "208.67.220.220",
            // // Cloudflare
            // "1.1.1.1",
            // "1.0.0.1",
            // // CleanBrowsing
            // "185.228.168.9",
            // "185.228.169.9",
            // // Alternate DNS
            // "76.76.19.19",
            // "76.223.122.150",
            // // AdGuard DNS
            // "94.140.14.14",
            // "94.140.15.15",
        ];

        loop {
            let server = servers[self.next];
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

            let output = format!("{}{}", self.prefix, ping);

            self.tx.send((self.position, output)).await.unwrap();

            if self.next == servers.len() - 1 {
                self.next = 0;
            } else {
                self.next += 1;
            }

            sleep(Duration::from_secs(1)).await;
        }
    }
}

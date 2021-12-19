pub mod status_bar;

use std::process::Command;
use tokio::time::{ sleep, Duration };
use tokio::sync::mpsc;
use status_bar::StatusBar;

pub fn start_status_bar(mut rx: mpsc::Receiver<(u8, String)>) {
    let mut status_bar = StatusBar::new(255);
    let blocks = status_bar.blocks.clone();

    tokio::spawn(async move {
        while let Some((n, data)) = rx.recv().await {
            status_bar.update(n, data).await;
        }
    });

    tokio::spawn(async move {
        loop {
            let blocks = blocks.clone();
            let blocks = blocks.read().await;

            let status = blocks.iter()
                .filter(|block| block.len() > 0)
                .map(|block| format!("[ {} ]", block))
                .collect::<Vec<String>>()
                .join(" ");

            Command::new("xsetroot")
                .args(&["-name", &status])
                .output()
                .unwrap();

            sleep(Duration::from_secs(1)).await;
        }
    });
}

#![allow(unused_imports)]

mod features;
mod status_bar;

use std::sync::Arc;
use std::process::Command;

use futures::future::join_all;
use futures::stream::FuturesUnordered;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::time::{ sleep, Duration };

use features::FeatureTrait;
use status_bar::StatusBar;

use dwm_status::start_status_bar;

use features::{
    DateTime,
    Ping,
    Memory,
    Cpu,
    Gpu,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (tx, rx) = mpsc::channel(100);

    start_status_bar(rx);

    let futures = FuturesUnordered::new();

    let resources: Vec<Box<dyn FeatureTrait + Send + Sync>> = vec![
        Box::new(DateTime::new(1, "", tx.clone())),
        Box::new(Ping::new(2, "Ping: ", tx.clone())),
        Box::new(Memory::new(3, "Mem: ", tx.clone())),
        Box::new(Cpu::new(4, "Cpu: ", tx.clone())),
        Box::new(Gpu::new(5, "Gpu: ", tx.clone())),
    ];

    for mut resource in resources {
        futures.push(tokio::spawn(async move {
            resource.pull().await;
        }));
    }

    join_all(futures).await;

    Ok(())
}

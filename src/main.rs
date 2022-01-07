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

use features::{
    DateTime,
    Ping,
    Memory,
    Cpu,
    Gpu,
    Vpn,
    NetStats,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let futures = FuturesUnordered::new();
    let status_bar = Arc::new(StatusBar::new());
    let mut net_stats = NetStats::new(status_bar.clone(), "", Duration::from_secs(1));

    net_stats.set_ifaces(vec!["enp10s0", "wlx04d4c464bd3c"]);

    let resources: Vec<Box<dyn FeatureTrait + Send + Sync>> = vec![
        Box::new(net_stats),
        Box::new(Vpn::new(status_bar.clone(), "", Duration::from_secs(1))),
        Box::new(DateTime::new(status_bar.clone(), "", Duration::from_secs(1))),
        Box::new(Ping::new(status_bar.clone(), "", Duration::from_secs(1))),
        Box::new(Memory::new(status_bar.clone(), "Mem: ", Duration::from_secs(1))),
        Box::new(Cpu::new(status_bar.clone(), "Cpu: ", Duration::from_secs(1))),
        Box::new(Gpu::new(status_bar.clone(), "Gpu: ", Duration::from_secs(1))),
    ];

    for mut resource in resources {
        futures.push(tokio::spawn(async move {
            resource.pull().await;
        }));
    }

    let status_bar = status_bar.clone();

    tokio::spawn(async move {
        loop {
            let mut output: Vec<String> = vec![];

            { output.push(status_bar.vpn.read().await.to_string()); }
            { output.push(status_bar.date_time.read().await.to_string()); }
            { output.push(status_bar.ping.read().await.to_string()); }
            { output.push(status_bar.memory.read().await.to_string()); }
            { output.push(status_bar.cpu.read().await.to_string()); }
            { output.push(status_bar.gpu.read().await.to_string()); }
            { output.push(status_bar.net_stats.read().await.to_string()); }

            let output: Vec<String> = output.iter()
                .rev()
                .filter(|stat| ! stat.is_empty())
                .map(|stat| stat.to_string())
                .collect();

            let output = format!("▏{}▕", output.join("▕▏"));

            Command::new("xsetroot")
                .args(&["-name", &output])
                .output()
                .unwrap();

            sleep(Duration::from_secs(1)).await;
        }
    });

    join_all(futures).await;

    Ok(())
}

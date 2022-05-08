#![allow(unused_imports)]

mod config;
mod features;
mod status_bar;

use std::sync::Arc;
use std::process::Command;
use std::fs::read_to_string;

use futures::future::join_all;
use futures::stream::FuturesUnordered;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::time::{ sleep, Duration };
use serde_yaml::from_str;

use config::Config;
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
    let config = from_str::<Config>(&read_to_string("config.yaml").unwrap());

    if config.is_err() {
        eprintln!("Error in config.yaml: {}", config.err().unwrap());
        return Ok(())
    }

    let config = config.unwrap();
    let futures = FuturesUnordered::new();
    let status_bar = Arc::new(StatusBar::new());
    let mut resources: Vec<Box<dyn FeatureTrait + Send + Sync>> = vec![];

    for feature in &config.features {
        match feature.as_str() {
            "net_stats" => {
                let mut net_stats = NetStats::new(status_bar.clone());
                net_stats.set_config(config.net_stats.clone());
                resources.push(Box::new(net_stats));
            },
            "cpu" => {
                let mut cpu = Cpu::new(status_bar.clone());
                cpu.set_config(config.cpu.clone());
                resources.push(Box::new(cpu));
            },
            "vpn" => {
                let mut vpn = Vpn::new(status_bar.clone());
                vpn.set_config(config.vpn.clone());
                resources.push(Box::new(vpn));
            },
            "date_time" => {
                let mut date_time = DateTime::new(status_bar.clone());
                date_time.set_config(config.date_time.clone());
                resources.push(Box::new(date_time));
            },
            "ping" => {
                let mut ping = Ping::new(status_bar.clone());
                ping.set_config(config.ping.clone());
                resources.push(Box::new(ping));
            },
            "memory" => {
                let mut memory = Memory::new(status_bar.clone());
                memory.set_config(config.memory.clone());
                resources.push(Box::new(memory));
            },
            "gpu" => {
                let mut gpu = Gpu::new(status_bar.clone());
                gpu.set_config(config.gpu.clone());
                resources.push(Box::new(gpu));
            },
            name => unimplemented!("Unsupported feature: {}", name),
        };
    }

    for mut resource in resources {
        futures.push(tokio::spawn(async move {
            resource.pull().await;
        }));
    }

    let status_bar = status_bar.clone();

    tokio::spawn(async move {
        loop {
            let mut output: Vec<String> = vec![];

            for feature in &config.features {
                match feature.as_str() {
                    "net_stats" => output.push(status_bar.net_stats.read().await.to_string()),
                    "cpu" => output.push(status_bar.cpu.read().await.to_string()),
                    "vpn" => output.push(status_bar.vpn.read().await.to_string()),
                    "date_time" => output.push(status_bar.date_time.read().await.to_string()),
                    "ping" => output.push(status_bar.ping.read().await.to_string()),
                    "memory" => output.push(status_bar.memory.read().await.to_string()),
                    "gpu" => output.push(status_bar.gpu.read().await.to_string()),
                    name => unimplemented!("Unsupported feature: {}", name),
                };
            }

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

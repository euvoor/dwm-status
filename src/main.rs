#![allow(unused_imports)]

mod config;
mod features;
mod network;
mod status_bar;

use std::fs::read_to_string;
use std::process::Command;
use std::sync::Arc;

use futures::future::join_all;
use futures::stream::FuturesUnordered;

use config::Config;
use features::FeatureTrait;
use status_bar::StatusBar;

use features::{Connectivity, Cpu, DateTime, Gpu, Memory, NetStats};

/// Read the working config.
fn load_config() -> Result<Config, String> {
    let config_path = "config.toml";
    let config = read_to_string(config_path)
        .map_err(|err| format!("Failed to read {config_path}: {err}"))?;

    toml::from_str::<Config>(&config).map_err(|err| format!("Error in {config_path}: {err}"))
}

/// Build the root-window payload.
async fn _build_output(status_bar: &StatusBar, config: &Config) -> String {
    let mut output: Vec<String> = vec![];

    for feature in &config.features {
        match feature.as_str() {
            "connectivity" => output.push(status_bar.connectivity.read().await.to_string()),
            "net_stats" => output.push(status_bar.net_stats.read().await.to_string()),
            "cpu" => output.push(status_bar.cpu.read().await.to_string()),
            "date_time" => output.push(status_bar.date_time.read().await.to_string()),
            "memory" => output.push(status_bar.memory.read().await.to_string()),
            "gpu" => output.push(status_bar.gpu.read().await.to_string()),
            name => unimplemented!("Unsupported feature: {}", name),
        };
    }

    let output: Vec<String> = output
        .iter()
        .rev()
        .filter(|stat| !stat.is_empty())
        .map(|stat| stat.to_string())
        .collect();

    format!("▏{}▕", output.join("▕▏"))
}

/// Push the current line through xsetroot.
fn _set_root_name(output: &str) -> Result<(), String> {
    Command::new("xsetroot")
        .args(["-name", output])
        .output()
        .map_err(|err| format!("Failed to run xsetroot: {err}"))?;

    Ok(())
}

#[tokio::main]
/// Start workers and the renderer.
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = match load_config() {
        Ok(config) => config,
        Err(err) => {
            eprintln!("{err}");
            return Ok(());
        }
    };

    let futures = FuturesUnordered::new();
    let status_bar = Arc::new(StatusBar::new());
    let mut resources: Vec<Box<dyn FeatureTrait + Send + Sync>> = vec![];

    for feature in &config.features {
        match feature.as_str() {
            "connectivity" => {
                let mut connectivity = Connectivity::new(status_bar.clone());
                connectivity.set_config(config.connectivity.clone());
                resources.push(Box::new(connectivity));
            }
            "net_stats" => {
                let mut net_stats = NetStats::new(status_bar.clone());
                net_stats.set_config(config.net_stats.clone());
                resources.push(Box::new(net_stats));
            }
            "cpu" => {
                let mut cpu = Cpu::new(status_bar.clone());
                cpu.set_config(config.cpu.clone());
                resources.push(Box::new(cpu));
            }
            "date_time" => {
                let mut date_time = DateTime::new(status_bar.clone());
                date_time.set_config(config.date_time.clone());
                resources.push(Box::new(date_time));
            }
            "memory" => {
                let mut memory = Memory::new(status_bar.clone());
                memory.set_config(config.memory.clone());
                resources.push(Box::new(memory));
            }
            "gpu" => {
                let mut gpu = Gpu::new(status_bar.clone());
                gpu.set_config(config.gpu.clone());
                resources.push(Box::new(gpu));
            }
            name => unimplemented!("Unsupported feature: {}", name),
        };
    }

    for mut resource in resources {
        futures.push(tokio::spawn(async move {
            resource.pull().await;
        }));
    }

    let render_status_bar = status_bar.clone();

    tokio::spawn(async move {
        let mut last_output = String::new();

        loop {
            render_status_bar.redraw.notified().await;

            let output = _build_output(render_status_bar.as_ref(), &config).await;

            if output == last_output {
                continue;
            }

            match _set_root_name(&output) {
                Ok(_) => last_output = output,
                Err(err) => eprintln!("{err}"),
            }
        }
    });

    status_bar.redraw.notify_one();

    join_all(futures).await;

    Ok(())
}

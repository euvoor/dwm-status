#![allow(unused_imports)]

mod config;
mod features;
mod network;
mod status_bar;
mod x11_root;

use std::env::{args_os, current_dir, var_os};
use std::ffi::OsString;
use std::fs::read_to_string;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use config::Config;
use features::FeatureTrait;
use status_bar::StatusBar;
use x11_root::RootNameWriter;

use features::{Clock, Connectivity, Cpu, Gpu, Ram, Traffic};

/// Keep startup flags in one place.
struct CliArgs {
    config_path: Option<PathBuf>,
}

/// Read the selected config file.
fn load_config() -> Result<Config, String> {
    let cli_args = _parse_cli_args(args_os().skip(1).collect())?;
    let config_path = _resolve_config_path(cli_args.config_path.as_deref())?;
    let config = read_to_string(&config_path)
        .map_err(|err| format!("Failed to read {}: {err}", config_path.display()))?;

    toml::from_str::<Config>(&config)
        .map_err(|err| format!("Error in {}: {err}", config_path.display()))
}

/// Parse the supported startup flags.
fn _parse_cli_args(args: Vec<OsString>) -> Result<CliArgs, String> {
    let mut config_path = None;
    let mut index = 0;

    while index < args.len() {
        let arg = &args[index];

        if arg == "--config" || arg == "-c" {
            index += 1;

            if index >= args.len() {
                return Err("Missing path after --config".to_string());
            }

            config_path = Some(PathBuf::from(&args[index]));
            index += 1;
            continue;
        }

        return Err(format!("Unsupported argument: {}", arg.to_string_lossy()));
    }

    Ok(CliArgs { config_path })
}

/// Pick the first config file that exists.
fn _resolve_config_path(config_path: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(config_path) = config_path {
        return Ok(config_path.to_path_buf());
    }

    let config_paths = _candidate_config_paths();

    for path in &config_paths {
        if path.is_file() {
            return Ok(path.clone());
        }
    }

    Err(_missing_config_error(&config_paths))
}

/// Build the config search list in priority order.
fn _candidate_config_paths() -> Vec<PathBuf> {
    let xdg_config_home = var_os("XDG_CONFIG_HOME").map(PathBuf::from);
    let home = var_os("HOME").map(PathBuf::from);
    let current_dir = _current_dir();

    _candidate_config_paths_from(xdg_config_home, home, current_dir)
}

/// Expand XDG and local config candidates.
fn _candidate_config_paths_from(
    xdg_config_home: Option<PathBuf>,
    home: Option<PathBuf>,
    current_dir: PathBuf,
) -> Vec<PathBuf> {
    let mut config_paths = vec![];

    if let Some(xdg_config_home) = xdg_config_home {
        _push_unique_path(
            &mut config_paths,
            xdg_config_home.join("dwm_status").join("config.toml"),
        );
    }

    if let Some(home) = home {
        _push_unique_path(
            &mut config_paths,
            home.join(".config").join("dwm_status").join("config.toml"),
        );
    }

    _push_unique_path(&mut config_paths, current_dir.join("config.toml"));

    config_paths
}

/// Keep duplicate search paths out of error output.
fn _push_unique_path(config_paths: &mut Vec<PathBuf>, path: PathBuf) {
    if config_paths.contains(&path) {
        return;
    }

    config_paths.push(path);
}

/// Format the missing-config error with the real search order.
fn _missing_config_error(config_paths: &[PathBuf]) -> String {
    let mut message = String::from("Failed to find config.toml. Searched:");

    for path in config_paths {
        message.push_str(&format!("\n- {}", path.display()));
    }

    message.push_str("\nUse --config /path/to/config.toml to select a file explicitly.");

    message
}

/// Resolve the working directory for relative config lookup.
fn _current_dir() -> PathBuf {
    match current_dir() {
        Ok(path) => path,
        Err(_) => PathBuf::from("."),
    }
}

/// Build the root-window payload.
async fn _build_output(status_bar: &StatusBar, config: &Config) -> String {
    let mut output: Vec<String> = vec![];

    for feature in &config.features {
        match feature.as_str() {
            "connectivity" => output.push(status_bar.connectivity.read().await.to_string()),
            "traffic" => output.push(status_bar.traffic.read().await.to_string()),
            "cpu" => output.push(status_bar.cpu.read().await.to_string()),
            "clock" => output.push(status_bar.clock.read().await.to_string()),
            "ram" => output.push(status_bar.ram.read().await.to_string()),
            "gpu" => output.push(status_bar.gpu.read().await.to_string()),
            name => unimplemented!("Unsupported feature: {}", name),
        };
    }

    let mut rendered = vec![];

    for stat in output.iter().rev() {
        if stat.is_empty() {
            continue;
        }

        rendered.push(stat.to_string());
    }

    format!("▏{}▕", rendered.join("▕▏"))
}

/// Run one feature until it stops publishing.
async fn _run_feature(mut feature: Box<dyn FeatureTrait + Send + Sync>) {
    feature.pull().await;
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

    let status_bar = Arc::new(StatusBar::new());
    let root_name_writer = match RootNameWriter::connect() {
        Ok(root_name_writer) => root_name_writer,
        Err(err) => {
            eprintln!("{err}");
            return Ok(());
        }
    };
    let mut resources: Vec<Box<dyn FeatureTrait + Send + Sync>> = vec![];

    for feature in &config.features {
        match feature.as_str() {
            "connectivity" => {
                let mut connectivity = Connectivity::new(status_bar.clone());
                connectivity.set_config(config.connectivity.clone());
                resources.push(Box::new(connectivity));
            }
            "traffic" => {
                let mut traffic = Traffic::new(status_bar.clone());
                traffic.set_config(config.traffic.clone());
                resources.push(Box::new(traffic));
            }
            "cpu" => {
                let mut cpu = Cpu::new(status_bar.clone());
                cpu.set_config(config.cpu.clone());
                resources.push(Box::new(cpu));
            }
            "clock" => {
                let mut clock = Clock::new(status_bar.clone());
                if let Err(err) = clock.set_config(config.clock.clone()) {
                    eprintln!("{err}");
                    return Ok(());
                }
                resources.push(Box::new(clock));
            }
            "ram" => {
                let mut ram = Ram::new(status_bar.clone());
                ram.set_config(config.ram.clone());
                resources.push(Box::new(ram));
            }
            "gpu" => {
                let mut gpu = Gpu::new(status_bar.clone());
                gpu.set_config(config.gpu.clone());
                resources.push(Box::new(gpu));
            }
            name => unimplemented!("Unsupported feature: {}", name),
        };
    }

    for resource in resources {
        tokio::spawn(_run_feature(resource));
    }

    status_bar.redraw.notify_one();
    let mut last_output = String::new();

    loop {
        status_bar.redraw.notified().await;

        let output = _build_output(status_bar.as_ref(), &config).await;

        if output == last_output {
            continue;
        }

        match root_name_writer.set_status(&output) {
            Ok(_) => last_output = output,
            Err(err) => eprintln!("{err}"),
        }
    }

    #[allow(unreachable_code)]
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{_candidate_config_paths_from, _parse_cli_args};
    use std::ffi::OsString;
    use std::path::PathBuf;

    /// Parse an explicit config path from startup flags.
    #[test]
    fn parse_config_arg() {
        let cli_args = _parse_cli_args(vec![
            OsString::from("--config"),
            OsString::from("/tmp/dwm_status.toml"),
        ]).unwrap();

        assert_eq!(cli_args.config_path, Some(PathBuf::from("/tmp/dwm_status.toml")));
    }

    /// Keep config search order stable and deduplicated.
    #[test]
    fn build_config_candidates() {
        let config_paths = _candidate_config_paths_from(
            Some(PathBuf::from("/home/test/.config")),
            Some(PathBuf::from("/home/test")),
            PathBuf::from("/work/tree"),
        );

        assert_eq!(
            config_paths,
            vec![
                PathBuf::from("/home/test/.config/dwm_status/config.toml"),
                PathBuf::from("/work/tree/config.toml"),
            ]
        );
    }
}

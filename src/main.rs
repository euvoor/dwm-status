mod config;
mod features;
mod network;
mod status_bar;
mod x11_root;

use std::env::{args_os, current_dir, var_os};
use std::ffi::OsString;
use std::fs::read_to_string;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use config::{Config, FeatureName};
use futures_util::stream::{FuturesUnordered, StreamExt};
use features::FeatureTrait;
use status_bar::StatusBar;
use x11_root::RootNameWriter;

use features::{Clock, Connectivity, Cpu, Gpu, Ram, Traffic};

/// Keep startup flags in one place.
struct CliArgs {
    config_path: Option<PathBuf>,
}

/// Read the selected config file.
fn _load_config() -> Result<Config, String> {
    let cli_args = _parse_cli_args(args_os().skip(1).collect())?;
    let config_path = _resolve_config_path(cli_args.config_path.as_deref())?;
    let config = read_to_string(&config_path)
        .map_err(|err| format!("Failed to read {}: {err}", config_path.display()))?;

    let config = toml::from_str::<Config>(&config)
        .map_err(|err| format!("Error in {}: {err}", config_path.display()))?;

    config
        .validate()
        .map_err(|err| format!("Error in {}: {err}", config_path.display()))?;

    Ok(config)
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
        match feature {
            FeatureName::Connectivity => {
                output.push(status_bar.connectivity.read().await.to_string())
            }
            FeatureName::Traffic => output.push(status_bar.traffic.read().await.to_string()),
            FeatureName::Cpu => output.push(status_bar.cpu.read().await.to_string()),
            FeatureName::Clock => output.push(status_bar.clock.read().await.to_string()),
            FeatureName::Ram => output.push(status_bar.ram.read().await.to_string()),
            FeatureName::Gpu => output.push(status_bar.gpu.read().await.to_string()),
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

/// Turn an unexpected worker return into a contextual failure.
async fn _run_feature(
    feature_name: FeatureName,
    mut feature: Box<dyn FeatureTrait + Send + Sync>,
) -> Result<(), String> {
    feature.pull().await;

    Err(format!(
        "Feature {} worker returned unexpectedly",
        feature_name.as_str(),
    ))
}

/// Add feature context to a completed or panicked task.
fn _worker_exit(
    feature_name: FeatureName,
    result: Result<Result<(), String>, tokio::task::JoinError>,
) -> Result<(), String> {
    match result {
        Ok(Ok(())) => Err(format!(
            "Feature {} worker completed unexpectedly",
            feature_name.as_str(),
        )),
        Ok(Err(err)) => Err(err),
        Err(err) => Err(format!(
            "Feature {} worker task failed: {err}",
            feature_name.as_str(),
        )),
    }
}

/// Start workers and the renderer.
#[tokio::main]
async fn main() -> ExitCode {
    match _run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}

/// Keep startup failures on one nonzero exit path.
async fn _run() -> Result<(), String> {
    let config = _load_config()?;

    let status_bar = Arc::new(StatusBar::new());
    let mut resources: Vec<(FeatureName, Box<dyn FeatureTrait + Send + Sync>)> = vec![];

    for feature in &config.features {
        match feature {
            FeatureName::Connectivity => {
                let mut connectivity = Connectivity::new(status_bar.clone());
                connectivity.set_config(config.connectivity.clone());
                resources.push((*feature, Box::new(connectivity)));
            }
            FeatureName::Traffic => {
                let mut traffic = Traffic::new(status_bar.clone());
                traffic.set_config(config.traffic.clone());
                resources.push((*feature, Box::new(traffic)));
            }
            FeatureName::Cpu => {
                let mut cpu = Cpu::new(status_bar.clone());
                cpu.set_config(config.cpu.clone());
                resources.push((*feature, Box::new(cpu)));
            }
            FeatureName::Clock => {
                let mut clock = Clock::new(status_bar.clone());
                clock.set_config(config.clock.clone())?;
                resources.push((*feature, Box::new(clock)));
            }
            FeatureName::Ram => {
                let mut ram = Ram::new(status_bar.clone());
                ram.set_config(config.ram.clone());
                resources.push((*feature, Box::new(ram)));
            }
            FeatureName::Gpu => {
                let mut gpu = Gpu::new(status_bar.clone());
                gpu.set_config(config.gpu.clone());
                resources.push((*feature, Box::new(gpu)));
            }
        };
    }

    let root_name_writer = RootNameWriter::connect()?;

    let mut workers = FuturesUnordered::new();

    for (feature_name, resource) in resources {
        let worker = tokio::spawn(_run_feature(feature_name, resource));

        workers.push(async move { (feature_name, worker.await) });
    }

    status_bar.redraw.notify_one();
    let mut last_output = String::new();

    loop {
        tokio::select! {
            _ = status_bar.redraw.notified() => {
                let output = _build_output(status_bar.as_ref(), &config).await;

                if output == last_output {
                    continue;
                }

                root_name_writer.set_status(&output)?;
                last_output = output;
            }
            worker = workers.next() => {
                let Some((feature_name, result)) = worker else {
                    return Err("All feature workers stopped unexpectedly".to_string());
                };

                _worker_exit(feature_name, result)?;
            }
        }
    }

    #[allow(unreachable_code)]
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        _build_output, _candidate_config_paths_from, _parse_cli_args, _run_feature, _worker_exit,
    };
    use crate::config::{Config, FeatureName};
    use crate::features::FeatureTrait;
    use crate::status_bar::StatusBar;
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

    /// Preserve config order and the established reverse rendering pass.
    #[tokio::test]
    async fn render_features_in_reverse_config_order() {
        let config = toml::from_str::<Config>(
            r#"features = ["clock", "cpu"]"#,
        ).unwrap();
        let status_bar = StatusBar::new();

        *status_bar.clock.write().await = "clock".to_string();
        *status_bar.cpu.write().await = "cpu".to_string();

        assert_eq!(_build_output(&status_bar, &config).await, "▏cpu▕▏clock▕");
    }

    /// Treat a completed infinite worker as a fatal runtime error.
    #[tokio::test]
    async fn reject_unexpected_worker_return() {
        let status_bar = std::sync::Arc::new(StatusBar::new());
        let worker = ReturningFeature::new(status_bar);
        let error = _run_feature(FeatureName::Cpu, Box::new(worker))
            .await
            .unwrap_err();

        assert_eq!(error, "Feature cpu worker returned unexpectedly");
    }

    /// Attach the feature name to a panicked worker task.
    #[tokio::test]
    async fn reject_worker_panic() {
        let status_bar = std::sync::Arc::new(StatusBar::new());
        let worker = PanickingFeature::new(status_bar);
        let task = tokio::spawn(_run_feature(FeatureName::Gpu, Box::new(worker)));
        let error = _worker_exit(FeatureName::Gpu, task.await).unwrap_err();

        assert!(error.contains("Feature gpu worker task failed"));
        assert!(error.contains("panicked"));
    }

    struct ReturningFeature;

    #[async_trait::async_trait]
    impl FeatureTrait for ReturningFeature {
        /// Construct a worker that returns immediately.
        fn new(_status_bar: std::sync::Arc<StatusBar>) -> Self {
            Self
        }

        /// Simulate an unexpected clean worker return.
        async fn pull(&mut self) {}
    }

    struct PanickingFeature;

    #[async_trait::async_trait]
    impl FeatureTrait for PanickingFeature {
        /// Construct a worker that panics when polled.
        fn new(_status_bar: std::sync::Arc<StatusBar>) -> Self {
            Self
        }

        /// Simulate an unexpected worker panic.
        async fn pull(&mut self) {
            panic!("injected worker panic");
        }
    }
}

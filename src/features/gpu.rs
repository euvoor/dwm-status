use crate::config::GpuConfig;
use crate::FeatureTrait;
use crate::StatusBar;
use std::io::prelude::*;
use std::process::{Command, Stdio};
use std::sync::Arc;
use tokio::fs::read_to_string;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};

pub struct Gpu {
    status_bar: Arc<StatusBar>,
    config: GpuConfig,
}

#[async_trait::async_trait]
impl FeatureTrait for Gpu {
    /// Default GPU state.
    fn new(status_bar: Arc<StatusBar>) -> Self {
        Self {
            status_bar,
            config: GpuConfig::default(),
        }
    }

    /// Refresh GPU telemetry and fan state.
    async fn pull(&mut self) {
        let mut interval = interval(Duration::from_secs(self.config.idle));

        loop {
            let usage = self._usage();
            if usage.is_none() {
                break;
            }
            let usage = usage.unwrap();

            let temp = self._temperature();
            if temp.is_none() {
                break;
            }
            let temp = temp.unwrap();

            self._adjuest_fan(temp);

            let fan_rpm = self._fan_rpm();
            if fan_rpm.is_none() {
                break;
            }
            let fan_rpm = fan_rpm.unwrap();

            let output = format!(
                "{}(U: {}%) (M: {}%) (T: +{}°C) (F-RPM: {})",
                self.config.glyph, usage.0, usage.1, temp, fan_rpm,
            );

            *self.status_bar.gpu.write().await = output;
            self.status_bar.redraw.notify_one();

            interval.tick().await;
        }
    }
}

impl Gpu {
    /// Swap feature settings.
    pub fn set_config(&mut self, config: GpuConfig) {
        self.config = config;
    }

    /// Push the fan target for the current temperature.
    fn _adjuest_fan(&mut self, temp: f64) {
        let mut fan = "GPUTargetFanSpeed=0";

        if temp < 35. {
            fan = "GPUTargetFanSpeed=0";
        }
        if (45. ..55.).contains(&temp) {
            fan = "GPUTargetFanSpeed=25";
        }
        if (55. ..65.).contains(&temp) {
            fan = "GPUTargetFanSpeed=50";
        }
        if (65. ..75.).contains(&temp) {
            fan = "GPUTargetFanSpeed=75";
        }
        if temp >= 75. {
            fan = "GPUTargetFanSpeed=100";
        }

        Command::new("nvidia-settings")
            .args(&["-a", "GPUFanControlState=1", "-a", fan])
            .output()
            .unwrap();
    }

    /// Read the reported fan rpm.
    fn _fan_rpm(&self) -> Option<usize> {
        match Command::new("nvidia-settings")
            .args(&["-q", "GPUCurrentFanSpeedRPM"])
            .output()
        {
            Ok(nvidia_settings) => {
                let output = String::from_utf8(nvidia_settings.stdout).unwrap();

                Some(
                    output
                        .trim()
                        .split('\n')
                        .take(1)
                        .collect::<Vec<&str>>()
                        .join("")
                        .split_whitespace()
                        .last()
                        .unwrap_or("0")
                        .parse::<f64>()
                        .unwrap() as usize,
                )
            }
            Err(_) => {
                eprintln!("'nvidia-settings' command not found!");
                None
            }
        }
    }

    /// Read the current GPU temperature.
    fn _temperature(&self) -> Option<f64> {
        match Command::new("nvidia-smi")
            .arg("-q")
            .arg("-d")
            .arg("TEMPERATURE")
            .output()
        {
            Ok(nvidia_smi) => {
                let output = String::from_utf8(nvidia_smi.stdout).unwrap();
                let mut temp = 0.0;

                output.split('\n').for_each(|line| {
                    let line = line.trim();

                    if line.contains("GPU Current Temp") {
                        temp = self._parse_value_fn(line, 4);
                    }
                });

                Some(temp)
            }
            Err(_) => {
                eprintln!("'nvidia-smi' command not found!");
                None
            }
        }
    }

    /// Read utilization from nvidia-smi.
    fn _usage(&self) -> Option<(f64, f64)> {
        match Command::new("nvidia-smi")
            .arg("-q")
            .arg("-d")
            .arg("UTILIZATION")
            .output()
        {
            Ok(nvidia_smi) => {
                let output = String::from_utf8(nvidia_smi.stdout).unwrap();
                let mut gpu: f64 = 0.0;
                let mut memory: f64 = 0.0;

                output.split('\n').for_each(|line| {
                    let line = line.trim();

                    if line.starts_with("Gpu                               :") {
                        gpu = self._parse_value_fn(line, 2);
                    } else if line.starts_with("Memory                            :") {
                        memory = self._parse_value_fn(line, 2);
                    }
                });

                Some((gpu, memory))
            }
            Err(_) => {
                eprintln!("'nvidia-smi' command not found!");
                None
            }
        }
    }

    /// Parse a numeric field from command output.
    fn _parse_value_fn(&self, line: &str, skip: usize) -> f64 {
        let value: Vec<f64> = line
            .split_whitespace()
            .skip(skip)
            .take(1)
            .map(|value| value.parse::<f64>().unwrap())
            .collect();

        value[0]
    }
}

use crate::config::MemoryConfig;
use crate::FeatureTrait;
use crate::StatusBar;
use byte_unit::{Byte, UnitType};
use std::sync::Arc;
use tokio::fs::read_to_string;
use tokio::time::{sleep, Duration};

pub struct Memory {
    status_bar: Arc<StatusBar>,
    config: MemoryConfig,
}

#[async_trait::async_trait]
impl FeatureTrait for Memory {
    /// Default memory state.
    fn new(status_bar: Arc<StatusBar>) -> Self {
        Self {
            status_bar,
            config: MemoryConfig::default(),
        }
    }

    /// Refresh memory usage.
    async fn pull(&mut self) {
        loop {
            let _parse_number_fn = |line: &str| -> u128 {
                let mut line = line.split(':');
                line.next().unwrap();

                Byte::parse_str(line.next().unwrap(), true)
                    .unwrap()
                    .as_u128()
            };

            let format_bytes = |bytes: u128| -> String {
                format!(
                    "{:#.1}",
                    Byte::from_u128(bytes)
                        .unwrap()
                        .get_appropriate_unit(UnitType::Binary)
                )
            };

            let mut memtotal = 0;
            let mut memfree = 0;
            let mut memavailable = 0;
            let mut buffers = 0;
            let mut cached = 0;

            read_to_string("/proc/meminfo")
                .await
                .unwrap()
                .split('\n')
                .for_each(|line| {
                    if line.starts_with("MemTotal:") {
                        memtotal = _parse_number_fn(line);
                    }
                    if line.starts_with("MemFree:") {
                        memfree = _parse_number_fn(line);
                    }
                    if line.starts_with("MemAvailable:") {
                        memavailable = _parse_number_fn(line);
                    }
                    if line.starts_with("Buffers:") {
                        buffers = _parse_number_fn(line);
                    }
                    if line.starts_with("Cached:") {
                        cached = _parse_number_fn(line);
                    }
                });

            let buff_cache = buffers + cached;
            let used = memtotal - memfree - buff_cache;
            let output = match self.config.output.as_str() {
                "used" => format!("{}{}", self.config.prefix, format_bytes(used)),
                "free" => format!("{}{}", self.config.prefix, format_bytes(memavailable)),
                _ => format!(
                    "{}{:.1}%",
                    self.config.prefix,
                    (used as f64 / memtotal as f64) * 100.0
                ),
            };

            *self.status_bar.memory.write().await = output;
            self.status_bar.redraw.notify_one();

            sleep(Duration::from_secs(self.config.idle)).await;
        }
    }
}

impl Memory {
    /// Swap feature settings.
    pub fn set_config(&mut self, config: MemoryConfig) {
        self.config = config;
    }
}

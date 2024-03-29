use std::sync::Arc;
use crate::FeatureTrait;
use tokio::sync::{ Mutex, mpsc };
use std::io::prelude::*;
use tokio::fs::read_to_string;
use byte_unit::Byte;
use tokio::time::{ sleep, Duration };
use crate::StatusBar;
use crate::config::MemoryConfig;

pub struct Memory {
    status_bar: Arc<StatusBar>,
    config: MemoryConfig,
}

#[async_trait::async_trait]
impl FeatureTrait for Memory {
    fn new(status_bar: Arc<StatusBar>) -> Self {
        Self {
            status_bar,
            config: MemoryConfig::default(),
        }
    }

    async fn pull(&mut self) {
        loop {
            let _parse_number_fn = |line: &str| -> u128 {
                let mut line = line.split(':');
                line.next().unwrap();

                Byte::from_str(line.next().unwrap()).unwrap().get_bytes()
            };

            let mut memtotal = 0;
            let mut memfree = 0;
            let mut memavailable = 0;
            let mut buffers = 0;
            let mut cached = 0;
            let mut swaptotal = 0;

            read_to_string("/proc/meminfo").await.unwrap()
                .split('\n')
                .for_each(|line| {
                    if line.starts_with("MemTotal:") { memtotal = _parse_number_fn(line); }
                    if line.starts_with("MemFree:") { memfree = _parse_number_fn(line); }
                    if line.starts_with("MemAvailable:") { memavailable = _parse_number_fn(line); }
                    if line.starts_with("Buffers:") { buffers = _parse_number_fn(line); }
                    if line.starts_with("Cached:") { cached = _parse_number_fn(line); }
                    if line.starts_with("SwapTotal:") { swaptotal = _parse_number_fn(line); }
                });

            let buff_cache = buffers + cached;
            let used = memtotal - memfree - buff_cache;
            let output = format!("{}{:.1}%", self.config.prefix, (used as f64 / memtotal as f64) * 100.0);

            *self.status_bar.memory.write().await = output;

            sleep(Duration::from_secs(self.config.idle)).await;
        }
    }
}

impl Memory {
    pub fn set_config(&mut self, config: MemoryConfig) {
        self.config = config;
    }
}

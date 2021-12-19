use std::sync::Arc;
use crate::FeatureTrait;
use tokio::sync::{ Mutex, mpsc };
use std::io::prelude::*;
use tokio::fs::read_to_string;
use byte_unit::Byte;
use tokio::time::{ sleep, Duration };

pub struct Memory {
    prefix: &'static str,
    position: u8,
    tx: mpsc::Sender<(u8, String)>,
}

#[async_trait::async_trait]
impl FeatureTrait for Memory {
    fn new(position: u8, prefix: &'static str, tx: mpsc::Sender<(u8, String)>) -> Self {
        Self { prefix, position, tx }
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
            let used = Byte::from_bytes(memtotal - memfree - buff_cache).get_appropriate_unit(true);
            let output = format!("{}(U: {})", self.prefix, used);

            self.tx.send((self.position, output)).await.unwrap();

            sleep(Duration::from_secs(10)).await;
        }
    }
}

use std::sync::Arc;
use crate::FeatureTrait;
use tokio::sync::{ mpsc, Mutex };
use tokio::time::{ sleep, Duration };
use chrono::offset::Utc;

pub struct DateTime {
    prefix: &'static str,
    position: u8,
    tx: mpsc::Sender<(u8, String)>,
}

#[async_trait::async_trait]
impl FeatureTrait for DateTime {
    fn new(position: u8, prefix: &'static str, tx: mpsc::Sender<(u8, String)>) -> Self {
        Self { prefix, position, tx }
    }

    async fn pull(&mut self) {
        loop {
            let date_time = Utc::now();
            let output = format!("{}{}", self.prefix, date_time.format("%e %b %X"));

            self.tx.send((self.position, output)).await.unwrap();

            sleep(Duration::from_secs(1)).await;
        }
    }
}

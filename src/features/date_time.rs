use std::sync::Arc;
use crate::FeatureTrait;
use tokio::sync::{ mpsc, Mutex };
use tokio::time::{ sleep, Duration };
use chrono::offset::Utc;
use crate::StatusBar;

pub struct DateTime {
    status_bar: Arc<StatusBar>,
    prefix: &'static str,
    idle: Duration,
}

#[async_trait::async_trait]
impl FeatureTrait for DateTime {
    fn new(
        status_bar: Arc<StatusBar>,
        prefix: &'static str,
        idle: Duration,
    ) -> Self {
        Self {
            status_bar,
            prefix,
            idle,
        }
    }

    async fn pull(&mut self) {
        loop {
            let date_time = Utc::now();
            let output = format!("{}{}", self.prefix, date_time.format("%a %e %b %X").to_string().trim());

            *self.status_bar.date_time.write().await = output;

            sleep(self.idle).await;
        }
    }
}

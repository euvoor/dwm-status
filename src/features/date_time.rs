use crate::config::DateTimeConfig;
use crate::FeatureTrait;
use crate::StatusBar;
use chrono::offset::Utc;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio::time::{sleep, Duration};

pub struct DateTime {
    status_bar: Arc<StatusBar>,
    config: DateTimeConfig,
}

#[async_trait::async_trait]
impl FeatureTrait for DateTime {
    fn new(status_bar: Arc<StatusBar>) -> Self {
        Self {
            status_bar,
            config: DateTimeConfig::default(),
        }
    }

    async fn pull(&mut self) {
        loop {
            let date_time = Utc::now();
            let output = format!(
                "{}{}",
                self.config.prefix,
                date_time
                    .format(self.config.format.as_str())
                    .to_string()
                    .trim()
            );

            *self.status_bar.date_time.write().await = output;

            sleep(Duration::from_secs(self.config.idle)).await;
        }
    }
}

impl DateTime {
    pub fn set_config(&mut self, config: DateTimeConfig) {
        self.config = config;
    }
}

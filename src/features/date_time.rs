use crate::config::DateTimeConfig;
use crate::FeatureTrait;
use crate::StatusBar;
use chrono::offset::Utc;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio::time::{interval, Duration};

pub struct DateTime {
    status_bar: Arc<StatusBar>,
    config: DateTimeConfig,
}

#[async_trait::async_trait]
impl FeatureTrait for DateTime {
    /// Default clock state.
    fn new(status_bar: Arc<StatusBar>) -> Self {
        Self {
            status_bar,
            config: DateTimeConfig::default(),
        }
    }

    /// Refresh the wall clock.
    async fn pull(&mut self) {
        let mut interval = interval(Duration::from_secs(self.config.idle));

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
            self.status_bar.redraw.notify_one();

            interval.tick().await;
        }
    }
}

impl DateTime {
    /// Swap feature settings.
    pub fn set_config(&mut self, config: DateTimeConfig) {
        self.config = config;
    }
}

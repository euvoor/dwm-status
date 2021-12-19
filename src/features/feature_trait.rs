use std::sync::Arc;
use tokio::sync::{ Mutex, mpsc };
use tokio::time::Duration;
use crate::StatusBar;

#[async_trait::async_trait]
pub trait FeatureTrait {
    fn new(
        status_bar: Arc<StatusBar>,
        prefix: &'static str,
        idle: Duration,
    ) -> Self where Self: Sized;

    async fn pull(&mut self);
}

use crate::StatusBar;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio::time::Duration;

#[async_trait::async_trait]
pub trait FeatureTrait {
    fn new(status_bar: Arc<StatusBar>) -> Self
    where
        Self: Sized;

    async fn pull(&mut self);
}

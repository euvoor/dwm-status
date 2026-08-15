use crate::StatusBar;
use std::sync::Arc;

#[async_trait::async_trait]
pub trait FeatureTrait {
    /// Seed a worker with its shared output slots.
    fn new(status_bar: Arc<StatusBar>) -> Self
    where
        Self: Sized;

    /// Publish samples until the worker stops.
    async fn pull(&mut self);
}

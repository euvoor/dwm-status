use std::sync::Arc;
use tokio::sync::{ Mutex, mpsc };

#[async_trait::async_trait]
pub trait FeatureTrait {
    fn new(position: u8, prefix: &'static str, tx: mpsc::Sender<(u8, String)>) -> Self where Self: Sized;
    async fn pull(&mut self);
}

use std::sync::Arc;
use crate::FeatureTrait;
use tokio::sync::{ Mutex, mpsc };

pub struct VpnStatus {
    prefix: &'static str,
    position: u8,
    tx: mpsc::Sender<(u8, String)>,
}

#[async_trait::async_trait]
impl FeatureTrait for VpnStatus {
    fn new(position: u8, prefix: &'static str, tx: mpsc::Sender<(u8, String)>) -> Self {
        Self { prefix, position, tx }
    }

    async fn pull(&mut self) {
        println!("VPN Status: _");
    }
}

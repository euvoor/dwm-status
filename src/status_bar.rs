use tokio::sync::RwLock;

#[derive(Default, Debug)]
pub struct StatusBar {
    pub cpu: RwLock<String>,
    pub date_time: RwLock<String>,
    pub gpu: RwLock<String>,
    pub memory: RwLock<String>,
    pub ping: RwLock<String>,
    pub vpn: RwLock<String>,
    pub net_stats: RwLock<String>,
}

impl StatusBar {
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }
}

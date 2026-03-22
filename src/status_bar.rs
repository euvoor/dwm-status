use tokio::sync::{Notify, RwLock};

#[derive(Default, Debug)]
pub struct StatusBar {
    pub redraw: Notify,
    pub connectivity: RwLock<String>,
    pub clock: RwLock<String>,
    pub cpu: RwLock<String>,
    pub gpu: RwLock<String>,
    pub ram: RwLock<String>,
    pub traffic: RwLock<String>,
}

impl StatusBar {
    /// Shared status slots.
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }
}

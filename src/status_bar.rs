#![allow(dead_code)]

use std::sync::Arc;
use tokio::sync::RwLock;

pub struct StatusBar {
    pub blocks: Arc<RwLock<Vec<String>>>,
}

impl StatusBar {
    pub fn new(nb_blocks: u8) -> Self {
        let blocks = vec![String::new(); nb_blocks.into()];

        Self {
            blocks: Arc::new(RwLock::new(blocks)),
        }
    }

    pub async fn update(&mut self, n: u8, data: String) {
        let blocks = self.blocks.clone();
        let mut blocks = blocks.write().await;
        let len = blocks.len();
        let pos = (len - 1) - (n as usize);

        blocks[pos] = data;
    }
}

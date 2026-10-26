use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use anyhow::Result;
use async_trait::async_trait;
use dashmap::DashMap;
use tokio::sync::mpsc;
use uzlinux_core::models::DeviceInfo;

use crate::event::DeviceEvent;
use crate::traits::StreamDeckDevice;

pub struct VirtualDeck {
    info: DeviceInfo,
    brightness: AtomicU8,
    key_images: Arc<DashMap<u8, Vec<u8>>>,
    lcd_strip_image: Arc<DashMap<u8, Vec<u8>>>,
    event_tx: mpsc::UnboundedSender<(String, DeviceEvent)>,
}

impl VirtualDeck {
    pub fn new(
        info: DeviceInfo,
        event_tx: mpsc::UnboundedSender<(String, DeviceEvent)>,
    ) -> Self {
        Self {
            info,
            brightness: AtomicU8::new(100),
            key_images: Arc::new(DashMap::new()),
            lcd_strip_image: Arc::new(DashMap::new()),
            event_tx,
        }
    }

    pub fn trigger_key_down(&self, key_index: u8) -> Result<()> {
        let cols = self.info.columns;
        let row = key_index / cols;
        let col = key_index % cols;
        self.event_tx.send((
            self.info.id.clone(),
            DeviceEvent::KeyDown {
                row,
                column: col,
                key_index,
            },
        ))?;
        Ok(())
    }

    pub fn trigger_key_up(&self, key_index: u8) -> Result<()> {
        let cols = self.info.columns;
        let row = key_index / cols;
        let col = key_index % cols;
        self.event_tx.send((
            self.info.id.clone(),
            DeviceEvent::KeyUp {
                row,
                column: col,
                key_index,
            },
        ))?;
        Ok(())
    }

    pub fn get_key_image(&self, key_index: u8) -> Option<Vec<u8>> {
        self.key_images.get(&key_index).map(|r| r.value().clone())
    }

    pub fn get_brightness(&self) -> u8 {
        self.brightness.load(Ordering::Relaxed)
    }
}

#[async_trait]
impl StreamDeckDevice for VirtualDeck {
    fn id(&self) -> &str {
        &self.info.id
    }

    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    async fn set_brightness(&self, percentage: u8) -> Result<()> {
        self.brightness.store(percentage, Ordering::Relaxed);
        Ok(())
    }

    async fn set_key_image(&self, key_index: u8, image_bytes: &[u8]) -> Result<()> {
        self.key_images.insert(key_index, image_bytes.to_vec());
        Ok(())
    }

    async fn set_lcd_strip(&self, image_bytes: &[u8]) -> Result<()> {
        self.lcd_strip_image.insert(0, image_bytes.to_vec());
        Ok(())
    }

    async fn clear_all_keys(&self) -> Result<()> {
        self.key_images.clear();
        Ok(())
    }
}

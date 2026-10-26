use std::sync::Arc;
use dashmap::DashMap;
use log::info;
use tokio::sync::mpsc;
use uzlinux_core::models::DeviceInfo;

use crate::event::DeviceEvent;
use crate::traits::StreamDeckDevice;
use crate::virtual_deck::VirtualDeck;

pub struct DeviceManager {
    devices: Arc<DashMap<String, Arc<dyn StreamDeckDevice>>>,
    event_tx: mpsc::UnboundedSender<(String, DeviceEvent)>,
}

impl DeviceManager {
    pub fn new() -> (Self, mpsc::UnboundedReceiver<(String, DeviceEvent)>) {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let manager = Self {
            devices: Arc::new(DashMap::new()),
            event_tx,
        };
        (manager, event_rx)
    }

    pub fn event_sender(&self) -> mpsc::UnboundedSender<(String, DeviceEvent)> {
        self.event_tx.clone()
    }

    pub fn register_device(&self, device: Arc<dyn StreamDeckDevice>) {
        let id = device.id().to_string();
        info!("Registered device: {}", id);
        self.devices.insert(id, device);
    }

    pub fn get_device(&self, id: &str) -> Option<Arc<dyn StreamDeckDevice>> {
        self.devices.get(id).map(|r| r.value().clone())
    }

    pub fn list_device_ids(&self) -> Vec<String> {
        self.devices.iter().map(|kv| kv.key().clone()).collect()
    }

    pub fn add_virtual_deck(&self, info: DeviceInfo) -> Arc<VirtualDeck> {
        let id = info.id.clone();
        let deck = Arc::new(VirtualDeck::new(info, self.event_tx.clone()));
        self.devices.insert(id, deck.clone());
        deck
    }

    pub async fn auto_detect_devices(&self) -> bool {
        match crate::ulanzi::UlanziD200Device::find_and_connect(self.event_tx.clone()).await {
            Ok(Some(dev)) => {
                let id = dev.id().to_string();
                info!("Auto-detected native physical Ulanzi D200 hardware: '{}'", id);
                self.devices.insert(id, dev);
                true
            }
            Ok(None) => false,
            Err(e) => {
                log::warn!("Error connecting to physical Ulanzi D200: {}", e);
                false
            }
        }
    }
}

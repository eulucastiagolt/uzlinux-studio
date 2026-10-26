use anyhow::Result;
use async_trait::async_trait;
use uzlinux_core::models::DeviceInfo;

#[async_trait]
pub trait StreamDeckDevice: Send + Sync {
    fn id(&self) -> &str;
    fn info(&self) -> &DeviceInfo;
    fn is_hardware(&self) -> bool { false }
    async fn set_brightness(&self, percentage: u8) -> Result<()>;
    async fn set_key_image(&self, key_index: u8, image_bytes: &[u8]) -> Result<()>;
    async fn set_lcd_strip(&self, image_bytes: &[u8]) -> Result<()>;
    async fn clear_all_keys(&self) -> Result<()>;
}

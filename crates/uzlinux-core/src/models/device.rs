use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceType {
    StreamDeckOriginal,
    StreamDeckOriginalV2,
    StreamDeckMini,
    StreamDeckXl,
    StreamDeckPlus,
    StreamDeckPedal,
    StreamDeckNeo,
    UlanziD200,
    VirtualDeck,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub device_type: DeviceType,
    pub rows: u8,
    pub columns: u8,
    pub key_count: u8,
    pub key_pixel_size: (u32, u32),
    pub has_encoders: bool,
    pub has_lcd_strip: bool,
    pub lcd_strip_size: Option<(u32, u32)>,
    pub serial_number: Option<String>,
    pub firmware_version: Option<String>,
}

impl DeviceInfo {
    pub fn new_ulanzi_d200(serial: &str) -> Self {
        Self {
            id: format!("e9-{}", serial),
            name: "Ulanzi Stream Controller D200".to_string(),
            device_type: DeviceType::UlanziD200,
            rows: 3,
            columns: 5,
            key_count: 14,
            key_pixel_size: (72, 72),
            has_encoders: false,
            has_lcd_strip: true,
            lcd_strip_size: Some((800, 100)),
            serial_number: Some(serial.to_string()),
            firmware_version: None,
        }
    }

    pub fn new_virtual(id: &str, rows: u8, columns: u8) -> Self {
        Self {
            id: id.to_string(),
            name: format!("Virtual Deck ({}x{})", rows, columns),
            device_type: DeviceType::VirtualDeck,
            rows,
            columns,
            key_count: rows * columns,
            key_pixel_size: (72, 72),
            has_encoders: false,
            has_lcd_strip: false,
            lcd_strip_size: None,
            serial_number: Some("VIRTUAL001".to_string()),
            firmware_version: Some("1.0.0".to_string()),
        }
    }
}

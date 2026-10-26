use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DeviceEvent {
    KeyDown {
        row: u8,
        column: u8,
        key_index: u8,
    },
    KeyUp {
        row: u8,
        column: u8,
        key_index: u8,
    },
    DialRotate {
        dial_index: u8,
        ticks: i32,
        pressed: bool,
    },
    DialDown {
        dial_index: u8,
    },
    DialUp {
        dial_index: u8,
    },
    TouchTap {
        x: u32,
        y: u32,
    },
}

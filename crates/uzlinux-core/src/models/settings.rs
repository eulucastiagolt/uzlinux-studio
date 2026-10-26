use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppSettings {
    #[serde(default = "default_autolaunch")]
    pub autolaunch: bool,

    #[serde(default = "default_true")]
    pub background: bool,

    #[serde(default = "default_brightness")]
    pub brightness: u8,

    #[serde(default)]
    pub developer: bool,

    #[serde(default)]
    pub disableelgato: bool,

    #[serde(default = "default_language")]
    pub language: String,

    #[serde(default)]
    pub rotation: u16,

    #[serde(default)]
    pub separatewine: bool,

    #[serde(default = "default_sleep_timeout")]
    pub sleep_timeout_minutes: u32,

    #[serde(default = "default_true")]
    pub sleep_when_computer_locked: bool,

    #[serde(default = "default_true")]
    pub statistics: bool,

    #[serde(default = "default_true")]
    pub updatecheck: bool,

    #[serde(default = "default_version")]
    pub version: String,

    #[serde(default)]
    pub disabled_plugins: Vec<String>,
}

fn default_autolaunch() -> bool { false }
fn default_true() -> bool { true }
fn default_brightness() -> u8 { 80 }
fn default_language() -> String { "en_US".to_string() }
fn default_sleep_timeout() -> u32 { 30 }
fn default_version() -> String { "2.14.0".to_string() }

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            autolaunch: false,
            background: true,
            brightness: 80,
            developer: false,
            disableelgato: false,
            language: "en_US".to_string(),
            rotation: 0,
            separatewine: false,
            sleep_timeout_minutes: 30,
            sleep_when_computer_locked: true,
            statistics: true,
            updatecheck: true,
            version: "2.14.0".to_string(),
            disabled_plugins: Vec::new(),
        }
    }
}

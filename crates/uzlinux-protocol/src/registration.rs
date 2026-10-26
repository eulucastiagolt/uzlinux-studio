use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RegistrationMessage {
    pub event: String,
    #[serde(alias = "context")]
    pub uuid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PluginInfo {
    pub application: ApplicationInfo,
    pub plugin: PluginMeta,
    #[serde(default = "default_dpr")]
    pub device_pixel_ratio: u32,
    #[serde(default)]
    pub devices: Vec<DeviceRegistrationInfo>,
    #[serde(default)]
    pub colors: ColorsInfo,
}

fn default_dpr() -> u32 { 1 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationInfo {
    #[serde(default = "default_font")]
    pub font: String,
    #[serde(default = "default_lang")]
    pub language: String,
    #[serde(default = "default_platform")]
    pub platform: String,
    #[serde(default)]
    pub platform_version: String,
    pub version: String,
}

fn default_font() -> String { "Liberation Sans".to_string() }
fn default_lang() -> String { "pt_BR".to_string() }
fn default_platform() -> String { "linux".to_string() }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PluginMeta {
    pub uuid: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceRegistrationInfo {
    pub id: String,
    pub name: String,
    pub size: DeviceDimensions,
    #[serde(rename = "type")]
    pub device_type: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeviceDimensions {
    pub columns: u8,
    pub rows: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ColorsInfo {
    #[serde(default = "default_btn_press")]
    pub button_pressed_background_color: String,
    #[serde(default = "default_btn_border")]
    pub button_pressed_border_color: String,
    #[serde(default = "default_btn_text")]
    pub button_pressed_text_color: String,
    #[serde(default = "default_highlight")]
    pub highlight_color: String,
}

fn default_btn_press() -> String { "#303030FF".to_string() }
fn default_btn_border() -> String { "#606060FF".to_string() }
fn default_btn_text() -> String { "#FFFFFFFF".to_string() }
fn default_highlight() -> String { "#0078D7FF".to_string() }

impl Default for ColorsInfo {
    fn default() -> Self {
        Self {
            button_pressed_background_color: default_btn_press(),
            button_pressed_border_color: default_btn_border(),
            button_pressed_text_color: default_btn_text(),
            highlight_color: default_highlight(),
        }
    }
}

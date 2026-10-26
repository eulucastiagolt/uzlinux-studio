use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "event", rename_all = "camelCase")]
pub enum InboundEvent {
    SetSettings(SetSettingsMessage),
    GetSettings(ContextOnlyMessage),
    SetGlobalSettings(SetGlobalSettingsMessage),
    GetGlobalSettings(ContextOnlyMessage),
    OpenUrl(OpenUrlMessage),
    LogMessage(LogMessagePayload),
    SetTitle(SetTitleMessage),
    SetImage(SetImageMessage),
    ShowAlert(ContextOnlyMessage),
    ShowOk(ContextOnlyMessage),
    SetState(SetStateMessage),
    SwitchToProfile(SwitchToProfileMessage),
    SendToPropertyInspector(SendToPropertyInspectorMessage),
    SendToPlugin(SendToPluginMessage),
    SetFeedback(SetFeedbackMessage),
    SetFeedbackLayout(SetFeedbackLayoutMessage),

    // Device extensions (OpenDeck / Ulanzi D200)
    #[serde(rename = "device_registered")]
    DeviceRegistered(DeviceRegisteredMessage),

    #[serde(rename = "device_plugin_set_image")]
    DevicePluginSetImage(DevicePluginSetImageMessage),

    #[serde(rename = "device_plugin_set_brightness")]
    DevicePluginSetBrightness(DevicePluginSetBrightnessMessage),

    #[serde(rename = "registerDevice")]
    RegisterDevice(RegisterDeviceMessage),

    #[serde(rename = "keyDown")]
    DeviceKeyDown(DeviceKeyPressMessage),

    #[serde(rename = "keyUp")]
    DeviceKeyUp(DeviceKeyPressMessage),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeviceKeyPressMessage {
    pub payload: DeviceKeyPressPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeviceKeyPressPayload {
    pub device: String,
    pub position: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RegisterDeviceMessage {
    pub payload: RegisterDevicePayload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RegisterDevicePayload {
    pub id: String,
    pub name: String,
    pub rows: u8,
    pub columns: u8,
    #[serde(default)]
    pub encoders: Option<u8>,
    #[serde(default, rename = "type")]
    pub device_type: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextOnlyMessage {
    #[serde(default)]
    pub context: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SetSettingsMessage {
    #[serde(default)]
    pub context: String,
    #[serde(default)]
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SetGlobalSettingsMessage {
    #[serde(default)]
    pub context: String,
    #[serde(default)]
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OpenUrlMessage {
    pub payload: OpenUrlPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OpenUrlPayload {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LogMessagePayload {
    pub payload: LogMessageInner,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LogMessageInner {
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SetTitleMessage {
    pub context: String,
    pub payload: SetTitlePayload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SetTitlePayload {
    pub title: String,
    #[serde(default)]
    pub target: Option<u8>,
    #[serde(default)]
    pub state: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SetImageMessage {
    pub context: String,
    pub payload: SetImagePayload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SetImagePayload {
    pub image: String,
    #[serde(default)]
    pub target: Option<u8>,
    #[serde(default)]
    pub state: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SetStateMessage {
    pub context: String,
    pub payload: SetStatePayload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SetStatePayload {
    pub state: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SwitchToProfileMessage {
    pub context: String,
    pub device: String,
    pub payload: SwitchToProfilePayload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SwitchToProfilePayload {
    pub profile: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SendToPropertyInspectorMessage {
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub context: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SendToPluginMessage {
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub context: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SetFeedbackMessage {
    #[serde(default)]
    pub context: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SetFeedbackLayoutMessage {
    #[serde(default)]
    pub context: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeviceRegisteredMessage {
    pub device: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub size: Option<Vec<u8>>,
    #[serde(default, rename = "type")]
    pub device_type: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DevicePluginSetImageMessage {
    pub device: String,
    pub key: u8,
    pub image: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DevicePluginSetBrightnessMessage {
    pub device: String,
    pub value: u8,
}

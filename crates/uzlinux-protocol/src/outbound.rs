use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "event", rename_all = "camelCase")]
pub enum OutboundEvent {
    DidReceiveSettings(DidReceiveSettingsMessage),
    DidReceiveGlobalSettings(DidReceiveGlobalSettingsMessage),
    KeyDown(KeyEventMessage),
    KeyUp(KeyEventMessage),
    WillAppear(AppearanceMessage),
    WillDisappear(AppearanceMessage),
    TitleParametersDidChange(TitleParametersDidChangeMessage),
    DeviceDidConnect(DeviceConnectMessage),
    DeviceDidDisconnect(DeviceDisconnectMessage),
    ApplicationDidLaunch(ApplicationMessage),
    ApplicationDidTerminate(ApplicationMessage),
    SystemDidWakeUp,
    PropertyInspectorDidAppear(ContextActionMessage),
    PropertyInspectorDidDisappear(ContextActionMessage),
    SendToPlugin(SendToPluginOutboundMessage),
    SendToPropertyInspector(SendToPropertyInspectorOutboundMessage),
    DialRotate(DialRotateMessage),
    DialDown(DialDownMessage),
    DialUp(DialDownMessage),
    TouchTap(TouchTapMessage),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextActionMessage {
    pub action: String,
    pub context: String,
    pub device: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DidReceiveSettingsMessage {
    pub action: String,
    pub context: String,
    pub device: String,
    pub payload: SettingsPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPayload {
    pub settings: Value,
    pub coordinates: Coordinates,
    #[serde(default)]
    pub is_in_multi_action: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DidReceiveGlobalSettingsMessage {
    pub payload: GlobalSettingsPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GlobalSettingsPayload {
    pub settings: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Coordinates {
    pub column: u8,
    pub row: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KeyEventMessage {
    pub action: String,
    pub context: String,
    pub device: String,
    pub payload: KeyEventPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct KeyEventPayload {
    pub settings: Value,
    pub coordinates: Coordinates,
    pub state: usize,
    #[serde(default)]
    pub user_desired_state: usize,
    pub is_in_multi_action: bool,
    #[serde(default = "default_controller")]
    pub controller: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppearanceMessage {
    pub action: String,
    pub context: String,
    pub device: String,
    pub payload: AppearancePayload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppearancePayload {
    pub settings: Value,
    pub coordinates: Coordinates,
    pub state: usize,
    pub is_in_multi_action: bool,
    #[serde(default = "default_controller")]
    pub controller: String,
}

fn default_controller() -> String {
    "Keypad".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TitleParametersDidChangeMessage {
    pub action: String,
    pub context: String,
    pub device: String,
    pub payload: TitleParametersPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TitleParametersPayload {
    pub settings: Value,
    pub coordinates: Coordinates,
    pub state: usize,
    pub title: String,
    pub title_parameters: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceConnectMessage {
    pub device: String,
    pub device_info: DeviceConnectInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeviceConnectInfo {
    pub name: String,
    #[serde(rename = "type")]
    pub device_type: u8,
    pub size: DeviceConnectSize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeviceConnectSize {
    pub columns: u8,
    pub rows: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeviceDisconnectMessage {
    pub device: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApplicationMessage {
    pub payload: ApplicationPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApplicationPayload {
    pub application: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SendToPluginOutboundMessage {
    pub action: String,
    pub context: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SendToPropertyInspectorOutboundMessage {
    pub action: String,
    pub context: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DialRotateMessage {
    pub action: String,
    pub context: String,
    pub device: String,
    pub payload: DialRotatePayload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DialRotatePayload {
    pub settings: Value,
    pub coordinates: Coordinates,
    pub ticks: i32,
    pub pressed: bool,
    pub controller: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DialDownMessage {
    pub action: String,
    pub context: String,
    pub device: String,
    pub payload: DialDownPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DialDownPayload {
    pub settings: Value,
    pub coordinates: Coordinates,
    pub controller: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TouchTapMessage {
    pub action: String,
    pub context: String,
    pub device: String,
    pub payload: TouchTapPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TouchTapPayload {
    pub settings: Value,
    pub coordinates: Coordinates,
    pub tap_pos: [u32; 2],
    pub hold: bool,
}

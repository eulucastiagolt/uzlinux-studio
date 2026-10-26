use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProfileSelection {
    pub selected_profile: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Profile {
    #[serde(default)]
    pub infobars: Vec<serde_json::Value>,

    #[serde(default)]
    pub keys: Vec<Option<KeyInstance>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KeyInstance {
    pub action: ActionInstance,
    #[serde(default)]
    pub children: Option<Vec<KeyInstance>>,
    pub context: String,
    #[serde(default)]
    pub current_state: usize,
    #[serde(default)]
    pub settings: serde_json::Value,
    #[serde(default)]
    pub states: Vec<StateDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActionInstance {
    #[serde(default)]
    pub controllers: Vec<String>,
    #[serde(default)]
    pub disable_automatic_states: bool,
    #[serde(default)]
    pub encoder: Option<serde_json::Value>,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub plugin: String,
    #[serde(default)]
    pub property_inspector: String,
    #[serde(default)]
    pub states: Vec<StateDefinition>,
    #[serde(default = "default_true")]
    pub supported_in_multi_actions: bool,
    #[serde(default)]
    pub tooltip: String,
    pub uuid: String,
    #[serde(default = "default_true")]
    pub visible_in_action_list: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StateDefinition {
    #[serde(default = "default_alignment")]
    pub alignment: String,
    #[serde(default = "default_bg_colour")]
    pub background_colour: String,
    #[serde(default = "default_colour")]
    pub colour: String,
    #[serde(default = "default_family")]
    pub family: String,
    #[serde(default)]
    pub image: String,
    #[serde(default)]
    pub image_override: Option<String>,
    #[serde(default = "default_scale")]
    pub image_scale: u32,
    #[serde(default)]
    pub name: String,
    #[serde(default = "default_true")]
    pub show: bool,
    #[serde(default = "default_size")]
    pub size: u32,
    #[serde(default = "default_bg_colour")]
    pub stroke_colour: String,
    #[serde(default = "default_stroke_size")]
    pub stroke_size: u32,
    #[serde(default = "default_style")]
    pub style: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub text_override: Option<String>,
    #[serde(default)]
    pub underline: bool,
}

fn default_true() -> bool { true }
fn default_alignment() -> String { "middle".to_string() }
fn default_bg_colour() -> String { "#000000".to_string() }
fn default_colour() -> String { "#FFFFFF".to_string() }
fn default_family() -> String { "Liberation Sans".to_string() }
fn default_scale() -> u32 { 100 }
fn default_size() -> u32 { 16 }
fn default_stroke_size() -> u32 { 3 }
fn default_style() -> String { "Regular".to_string() }

impl Default for StateDefinition {
    fn default() -> Self {
        Self {
            alignment: default_alignment(),
            background_colour: default_bg_colour(),
            colour: default_colour(),
            family: default_family(),
            image: String::new(),
            image_override: None,
            image_scale: 100,
            name: String::new(),
            show: true,
            size: 16,
            stroke_colour: default_bg_colour(),
            stroke_size: 3,
            style: default_style(),
            text: String::new(),
            text_override: None,
            underline: false,
        }
    }
}

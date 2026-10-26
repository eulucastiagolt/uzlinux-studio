use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct PluginManifest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    pub version: String,
    #[serde(default)]
    pub code_paths: Option<HashMap<String, String>>,
    #[serde(default)]
    pub code_path_win: Option<String>,
    #[serde(default)]
    pub code_path_mac: Option<String>,
    #[serde(default)]
    pub code_path_lin: Option<String>,
    #[serde(default)]
    pub code_path: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub category_icon: Option<String>,
    #[serde(default)]
    pub os: Option<Vec<OSManifest>>,
    #[serde(default)]
    pub actions: Vec<ActionManifest>,
    #[serde(default, rename = "DeviceNamespace")]
    pub device_namespace: Option<String>,
    #[serde(default, rename = "PluginUUID")]
    pub plugin_uuid: Option<String>,
    #[serde(default)]
    pub property_inspector_path: Option<String>,
}

impl PluginManifest {
    /// Resolves the executable path relative to the plugin root on Linux.
    pub fn get_linux_executable(&self) -> Option<String> {
        if let Some(ref lin) = self.code_path_lin {
            return Some(lin.clone());
        }
        if let Some(ref map) = self.code_paths {
            // Check for current target architecture
            #[cfg(target_arch = "x86_64")]
            {
                if let Some(path) = map.get("x86_64-unknown-linux-gnu") {
                    return Some(path.clone());
                }
            }
            #[cfg(target_arch = "aarch64")]
            {
                if let Some(path) = map.get("aarch64-unknown-linux-gnu") {
                    return Some(path.clone());
                }
            }
            // Fallback to any linux path
            for (key, val) in map {
                if key.contains("linux") {
                    return Some(val.clone());
                }
            }
        }
        if let Some(ref generic) = self.code_path {
            return Some(generic.clone());
        }
        None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct OSManifest {
    pub platform: String,
    #[serde(default)]
    pub minimum_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct ActionManifest {
    #[serde(default)]
    pub icon: Option<String>,
    pub name: String,
    #[serde(default)]
    pub states: Vec<ActionStateManifest>,
    #[serde(default)]
    pub controllers: Vec<String>,
    #[serde(default = "default_true")]
    pub supported_in_multi_actions: bool,
    #[serde(default)]
    pub tooltip: Option<String>,
    #[serde(rename = "UUID")]
    pub uuid: String,
    #[serde(default)]
    pub property_inspector_path: Option<String>,
    #[serde(default)]
    pub user_title_enabled: Option<bool>,
    #[serde(default)]
    pub visible_in_action_list: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct ActionStateManifest {
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub font_family: Option<String>,
    #[serde(default)]
    pub font_size: Option<u32>,
    #[serde(default)]
    pub font_style: Option<String>,
    #[serde(default)]
    pub font_underline: Option<bool>,
    #[serde(default)]
    pub title_alignment: Option<String>,
    #[serde(default)]
    pub title_color: Option<String>,
    #[serde(default)]
    pub show_title: Option<bool>,
}

fn default_true() -> bool { true }

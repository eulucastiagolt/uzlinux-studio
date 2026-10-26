use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// Maps Application identifier (e.g., "com.obsproject.Studio", "vlc", "deezer-enhanced")
/// to a map of Device ID -> Profile Name.
pub type ApplicationProfileMap = HashMap<String, HashMap<String, String>>;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApplicationProfiles {
    #[serde(flatten)]
    pub map: ApplicationProfileMap,
}

impl ApplicationProfiles {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn get_profile_for_app(&self, app_id: &str, device_id: &str) -> Option<&str> {
        self.map
            .get(app_id)
            .and_then(|device_map| device_map.get(device_id))
            .map(|s| s.as_str())
    }

    pub fn set_profile_for_app(&mut self, app_id: String, device_id: String, profile_name: String) {
        self.map
            .entry(app_id)
            .or_default()
            .insert(device_id, profile_name);
    }

    pub fn remove_app(&mut self, app_id: &str) {
        self.map.remove(app_id);
    }

    pub fn remove_profile_for_device(&mut self, app_id: &str, device_id: &str) {
        if let Some(device_map) = self.map.get_mut(app_id) {
            device_map.remove(device_id);
            if device_map.is_empty() {
                self.map.remove(app_id);
            }
        }
    }
}

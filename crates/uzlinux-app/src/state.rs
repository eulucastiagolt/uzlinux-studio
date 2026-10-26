use std::sync::{Arc, RwLock};
use base64::Engine;
use dashmap::DashMap;
use log::info;
use tauri::{AppHandle, Emitter};
use uzlinux_core::models::Profile;
use uzlinux_core::storage::ConfigManager;
use uzlinux_device_driver::DeviceManager;
use uzlinux_plugin_manager::ProcessManager;
use uzlinux_renderer::ButtonComposer;
use uzlinux_server::{AssetServer, PluginHub, PropertyInspectorHub, ServerPorts};

pub struct AppState {
    pub config_manager: Arc<ConfigManager>,
    pub ports: ServerPorts,
    pub plugin_hub: PluginHub,
    pub pi_hub: PropertyInspectorHub,
    pub asset_server: AssetServer,
    pub device_manager: Arc<DeviceManager>,
    pub process_manager: Arc<ProcessManager>,
    pub composer: Arc<ButtonComposer>,
    // device_id -> (profile_name, Profile)
    pub active_profiles: Arc<DashMap<String, (String, Profile)>>,
    // context -> (device_id, key_index)
    pub context_to_key: Arc<DashMap<String, (String, u8)>>,
    pub app_handle: Arc<RwLock<Option<AppHandle>>>,
}

impl AppState {
    pub fn new(
        config_manager: ConfigManager,
        ports: ServerPorts,
        plugin_hub: PluginHub,
        pi_hub: PropertyInspectorHub,
        asset_server: AssetServer,
        device_manager: DeviceManager,
    ) -> Self {
        Self {
            config_manager: Arc::new(config_manager),
            ports,
            plugin_hub,
            pi_hub,
            asset_server,
            device_manager: Arc::new(device_manager),
            process_manager: Arc::new(ProcessManager::new()),
            composer: Arc::new(ButtonComposer::new(72, 72)),
            active_profiles: Arc::new(DashMap::new()),
            context_to_key: Arc::new(DashMap::new()),
            app_handle: Arc::new(RwLock::new(None)),
        }
    }

    pub fn set_app_handle(&self, handle: AppHandle) {
        if let Ok(mut lock) = self.app_handle.write() {
            *lock = Some(handle);
        }
    }

    pub fn emit_to_frontend<S: serde::Serialize + Clone>(&self, event: &str, payload: S) {
        if let Ok(lock) = self.app_handle.read() {
            if let Some(ref handle) = *lock {
                let _ = handle.emit(event, payload);
            }
        }
    }

    pub fn set_active_profile(&self, device_id: &str, profile_name: &str, profile: Profile) {
        info!("Setting active profile for device '{}' to '{}'", device_id, profile_name);

        // Remove old context mappings for this device
        self.context_to_key.retain(|_, (dev, _)| dev != device_id);

        // Update context mappings
        for (idx, key_opt) in profile.keys.iter().enumerate() {
            if let Some(key) = key_opt {
                self.context_to_key.insert(key.context.clone(), (device_id.to_string(), idx as u8));
                self.plugin_hub.register_context(key.context.clone(), key.action.plugin.clone());
            }
        }

        self.active_profiles.insert(
            device_id.to_string(),
            (profile_name.to_string(), profile),
        );
    }

    pub fn get_active_profile(&self, device_id: &str) -> Option<(String, Profile)> {
        self.active_profiles.get(device_id).map(|r| r.value().clone())
    }

    pub fn send_image_to_device(&self, device_id: &str, position: u8, png_bytes: &[u8]) {
        if device_id.starts_with("e9-") && position >= 14 {
            return;
        }
        let b64 = base64::engine::general_purpose::STANDARD.encode(png_bytes);
        let data_uri = format!("data:image/png;base64,{}", b64);
        let msg = serde_json::json!({
            "event": "setImage",
            "device": device_id,
            "position": position,
            "image": data_uri,
            "controller": "Keypad"
        });
        self.plugin_hub.broadcast_raw(&msg);
    }

    pub fn send_clear_to_device(&self, device_id: &str, position: u8) {
        if device_id.starts_with("e9-") && position >= 14 {
            return;
        }
        let empty_canvas = image::RgbaImage::new(72, 72);
        if let Ok(png_bytes) = ButtonComposer::to_png_bytes(&empty_canvas) {
            self.send_image_to_device(device_id, position, &png_bytes);
        }
    }

    pub fn send_brightness_to_device(&self, device_id: &str, brightness: u8) {
        let msg = serde_json::json!({
            "event": "setBrightness",
            "device": device_id,
            "brightness": brightness
        });
        self.plugin_hub.broadcast_raw(&msg);
    }

    pub async fn render_all_keys_for_device(&self, device_id: &str, profile: &Profile) {
        let max_device_keys = if device_id.starts_with("e9-") {
            14
        } else {
            15
        };

        if let Some(device) = self.device_manager.get_device(device_id) {
            let _ = device.clear_all_keys().await;
        }

        for idx in 0..max_device_keys {
            let key_opt = profile.keys.get(idx).and_then(|k| k.as_ref());
            if let Some(key) = key_opt {
                if let Some(state) = key.states.get(key.current_state) {
                    if let Ok(canvas) = self.composer.compose_state(
                        state,
                        Some(&self.config_manager.base_dir()),
                    ) {
                        if let Ok(png_bytes) = ButtonComposer::to_png_bytes(&canvas) {
                            if let Some(dev) = self.device_manager.get_device(device_id) {
                                let _ = dev.set_key_image(idx as u8, &png_bytes).await;
                            }
                            self.send_image_to_device(device_id, idx as u8, &png_bytes);
                            continue;
                        }
                    }
                }
            }
            // Empty key slot
            self.send_clear_to_device(device_id, idx as u8);
        }
    }
}

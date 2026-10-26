use std::sync::Arc;
use std::time::Duration;
use log::{debug, error, info, warn};
use tokio::sync::mpsc;

use uzlinux_core::models::Profile;
use uzlinux_device_driver::DeviceEvent;
use uzlinux_protocol::{
    AppearanceMessage, AppearancePayload, Coordinates, DidReceiveGlobalSettingsMessage,
    DidReceiveSettingsMessage, GlobalSettingsPayload, InboundEvent, KeyEventMessage,
    KeyEventPayload, OutboundEvent, SettingsPayload,
};
use uzlinux_renderer::{ButtonComposer, ImageLoader};
use uzlinux_server::{InboundReceiver, PiInboundReceiver};
use uzlinux_window_watcher::ActiveAppEvent;

use crate::state::AppState;

pub struct EventRouter {
    state: Arc<AppState>,
}

impl EventRouter {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    pub fn start(
        self: Arc<Self>,
        mut plugin_rx: InboundReceiver,
        mut pi_rx: PiInboundReceiver,
        mut device_rx: mpsc::UnboundedReceiver<(String, DeviceEvent)>,
        mut window_rx: mpsc::UnboundedReceiver<ActiveAppEvent>,
    ) {
        // 1. Task: Inbound events from plugins
        let router1 = self.clone();
        tokio::spawn(async move {
            while let Some((plugin_uuid, event)) = plugin_rx.recv().await {
                router1.handle_plugin_inbound(&plugin_uuid, event).await;
            }
        });

        // 2. Task: Inbound events from Property Inspectors
        let router2 = self.clone();
        tokio::spawn(async move {
            while let Some((context, event)) = pi_rx.recv().await {
                router2.handle_pi_inbound(&context, event).await;
            }
        });

        // 3. Task: Device hardware events (buttons, dials, touch)
        let router3 = self.clone();
        tokio::spawn(async move {
            while let Some((device_id, event)) = device_rx.recv().await {
                router3.handle_device_event(&device_id, event).await;
            }
        });

        // 4. Task: Window focus watcher events (automatic profile switching)
        let router4 = self.clone();
        tokio::spawn(async move {
            while let Some(event) = window_rx.recv().await {
                router4.handle_window_event(event).await;
            }
        });
    }

    /// Robustly resolves a key context to (device_id, key_idx, plugin_uuid, action_uuid)
    /// across context_to_key cache, active profiles, and fallback context string patterns.
    fn resolve_key(&self, context: &str) -> Option<(String, u8, String, String)> {
        if context.is_empty() {
            return None;
        }

        // 1. Check direct context_to_key mapping
        if let Some((dev, idx)) = self.state.context_to_key.get(context).map(|r| r.value().clone()) {
            if let Some(active) = self.state.active_profiles.get(&dev) {
                if let Some(Some(ref key)) = active.1.keys.get(idx as usize) {
                    return Some((dev, idx, key.action.plugin.clone(), key.action.uuid.clone()));
                }
            }
        }

        // 2. Search active profiles for matching key context
        for item in self.state.active_profiles.iter() {
            let dev_id = item.key();
            for (idx, k_opt) in item.value().1.keys.iter().enumerate() {
                if let Some(k) = k_opt {
                    if k.context == context {
                        return Some((dev_id.clone(), idx as u8, k.action.plugin.clone(), k.action.uuid.clone()));
                    }
                }
            }
        }

        // 3. Fallback: Parse "Keypad.<idx>" or similar context pattern
        if context.starts_with("Keypad.") {
            let parts: Vec<&str> = context.split('.').collect();
            if parts.len() >= 2 {
                if let Ok(idx) = parts[1].parse::<u8>() {
                    for item in self.state.active_profiles.iter() {
                        let dev_id = item.key();
                        if let Some(Some(ref k)) = item.value().1.keys.get(idx as usize) {
                            return Some((dev_id.clone(), idx, k.action.plugin.clone(), k.action.uuid.clone()));
                        }
                    }
                }
            }
        }

        None
    }

    async fn handle_plugin_inbound(&self, plugin_uuid: &str, event: InboundEvent) {
        debug!("Plugin inbound from '{}': {:?}", plugin_uuid, event);
        match event {
            InboundEvent::SetTitle(msg) => {
                if let Some((device_id, key_idx, _, _)) = self.resolve_key(&msg.context) {
                    self.update_key_title(&device_id, key_idx, &msg.payload.title).await;
                }
            }
            InboundEvent::SetImage(msg) => {
                if let Some((device_id, key_idx, _, _)) = self.resolve_key(&msg.context) {
                    if let Ok(dyn_img) = ImageLoader::load_image(&msg.payload.image, Some(&self.state.config_manager.plugins_dir())) {
                        if let Ok(png_bytes) = ButtonComposer::to_png_bytes(&dyn_img.to_rgba8()) {
                            if let Some(dev) = self.state.device_manager.get_device(&device_id) {
                                let _ = dev.set_key_image(key_idx, &png_bytes).await;
                            }
                            self.state.send_image_to_device(&device_id, key_idx, &png_bytes);
                        }
                    }
                }
            }
            InboundEvent::ShowAlert(msg) => {
                if let Some((device_id, key_idx, _, _)) = self.resolve_key(&msg.context) {
                    self.show_indicator_overlay(&device_id, key_idx, true).await;
                }
            }
            InboundEvent::ShowOk(msg) => {
                if let Some((device_id, key_idx, _, _)) = self.resolve_key(&msg.context) {
                    self.show_indicator_overlay(&device_id, key_idx, false).await;
                }
            }
            InboundEvent::SetState(msg) => {
                if let Some((device_id, key_idx, _, _)) = self.resolve_key(&msg.context) {
                    self.set_key_state(&device_id, key_idx, msg.payload.state).await;
                }
            }
            InboundEvent::SwitchToProfile(msg) => {
                self.switch_profile(&msg.device, &msg.payload.profile).await;
            }
            InboundEvent::OpenUrl(msg) => {
                info!("Opening URL requested by plugin: {}", msg.payload.url);
                if let Err(e) = open::that(&msg.payload.url) {
                    error!("Failed to open URL {}: {}", msg.payload.url, e);
                }
            }
            InboundEvent::LogMessage(msg) => {
                info!("[Plugin Log: {}] {}", plugin_uuid, msg.payload.message);
            }
            InboundEvent::SendToPropertyInspector(msg) => {
                let pi_event = serde_json::json!({
                    "action": msg.action,
                    "event": "sendToPropertyInspector",
                    "context": msg.context,
                    "payload": msg.payload,
                });
                self.state.pi_hub.send_to_property_inspector(&msg.context, &pi_event);
            }
            InboundEvent::SendToPlugin(msg) => {
                let outbound = OutboundEvent::SendToPlugin(uzlinux_protocol::SendToPluginOutboundMessage {
                    action: msg.action,
                    context: msg.context,
                    payload: msg.payload,
                });
                self.state.plugin_hub.send_to_plugin(plugin_uuid, &outbound);
            }
            InboundEvent::SetSettings(msg) => {
                let resolved = self.resolve_key(&msg.context);
                if let Some((device_id, key_idx, plugin, action_uuid)) = resolved {
                    let new_settings = msg.payload.clone();
                    info!("Persisting settings for key {} on device '{}': {:?}", key_idx, device_id, new_settings);

                    if let Some(mut active) = self.state.active_profiles.get_mut(&device_id) {
                        if let Some(Some(ref mut key)) = active.1.keys.get_mut(key_idx as usize) {
                            key.settings = new_settings.clone();
                            // Persist profile immediately to disk
                            if let Err(e) = self.state.config_manager.save_profile(&device_id, &active.0, &active.1) {
                                error!("Failed to save profile on SetSettings: {}", e);
                            }
                        }
                    }

                    // Synchronize settings with frontend UI so currentProfileData is never stale
                    self.state.emit_to_frontend("key_settings_changed", serde_json::json!({
                        "deviceId": device_id,
                        "keyIndex": key_idx,
                        "settings": new_settings,
                    }));

                    let plugin_target = if !plugin.is_empty() { plugin } else { plugin_uuid.to_string() };
                    if !plugin_target.is_empty() {
                        // Notify plugin
                        let did_receive = OutboundEvent::DidReceiveSettings(DidReceiveSettingsMessage {
                            action: action_uuid.clone(),
                            context: msg.context.clone(),
                            device: device_id.clone(),
                            payload: SettingsPayload {
                                settings: new_settings.clone(),
                                coordinates: Coordinates {
                                    column: key_idx % 5,
                                    row: key_idx / 5,
                                },
                                is_in_multi_action: false,
                            },
                        });
                        self.state.plugin_hub.send_to_plugin(&plugin_target, &did_receive);
                    }

                    // Also notify Property Inspector
                    self.state.pi_hub.send_to_property_inspector(&msg.context, &serde_json::json!({
                        "event": "didReceiveSettings",
                        "action": action_uuid,
                        "context": msg.context,
                        "device": device_id,
                        "payload": {
                            "settings": new_settings,
                            "coordinates": {
                                "column": key_idx % 5,
                                "row": key_idx / 5,
                            },
                            "isInMultiAction": false
                        }
                    }));

                    // Re-render button
                    self.rerender_key(&device_id, key_idx).await;
                } else {
                    warn!("Could not resolve key context for SetSettings: '{}'", msg.context);
                }
            }
            InboundEvent::GetSettings(msg) => {
                let resolved = self.resolve_key(&msg.context);
                if let Some((device_id, key_idx, plugin, action_uuid)) = resolved {
                    if let Some(active) = self.state.active_profiles.get(&device_id) {
                        if let Some(Some(ref key)) = active.1.keys.get(key_idx as usize) {
                            let plugin_target = if !plugin.is_empty() { plugin } else { plugin_uuid.to_string() };
                            if !plugin_target.is_empty() {
                                let resp = OutboundEvent::DidReceiveSettings(DidReceiveSettingsMessage {
                                    action: action_uuid.clone(),
                                    context: msg.context.clone(),
                                    device: device_id.clone(),
                                    payload: SettingsPayload {
                                        settings: key.settings.clone(),
                                        coordinates: Coordinates {
                                            column: key_idx % 5,
                                            row: key_idx / 5,
                                        },
                                        is_in_multi_action: false,
                                    },
                                });
                                self.state.plugin_hub.send_to_plugin(&plugin_target, &resp);
                            }

                            // Also reply to Property Inspector
                            self.state.pi_hub.send_to_property_inspector(&msg.context, &serde_json::json!({
                                "event": "didReceiveSettings",
                                "action": action_uuid,
                                "context": msg.context,
                                "device": device_id,
                                "payload": {
                                    "settings": key.settings,
                                    "coordinates": {
                                        "column": key_idx % 5,
                                        "row": key_idx / 5,
                                    },
                                    "isInMultiAction": false
                                }
                            }));
                        }
                    }
                }
            }
            InboundEvent::SetGlobalSettings(msg) => {
                let target_plugin = if !plugin_uuid.is_empty() {
                    plugin_uuid.to_string()
                } else {
                    self.resolve_key(&msg.context).map(|r| r.2).unwrap_or_default()
                };
                if !target_plugin.is_empty() {
                    let _ = self.state.config_manager.save_plugin_settings(&target_plugin, &msg.payload);
                    let did_receive = OutboundEvent::DidReceiveGlobalSettings(DidReceiveGlobalSettingsMessage {
                        payload: GlobalSettingsPayload { settings: msg.payload.clone() },
                    });
                    self.state.plugin_hub.send_to_plugin(&target_plugin, &did_receive);
                }

                self.state.pi_hub.send_to_property_inspector(&msg.context, &serde_json::json!({
                    "event": "didReceiveGlobalSettings",
                    "payload": {
                        "settings": msg.payload
                    }
                }));
            }
            InboundEvent::GetGlobalSettings(msg) => {
                let target_plugin = if !plugin_uuid.is_empty() {
                    plugin_uuid.to_string()
                } else {
                    self.resolve_key(&msg.context).map(|r| r.2).unwrap_or_default()
                };
                if !target_plugin.is_empty() {
                    if let Ok(settings) = self.state.config_manager.load_plugin_settings(&target_plugin) {
                        let resp = OutboundEvent::DidReceiveGlobalSettings(DidReceiveGlobalSettingsMessage {
                            payload: GlobalSettingsPayload { settings: settings.clone() },
                        });
                        self.state.plugin_hub.send_to_plugin(&target_plugin, &resp);

                        self.state.pi_hub.send_to_property_inspector(&msg.context, &serde_json::json!({
                            "event": "didReceiveGlobalSettings",
                            "payload": {
                                "settings": settings
                            }
                        }));
                    }
                }
            }
            InboundEvent::DeviceRegistered(msg) => {
                info!("Device registered via plugin bridge: id='{}', name={:?}", msg.device, msg.name);
            }
            InboundEvent::RegisterDevice(msg) => {
                let dev_id = msg.payload.id.clone();
                info!("Device registered: id='{}', name='{}' ({}x{})", dev_id, msg.payload.name, msg.payload.rows, msg.payload.columns);

                // Register device in device_manager if not already present
                if self.state.device_manager.get_device(&dev_id).is_none() {
                    let info = if dev_id.starts_with("e9-") {
                        let serial = dev_id.trim_start_matches("e9-");
                        uzlinux_core::models::DeviceInfo::new_ulanzi_d200(serial)
                    } else {
                        uzlinux_core::models::DeviceInfo::new_virtual(&dev_id, msg.payload.rows, msg.payload.columns)
                    };
                    let _ = self.state.device_manager.add_virtual_deck(info);
                }

                // Send brightness
                if let Ok(settings) = self.state.config_manager.load_settings() {
                    self.state.send_brightness_to_device(&dev_id, settings.brightness);
                }

                // Initial profile switch and render
                let initial_profile = self.state.config_manager
                    .get_device_selected_profile(&dev_id)
                    .unwrap_or_else(|_| "Default".to_string());
                self.switch_profile(&dev_id, &initial_profile).await;
            }
            InboundEvent::DeviceKeyDown(msg) => {
                let position = msg.payload.position;
                let cols = 5;
                let row = position / cols;
                let col = position % cols;
                self.process_key_event(&msg.payload.device, row, col, position, true).await;
            }
            InboundEvent::DeviceKeyUp(msg) => {
                let position = msg.payload.position;
                let cols = 5;
                let row = position / cols;
                let col = position % cols;
                self.process_key_event(&msg.payload.device, row, col, position, false).await;
            }
            InboundEvent::DevicePluginSetImage(msg) => {
                if let Ok(dyn_img) = ImageLoader::load_image(&msg.image, Some(&self.state.config_manager.plugins_dir())) {
                    if let Ok(png_bytes) = ButtonComposer::to_png_bytes(&dyn_img.to_rgba8()) {
                        if let Some(dev) = self.state.device_manager.get_device(&msg.device) {
                            let _ = dev.set_key_image(msg.key, &png_bytes).await;
                        }
                    }
                }
            }
            InboundEvent::DevicePluginSetBrightness(msg) => {
                if let Some(dev) = self.state.device_manager.get_device(&msg.device) {
                    let _ = dev.set_brightness(msg.value).await;
                }
            }
            _ => {}
        }
    }

    async fn handle_pi_inbound(&self, context: &str, mut event: InboundEvent) {
        debug!("PI inbound from context '{}': {:?}", context, event);

        // Normalize empty context inside messages
        match &mut event {
            InboundEvent::SetSettings(ref mut msg) => {
                if msg.context.is_empty() {
                    msg.context = context.to_string();
                }
            }
            InboundEvent::GetSettings(ref mut msg) => {
                if msg.context.is_empty() {
                    msg.context = context.to_string();
                }
            }
            InboundEvent::SetGlobalSettings(ref mut msg) => {
                if msg.context.is_empty() {
                    msg.context = context.to_string();
                }
            }
            InboundEvent::GetGlobalSettings(ref mut msg) => {
                if msg.context.is_empty() {
                    msg.context = context.to_string();
                }
            }
            InboundEvent::SendToPlugin(ref mut msg) => {
                if msg.context.is_empty() {
                    msg.context = context.to_string();
                }
            }
            _ => {}
        }

        let resolved = self.resolve_key(context);
        let plugin_uuid = self.state.plugin_hub.get_plugin_for_context(context)
            .or_else(|| resolved.as_ref().map(|r| r.2.clone()))
            .unwrap_or_default();

        match event {
            InboundEvent::SendToPlugin(msg) => {
                if !plugin_uuid.is_empty() {
                    let outbound = OutboundEvent::SendToPlugin(uzlinux_protocol::SendToPluginOutboundMessage {
                        action: msg.action,
                        context: msg.context,
                        payload: msg.payload,
                    });
                    self.state.plugin_hub.send_to_plugin(&plugin_uuid, &outbound);
                }
            }
            other => {
                self.handle_plugin_inbound(&plugin_uuid, other).await;
            }
        }
    }

    async fn handle_device_event(&self, device_id: &str, event: DeviceEvent) {
        info!("Hardware event from device '{}': {:?}", device_id, event);
        match event {
            DeviceEvent::KeyDown { row, column, key_index } => {
                self.process_key_event(device_id, row, column, key_index, true).await;
            }
            DeviceEvent::KeyUp { row, column, key_index } => {
                self.process_key_event(device_id, row, column, key_index, false).await;
            }
            _ => {}
        }
    }

    async fn process_key_event(&self, device_id: &str, row: u8, column: u8, key_index: u8, is_down: bool) {
        if let Some(active) = self.state.active_profiles.get(device_id) {
            if let Some(Some(ref key)) = active.1.keys.get(key_index as usize) {
                // Check if plugin is disabled
                if let Ok(app_settings) = self.state.config_manager.load_settings() {
                    let plugin_id = &key.action.plugin;
                    if app_settings.disabled_plugins.contains(plugin_id) {
                        info!("Key event ignored: plugin '{}' is disabled", plugin_id);
                        return;
                    }
                }

                let is_starterpack = key.action.plugin == "com.amansprojects.starterpack.sdPlugin"
                    || key.action.plugin == "com.amansprojects.starterpack"
                    || key.action.plugin == "com.uzlinux.system-starterpack"
                    || key.action.uuid.starts_with("com.amansprojects.starterpack.")
                    || key.action.uuid.starts_with("com.uzlinux.system-starterpack.");

                let is_audio = key.action.plugin == "com.uzlinux.pipewire-audio"
                    || key.action.plugin.contains("pipewire-audio")
                    || key.action.uuid.starts_with("com.uzlinux.pipewire-audio.")
                    || key.action.uuid.starts_with("uzlinux.native.audio.")
                    || key.action.uuid.starts_with("com.sfgrimes.pipewire-audio.");

                let is_media = key.action.plugin == "com.uzlinux.mpris-media"
                    || key.action.plugin.contains("oampris")
                    || key.action.uuid.starts_with("com.uzlinux.mpris-media.")
                    || key.action.uuid.starts_with("uzlinux.native.media.")
                    || key.action.uuid.starts_with("me.amankhanna.oampris.");

                let is_ulanzi = key.action.plugin == "com.uzlinux.ulanzi-d200"
                    || key.action.plugin.contains("ulanzi.d200")
                    || key.action.uuid.starts_with("com.uzlinux.ulanzi-d200.")
                    || key.action.uuid.starts_with("com.glmagalhaes.ulanzi.d200.")
                    || key.action.uuid.starts_with("uzlinux.native.ulanzi.");

                let is_native = key.action.plugin == "uzlinux.native"
                    || key.action.plugin.starts_with("com.uzlinux.")
                    || key.action.uuid.starts_with("uzlinux.native.")
                    || is_starterpack
                    || is_audio
                    || is_media
                    || is_ulanzi;

                // Check if native action (built-in without plugins)
                if is_native {
                    self.execute_native_action(&key.action.uuid, &key.settings, device_id, is_down).await;
                    return;
                }

                // Send to plugin
                let payload = KeyEventPayload {
                    settings: key.settings.clone(),
                    coordinates: Coordinates { column, row },
                    state: key.current_state,
                    user_desired_state: key.current_state,
                    is_in_multi_action: false,
                    controller: "Keypad".to_string(),
                };

                let outbound = if is_down {
                    OutboundEvent::KeyDown(KeyEventMessage {
                        action: key.action.uuid.clone(),
                        context: key.context.clone(),
                        device: device_id.to_string(),
                        payload,
                    })
                } else {
                    OutboundEvent::KeyUp(KeyEventMessage {
                        action: key.action.uuid.clone(),
                        context: key.context.clone(),
                        device: device_id.to_string(),
                        payload,
                    })
                };

                self.state.plugin_hub.send_to_plugin(&key.action.plugin, &outbound);
            }
        }
    }

    async fn execute_native_action(&self, action_uuid: &str, settings: &serde_json::Value, device_id: &str, is_down: bool) {
        info!("Executing native action: '{}' (is_down={})", action_uuid, is_down);
        match action_uuid {
            // Launch Application
            "com.uzlinux.system-starterpack.launchapp" | "uzlinux.native.launchapp" | "me.amankhanna.oadesktopentry.launchapp" => {
                if is_down {
                    let app = settings.get("path").and_then(|v| v.as_str())
                        .or_else(|| settings.get("app").and_then(|v| v.as_str()))
                        .or_else(|| settings.get("exec").and_then(|v| v.as_str()))
                        .unwrap_or("");
                    let args = settings.get("args").and_then(|v| v.as_str());
                    if !app.is_empty() {
                        let _ = crate::desktop_apps::launch_app(app, args);
                    }
                }
            }

            // Open URL / Website
            "com.uzlinux.system-starterpack.openurl" | "uzlinux.native.openurl" | "com.amansprojects.starterpack.openurl" => {
                let url_opt = if is_down {
                    settings.get("down").and_then(|v| v.as_str())
                        .or_else(|| settings.get("url").and_then(|v| v.as_str()))
                } else {
                    settings.get("up").and_then(|v| v.as_str())
                };
                if let Some(url) = url_opt {
                    if !url.trim().is_empty() {
                        let _ = open::that(url);
                    }
                }
            }

            // Shell Command
            "com.uzlinux.system-starterpack.command" | "uzlinux.native.command" | "com.amansprojects.starterpack.runcommand" => {
                let cmd_opt = if is_down {
                    settings.get("down").and_then(|v| v.as_str())
                        .or_else(|| settings.get("command").and_then(|v| v.as_str()))
                } else {
                    settings.get("up").and_then(|v| v.as_str())
                };
                if let Some(cmd) = cmd_opt {
                    if !cmd.trim().is_empty() {
                        let file_opt = settings.get("file").and_then(|v| v.as_str());
                        if let Some(file_path) = file_opt {
                            if !file_path.trim().is_empty() {
                                let shell_cmd = format!("{} > {} 2>&1", cmd, file_path);
                                let mut c = std::process::Command::new("sh");
                                c.arg("-c").arg(&shell_cmd);
                                crate::desktop_apps::detach_command(&mut c);
                                let _ = c.spawn();
                                return;
                            }
                        }
                        let mut c = std::process::Command::new("sh");
                        c.arg("-c").arg(cmd);
                        crate::desktop_apps::detach_command(&mut c);
                        let _ = c.spawn();
                    }
                }
            }

            // Switch Profile
            "com.uzlinux.system-starterpack.switchprofile" | "uzlinux.native.switchprofile" | "com.amansprojects.starterpack.switchprofile" => {
                if is_down {
                    if let Some(target) = settings.get("profile").and_then(|v| v.as_str()) {
                        let dev = settings.get("device").and_then(|v| v.as_str()).unwrap_or(device_id);
                        self.switch_profile(dev, target).await;
                    }
                }
            }

            // Device Brightness
            "com.uzlinux.system-starterpack.brightness" | "uzlinux.native.brightness" | "com.amansprojects.starterpack.devicebrightness" => {
                if is_down {
                    let action = settings.get("action").and_then(|v| v.as_str()).unwrap_or("set");
                    let value = settings.get("value").and_then(|v| v.as_u64()).unwrap_or(50) as u8;

                    if let Ok(mut cur) = self.state.config_manager.load_settings() {
                        match action {
                            "set" => cur.brightness = value.min(100),
                            "increase" => cur.brightness = (cur.brightness + value).min(100),
                            "decrease" => cur.brightness = cur.brightness.saturating_sub(value),
                            _ => cur.brightness = value.min(100),
                        }
                        let _ = self.state.config_manager.save_settings(&cur);
                        if let Some(dev) = self.state.device_manager.get_device(device_id) {
                            let _ = dev.set_brightness(cur.brightness).await;
                        }
                        self.state.send_brightness_to_device(device_id, cur.brightness);
                    }
                }
            }
            "com.uzlinux.system-starterpack.brightness.up" | "uzlinux.native.brightness.up" => {
                if is_down {
                    if let Ok(mut cur) = self.state.config_manager.load_settings() {
                        cur.brightness = (cur.brightness + 10).min(100);
                        let _ = self.state.config_manager.save_settings(&cur);
                        if let Some(dev) = self.state.device_manager.get_device(device_id) {
                            let _ = dev.set_brightness(cur.brightness).await;
                        }
                        self.state.send_brightness_to_device(device_id, cur.brightness);
                    }
                }
            }
            "com.uzlinux.system-starterpack.brightness.down" | "uzlinux.native.brightness.down" => {
                if is_down {
                    if let Ok(mut cur) = self.state.config_manager.load_settings() {
                        cur.brightness = cur.brightness.saturating_sub(10);
                        let _ = self.state.config_manager.save_settings(&cur);
                        if let Some(dev) = self.state.device_manager.get_device(device_id) {
                            let _ = dev.set_brightness(cur.brightness).await;
                        }
                        self.state.send_brightness_to_device(device_id, cur.brightness);
                    }
                }
            }

            // Simulate Keyboard & Mouse Input
            "com.uzlinux.system-starterpack.simulateinput" | "uzlinux.native.simulateinput" | "com.amansprojects.starterpack.inputsimulation" => {
                let script_opt = if is_down {
                    settings.get("down").and_then(|v| v.as_str())
                        .or_else(|| settings.get("input").and_then(|v| v.as_str()))
                } else {
                    settings.get("up").and_then(|v| v.as_str())
                };
                if let Some(script) = script_opt {
                    crate::input_simulator::execute_simulation(script);
                }
            }

            // Ulanzi Screen Mode & Brightness
            "com.uzlinux.ulanzi-d200.display_sleep" | "com.uzlinux.ulanzi-d200.cycle_brightness" | "uzlinux.native.ulanzi.cycle" | "com.glmagalhaes.ulanzi.d200.cycle" => {
                if is_down {
                    info!("Toggling Ulanzi D200 display state natively");
                }
            }

            // PipeWire Master Volume (Dynamic / Adjustable)
            "com.uzlinux.pipewire-audio.volume" | "uzlinux.native.audio.volume" | "com.sfgrimes.pipewire-audio.volume" => {
                if is_down {
                    let dir = settings.get("direction").and_then(|v| v.as_str())
                        .or_else(|| settings.get("action").and_then(|v| v.as_str()))
                        .or_else(|| settings.get("mode").and_then(|v| v.as_str()))
                        .unwrap_or("up");
                    let step = settings.get("volumeStep").and_then(|v| v.as_i64())
                        .or_else(|| settings.get("step").and_then(|v| v.as_i64()))
                        .unwrap_or(5);

                    match dir {
                        "down" | "decrease" | "voldown" => {
                            Self::set_audio_volume("@DEFAULT_AUDIO_SINK@", &format!("{}%-", step));
                        }
                        "mute" | "toggleMute" | "toggle" => {
                            Self::toggle_audio_mute("@DEFAULT_AUDIO_SINK@");
                        }
                        "set" | "setVolume" => {
                            let val = settings.get("value").and_then(|v| v.as_f64()).unwrap_or(0.5);
                            Self::set_audio_volume("@DEFAULT_AUDIO_SINK@", &format!("{:.2}", val));
                        }
                        _ => {
                            Self::set_audio_volume("@DEFAULT_AUDIO_SINK@", &format!("{}%+", step));
                        }
                    }
                }
            }

            // PipeWire Microphone Volume (Dynamic / Adjustable)
            "com.uzlinux.pipewire-audio.micvolume" | "uzlinux.native.audio.micvolume" | "com.sfgrimes.pipewire-audio.micvolume" => {
                if is_down {
                    let dir = settings.get("direction").and_then(|v| v.as_str())
                        .or_else(|| settings.get("action").and_then(|v| v.as_str()))
                        .or_else(|| settings.get("mode").and_then(|v| v.as_str()))
                        .unwrap_or("up");
                    let step = settings.get("volumeStep").and_then(|v| v.as_i64())
                        .or_else(|| settings.get("step").and_then(|v| v.as_i64()))
                        .unwrap_or(5);

                    match dir {
                        "down" | "decrease" => {
                            Self::set_audio_volume("@DEFAULT_AUDIO_SOURCE@", &format!("{}%-", step));
                        }
                        "mute" | "toggleMute" => {
                            Self::toggle_audio_mute("@DEFAULT_AUDIO_SOURCE@");
                        }
                        _ => {
                            Self::set_audio_volume("@DEFAULT_AUDIO_SOURCE@", &format!("{}%+", step));
                        }
                    }
                }
            }

            // PipeWire Specific Output Volume
            "com.uzlinux.pipewire-audio.outputvolume" | "uzlinux.native.audio.outputvolume" | "com.sfgrimes.pipewire-audio.outputvolume" => {
                if is_down {
                    let target = settings.get("outputName").and_then(|v| v.as_str())
                        .or_else(|| settings.get("output").and_then(|v| v.as_str()))
                        .or_else(|| settings.get("device").and_then(|v| v.as_str()))
                        .unwrap_or("@DEFAULT_AUDIO_SINK@");
                    let dir = settings.get("direction").and_then(|v| v.as_str())
                        .or_else(|| settings.get("action").and_then(|v| v.as_str()))
                        .unwrap_or("up");
                    let step = settings.get("volumeStep").and_then(|v| v.as_i64())
                        .or_else(|| settings.get("step").and_then(|v| v.as_i64()))
                        .unwrap_or(5);

                    match dir {
                        "down" | "decrease" => {
                            Self::set_audio_volume(target, &format!("{}%-", step));
                        }
                        "mute" | "toggleMute" => {
                            Self::toggle_audio_mute(target);
                        }
                        _ => {
                            Self::set_audio_volume(target, &format!("{}%+", step));
                        }
                    }
                }
            }

            // PipeWire Specific Input Volume
            "com.uzlinux.pipewire-audio.inputvolume" | "uzlinux.native.audio.inputvolume" | "com.sfgrimes.pipewire-audio.inputvolume" => {
                if is_down {
                    let target = settings.get("inputName").and_then(|v| v.as_str())
                        .or_else(|| settings.get("input").and_then(|v| v.as_str()))
                        .or_else(|| settings.get("device").and_then(|v| v.as_str()))
                        .unwrap_or("@DEFAULT_AUDIO_SOURCE@");
                    let dir = settings.get("direction").and_then(|v| v.as_str())
                        .or_else(|| settings.get("action").and_then(|v| v.as_str()))
                        .unwrap_or("up");
                    let step = settings.get("volumeStep").and_then(|v| v.as_i64())
                        .or_else(|| settings.get("step").and_then(|v| v.as_i64()))
                        .unwrap_or(5);

                    match dir {
                        "down" | "decrease" => {
                            Self::set_audio_volume(target, &format!("{}%-", step));
                        }
                        "mute" | "toggleMute" => {
                            Self::toggle_audio_mute(target);
                        }
                        _ => {
                            Self::set_audio_volume(target, &format!("{}%+", step));
                        }
                    }
                }
            }

            // Push to Talk
            "com.uzlinux.pipewire-audio.pushtotalk" | "uzlinux.native.audio.pushtotalk" | "com.sfgrimes.pipewire-audio.pushtotalk" => {
                let target = settings.get("inputName").and_then(|v| v.as_str())
                    .or_else(|| settings.get("device").and_then(|v| v.as_str()))
                    .unwrap_or("@DEFAULT_AUDIO_SOURCE@");
                if is_down {
                    Self::set_audio_mute(target, false);
                } else {
                    Self::set_audio_mute(target, true);
                }
            }

            // Switch Output / Input Devices
            "com.uzlinux.pipewire-audio.switchoutput" | "uzlinux.native.audio.switchoutput" | "com.sfgrimes.pipewire-audio.switchoutput" => {
                if is_down {
                    let target_opt = settings.get("outputName").and_then(|v| v.as_str())
                        .or_else(|| settings.get("device").and_then(|v| v.as_str()))
                        .filter(|v| !v.trim().is_empty() && *v != "cycle");

                    if let Some(target) = target_opt {
                        Self::set_default_audio_device(target, true);
                    } else {
                        Self::cycle_audio_sinks();
                    }
                }
            }
            "com.uzlinux.pipewire-audio.switchinput" | "uzlinux.native.audio.switchinput" | "com.sfgrimes.pipewire-audio.switchinput" => {
                if is_down {
                    let target_opt = settings.get("inputName").and_then(|v| v.as_str())
                        .or_else(|| settings.get("device").and_then(|v| v.as_str()))
                        .filter(|v| !v.trim().is_empty() && *v != "cycle");

                    if let Some(target) = target_opt {
                        Self::set_default_audio_device(target, false);
                    } else {
                        Self::cycle_audio_sources();
                    }
                }
            }
            "com.uzlinux.pipewire-audio.appvolume" | "uzlinux.native.audio.appvolume" | "com.sfgrimes.pipewire-audio.appvolume" => {
                if is_down {
                    let app = settings.get("appName").and_then(|v| v.as_str())
                        .or_else(|| settings.get("app").and_then(|v| v.as_str()))
                        .unwrap_or("");
                    let dir = settings.get("direction").and_then(|v| v.as_str())
                        .or_else(|| settings.get("action").and_then(|v| v.as_str()))
                        .unwrap_or("up");
                    let step = settings.get("volumeStep").and_then(|v| v.as_i64())
                        .or_else(|| settings.get("step").and_then(|v| v.as_i64()))
                        .unwrap_or(5);
                    Self::adjust_app_volume(app, dir, step);
                }
            }

            // Dedicated audio shortcuts
            "com.uzlinux.pipewire-audio.volup" | "uzlinux.native.audio.volup" => {
                if is_down {
                    Self::set_audio_volume("@DEFAULT_AUDIO_SINK@", "5%+");
                }
            }
            "com.uzlinux.pipewire-audio.voldown" | "uzlinux.native.audio.voldown" => {
                if is_down {
                    Self::set_audio_volume("@DEFAULT_AUDIO_SINK@", "5%-");
                }
            }
            "com.uzlinux.pipewire-audio.mute" | "uzlinux.native.audio.mute" => {
                if is_down {
                    Self::toggle_audio_mute("@DEFAULT_AUDIO_SINK@");
                }
            }
            "com.uzlinux.pipewire-audio.micvolup" | "uzlinux.native.audio.micvolup" => {
                if is_down {
                    Self::set_audio_volume("@DEFAULT_AUDIO_SOURCE@", "5%+");
                }
            }
            "com.uzlinux.pipewire-audio.micvoldown" | "uzlinux.native.audio.micvoldown" => {
                if is_down {
                    Self::set_audio_volume("@DEFAULT_AUDIO_SOURCE@", "5%-");
                }
            }
            "com.uzlinux.pipewire-audio.micmute" | "uzlinux.native.audio.micmute" => {
                if is_down {
                    Self::toggle_audio_mute("@DEFAULT_AUDIO_SOURCE@");
                }
            }

            // MPRIS Media Actions
            "com.uzlinux.mpris-media.playpause" | "uzlinux.native.media.playpause" | "me.amankhanna.oampris.playpause" => {
                if is_down {
                    Self::control_mpris("PlayPause");
                }
            }
            "com.uzlinux.mpris-media.next" | "uzlinux.native.media.next" | "me.amankhanna.oampris.next" => {
                if is_down {
                    Self::control_mpris("Next");
                }
            }
            "com.uzlinux.mpris-media.prev" | "uzlinux.native.media.prev" | "uzlinux.native.media.previous" | "me.amankhanna.oampris.previous" => {
                if is_down {
                    Self::control_mpris("Previous");
                }
            }
            "com.uzlinux.mpris-media.stop" | "uzlinux.native.media.stop" | "me.amankhanna.oampris.stop" => {
                if is_down {
                    Self::control_mpris("Stop");
                }
            }
            "com.uzlinux.mpris-media.volup" | "uzlinux.native.media.volup" | "me.amankhanna.oampris.volumeup" => {
                if is_down {
                    Self::control_mpris_volume(0.05);
                }
            }
            "com.uzlinux.mpris-media.voldown" | "uzlinux.native.media.voldown" | "me.amankhanna.oampris.volumedown" => {
                if is_down {
                    Self::control_mpris_volume(-0.05);
                }
            }
            "com.uzlinux.mpris-media.seekforward" | "uzlinux.native.media.seekforward" | "me.amankhanna.oampris.seekforwards" => {
                if is_down {
                    Self::control_mpris_seek(10_000_000);
                }
            }
            "com.uzlinux.mpris-media.seekbackward" | "uzlinux.native.media.seekbackward" | "me.amankhanna.oampris.seekbackwards" => {
                if is_down {
                    Self::control_mpris_seek(-10_000_000);
                }
            }
            "com.uzlinux.mpris-media.shuffle" | "uzlinux.native.media.shuffle" | "me.amankhanna.oampris.shuffle" => {
                if is_down {
                    Self::control_mpris_shuffle_toggle();
                }
            }
            "com.uzlinux.mpris-media.repeat" | "uzlinux.native.media.repeat" | "me.amankhanna.oampris.repeat" => {
                if is_down {
                    Self::control_mpris_loop_cycle();
                }
            }
            _ => {
                warn!("Unknown native action: '{}'", action_uuid);
            }
        }
    }

    fn set_audio_volume(target: &str, delta_or_val: &str) {
        let tgt = if target.is_empty() { "@DEFAULT_AUDIO_SINK@" } else { target };
        let _ = std::process::Command::new("wpctl")
            .args(["set-volume", tgt, delta_or_val])
            .spawn();

        let pactl_target = if tgt == "@DEFAULT_AUDIO_SINK@" {
            "@DEFAULT_SINK@"
        } else if tgt == "@DEFAULT_AUDIO_SOURCE@" {
            "@DEFAULT_SOURCE@"
        } else {
            tgt
        };

        let pactl_val = if delta_or_val.ends_with('+') {
            format!("+{}", delta_or_val.trim_end_matches('+'))
        } else if delta_or_val.ends_with('-') {
            format!("-{}", delta_or_val.trim_end_matches('-'))
        } else {
            delta_or_val.to_string()
        };

        let is_source = tgt.contains("source") || tgt.contains("input");
        if is_source {
            let _ = std::process::Command::new("pactl")
                .args(["set-source-volume", pactl_target, &pactl_val])
                .spawn();
        } else {
            let _ = std::process::Command::new("pactl")
                .args(["set-sink-volume", pactl_target, &pactl_val])
                .spawn();
        }
    }

    fn toggle_audio_mute(target: &str) {
        let tgt = if target.is_empty() { "@DEFAULT_AUDIO_SINK@" } else { target };
        let _ = std::process::Command::new("wpctl")
            .args(["set-mute", tgt, "toggle"])
            .spawn();

        let pactl_target = if tgt == "@DEFAULT_AUDIO_SINK@" {
            "@DEFAULT_SINK@"
        } else if tgt == "@DEFAULT_AUDIO_SOURCE@" {
            "@DEFAULT_SOURCE@"
        } else {
            tgt
        };

        let is_source = tgt.contains("source") || tgt.contains("input");
        if is_source {
            let _ = std::process::Command::new("pactl")
                .args(["set-source-mute", pactl_target, "toggle"])
                .spawn();
        } else {
            let _ = std::process::Command::new("pactl")
                .args(["set-sink-mute", pactl_target, "toggle"])
                .spawn();
        }
    }

    fn set_audio_mute(target: &str, mute: bool) {
        let tgt = if target.is_empty() { "@DEFAULT_AUDIO_SOURCE@" } else { target };
        let val = if mute { "1" } else { "0" };
        let _ = std::process::Command::new("wpctl")
            .args(["set-mute", tgt, val])
            .spawn();

        let pactl_target = if tgt == "@DEFAULT_AUDIO_SOURCE@" {
            "@DEFAULT_SOURCE@"
        } else if tgt == "@DEFAULT_AUDIO_SINK@" {
            "@DEFAULT_SINK@"
        } else {
            tgt
        };

        let is_source = tgt.contains("source") || tgt.contains("input");
        if is_source {
            let _ = std::process::Command::new("pactl")
                .args(["set-source-mute", pactl_target, val])
                .spawn();
        } else {
            let _ = std::process::Command::new("pactl")
                .args(["set-sink-mute", pactl_target, val])
                .spawn();
        }
    }

    fn set_default_audio_device(target: &str, is_sink: bool) {
        let _ = std::process::Command::new("wpctl")
            .args(["set-default", target])
            .spawn();
        if is_sink {
            let _ = std::process::Command::new("pactl")
                .args(["set-default-sink", target])
                .spawn();
        } else {
            let _ = std::process::Command::new("pactl")
                .args(["set-default-source", target])
                .spawn();
        }
    }

    fn cycle_audio_sinks() {
        let script = r#"
            sinks=$(pactl list short sinks 2>/dev/null | awk '{print $2}')
            current=$(pactl get-default-sink 2>/dev/null)
            next=""
            first=""
            found=0
            for s in $sinks; do
                if [ -z "$first" ]; then first="$s"; fi
                if [ $found -eq 1 ]; then next="$s"; break; fi
                if [ "$s" = "$current" ]; then found=1; fi
            done
            if [ -z "$next" ]; then next="$first"; fi
            if [ -n "$next" ]; then
                pactl set-default-sink "$next" 2>/dev/null
                wpctl set-default "$next" 2>/dev/null
            fi
        "#;
        let _ = std::process::Command::new("sh")
            .arg("-c")
            .arg(script)
            .spawn();
    }

    fn cycle_audio_sources() {
        let script = r#"
            sources=$(pactl list short sources 2>/dev/null | grep -v '\.monitor' | awk '{print $2}')
            current=$(pactl get-default-source 2>/dev/null)
            next=""
            first=""
            found=0
            for s in $sources; do
                if [ -z "$first" ]; then first="$s"; fi
                if [ $found -eq 1 ]; then next="$s"; break; fi
                if [ "$s" = "$current" ]; then found=1; fi
            done
            if [ -z "$next" ]; then next="$first"; fi
            if [ -n "$next" ]; then
                pactl set-default-source "$next" 2>/dev/null
                wpctl set-default "$next" 2>/dev/null
            fi
        "#;
        let _ = std::process::Command::new("sh")
            .arg("-c")
            .arg(script)
            .spawn();
    }

    fn adjust_app_volume(app: &str, mode: &str, step: i64) {
        let arg = match mode {
            "decrease" | "down" => format!("-{}%", step),
            "toggleMute" | "mute" => "toggle".to_string(),
            _ => format!("+{}%", step),
        };
        let script = format!(
            r##"
            pactl list sink-inputs | awk -v app="{}" -v arg="{}" '
                /Sink Input #/ {{ id=$3; gsub(/#/, "", id) }}
                /application.name =/ || /media.name =/ || /application.process.binary =/ {{
                    if (tolower($0) ~ tolower(app)) {{
                        if (arg == "toggle") {{
                            system("pactl set-sink-input-mute " id " toggle")
                        }} else {{
                            system("pactl set-sink-input-volume " id " " arg)
                        }}
                    }}
                }}
            '
            "##,
            app, arg
        );
        let _ = std::process::Command::new("sh")
            .arg("-c")
            .arg(script)
            .spawn();
    }

    fn control_mpris(method: &str) {
        let cmd = format!(
            r#"for dest in $(busctl --user list --acquired 2>/dev/null | awk '/org\.mpris\.MediaPlayer2/ {{print $1}}'); do busctl --user call "$dest" /org/mpris/MediaPlayer2 org.mpris.MediaPlayer2.Player {} >/dev/null 2>&1 || qdbus "$dest" /org/mpris/MediaPlayer2 org.mpris.MediaPlayer2.Player.{} >/dev/null 2>&1; done"#,
            method, method
        );
        let _ = std::process::Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .spawn();
    }

    fn control_mpris_seek(offset_usec: i64) {
        let cmd = format!(
            r#"for dest in $(busctl --user list --acquired 2>/dev/null | awk '/org\.mpris\.MediaPlayer2/ {{print $1}}'); do busctl --user call "$dest" /org/mpris/MediaPlayer2 org.mpris.MediaPlayer2.Player Seek x {} >/dev/null 2>&1; done"#,
            offset_usec
        );
        let _ = std::process::Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .spawn();
    }

    fn control_mpris_volume(delta: f64) {
        let cmd = format!(
            r#"for dest in $(busctl --user list --acquired 2>/dev/null | awk '/org\.mpris\.MediaPlayer2/ {{print $1}}'); do
                curr=$(busctl --user get-property "$dest" /org/mpris/MediaPlayer2 org.mpris.MediaPlayer2.Player Volume 2>/dev/null | awk '{{print $2}}')
                if [ -n "$curr" ]; then
                    newval=$(awk "BEGIN {{ v = $curr + ({}); if (v < 0) v = 0; if (v > 1.5) v = 1.5; print v }}")
                    busctl --user set-property "$dest" /org/mpris/MediaPlayer2 org.mpris.MediaPlayer2.Player Volume d "$newval" 2>/dev/null
                fi
            done"#,
            delta
        );
        let _ = std::process::Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .spawn();
    }

    fn control_mpris_shuffle_toggle() {
        let cmd = r#"for dest in $(busctl --user list --acquired 2>/dev/null | awk '/org\.mpris\.MediaPlayer2/ {print $1}'); do
            curr=$(busctl --user get-property "$dest" /org/mpris/MediaPlayer2 org.mpris.MediaPlayer2.Player Shuffle 2>/dev/null | awk '{print $2}')
            if [ "$curr" = "true" ]; then
                busctl --user set-property "$dest" /org/mpris/MediaPlayer2 org.mpris.MediaPlayer2.Player Shuffle b false 2>/dev/null
            else
                busctl --user set-property "$dest" /org/mpris/MediaPlayer2 org.mpris.MediaPlayer2.Player Shuffle b true 2>/dev/null
            fi
        done"#;
        let _ = std::process::Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .spawn();
    }

    fn control_mpris_loop_cycle() {
        let cmd = r#"for dest in $(busctl --user list --acquired 2>/dev/null | awk '/org\.mpris\.MediaPlayer2/ {print $1}'); do
            curr=$(busctl --user get-property "$dest" /org/mpris/MediaPlayer2 org.mpris.MediaPlayer2.Player LoopStatus 2>/dev/null | awk '{print $2}' | tr -d '"')
            if [ "$curr" = "None" ]; then
                busctl --user set-property "$dest" /org/mpris/MediaPlayer2 org.mpris.MediaPlayer2.Player LoopStatus s "Playlist" 2>/dev/null
            elif [ "$curr" = "Playlist" ]; then
                busctl --user set-property "$dest" /org/mpris/MediaPlayer2 org.mpris.MediaPlayer2.Player LoopStatus s "Track" 2>/dev/null
            else
                busctl --user set-property "$dest" /org/mpris/MediaPlayer2 org.mpris.MediaPlayer2.Player LoopStatus s "None" 2>/dev/null
            fi
        done"#;
        let _ = std::process::Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .spawn();
    }

    async fn handle_window_event(&self, event: ActiveAppEvent) {
        if let Ok(apps) = self.state.config_manager.load_applications() {
            for (app_key, device_map) in &apps.map {
                // Match by application ID or process name substring
                if event.app_name.eq_ignore_ascii_case(app_key)
                    || event.title.to_lowercase().contains(&app_key.to_lowercase())
                {
                    for (device_id, target_profile) in device_map {
                        // Check if current active profile is already the target profile
                        let already_active = self.state.active_profiles.get(device_id)
                            .map(|p| p.0 == *target_profile)
                            .unwrap_or(false);

                        if !already_active {
                            info!("Window Watcher match for app '{}'! Switching device '{}' to profile '{}'", app_key, device_id, target_profile);
                            self.switch_profile(device_id, target_profile).await;
                        }
                    }
                }
            }
        }
    }

    pub async fn switch_profile(&self, device_id: &str, profile_name: &str) {
        info!("Switching profile on device '{}' to '{}'", device_id, profile_name);

        match self.state.config_manager.load_profile(device_id, profile_name) {
            Ok(profile) => {
                // Update selected profile file
                let _ = self.state.config_manager.set_device_selected_profile(device_id, profile_name);
                self.state.set_active_profile(device_id, profile_name, profile.clone());

                // Notify frontend UI of profile change
                self.state.emit_to_frontend("profile_changed", serde_json::json!({
                    "deviceId": device_id,
                    "profileName": profile_name,
                }));

                // Re-render all keys on the hardware
                self.render_all_keys(device_id, &profile).await;

                // Send willAppear events to plugins
                for (idx, key_opt) in profile.keys.iter().enumerate() {
                    if let Some(key) = key_opt {
                        let ev = OutboundEvent::WillAppear(AppearanceMessage {
                            action: key.action.uuid.clone(),
                            context: key.context.clone(),
                            device: device_id.to_string(),
                            payload: AppearancePayload {
                                settings: key.settings.clone(),
                                coordinates: Coordinates {
                                    column: (idx as u8) % 5,
                                    row: (idx as u8) / 5,
                                },
                                state: key.current_state,
                                is_in_multi_action: false,
                                controller: "Keypad".to_string(),
                            },
                        });
                        self.state.plugin_hub.send_to_plugin(&key.action.plugin, &ev);
                    }
                }
            }
            Err(e) => {
                warn!("Could not load profile '{}' for device '{}': {}", profile_name, device_id, e);
            }
        }
    }

    pub async fn render_all_keys(&self, device_id: &str, profile: &Profile) {
        self.state.render_all_keys_for_device(device_id, profile).await;
    }

    pub async fn rerender_key(&self, device_id: &str, key_idx: u8) {
        if let Some(active) = self.state.active_profiles.get(device_id) {
            if let Some(Some(ref key)) = active.1.keys.get(key_idx as usize) {
                if let Some(state) = key.states.get(key.current_state) {
                    if let Ok(canvas) = self.state.composer.compose_state(
                        state,
                        Some(&self.state.config_manager.base_dir()),
                    ) {
                        if let Ok(png_bytes) = ButtonComposer::to_png_bytes(&canvas) {
                            if let Some(dev) = self.state.device_manager.get_device(device_id) {
                                let _ = dev.set_key_image(key_idx, &png_bytes).await;
                            }
                            self.state.send_image_to_device(device_id, key_idx, &png_bytes);
                        }
                    }
                }
            }
        }
    }

    async fn update_key_title(&self, device_id: &str, key_idx: u8, title: &str) {
        let mut state_def = None;
        if let Some(mut active) = self.state.active_profiles.get_mut(device_id) {
            if let Some(Some(ref mut key)) = active.1.keys.get_mut(key_idx as usize) {
                if let Some(state) = key.states.get_mut(key.current_state) {
                    state.text = title.to_string();
                    state_def = Some(state.clone());
                }
            }
            if state_def.is_some() {
                let _ = self.state.config_manager.save_profile(device_id, &active.0, &active.1);
            }
        }

        if let Some(state) = state_def {
            if let Ok(canvas) = self.state.composer.compose_state(
                &state,
                Some(&self.state.config_manager.base_dir()),
            ) {
                if let Ok(png_bytes) = ButtonComposer::to_png_bytes(&canvas) {
                    if let Some(dev) = self.state.device_manager.get_device(device_id) {
                        let _ = dev.set_key_image(key_idx, &png_bytes).await;
                    }
                    self.state.send_image_to_device(device_id, key_idx, &png_bytes);
                }
            }
        }
    }

    async fn set_key_state(&self, device_id: &str, key_idx: u8, state_idx: usize) {
        if let Some(mut active) = self.state.active_profiles.get_mut(device_id) {
            if let Some(Some(ref mut key)) = active.1.keys.get_mut(key_idx as usize) {
                key.current_state = state_idx;
                if let Some(state) = key.states.get(state_idx) {
                    if let Ok(canvas) = self.state.composer.compose_state(
                        state,
                        Some(&self.state.config_manager.base_dir()),
                    ) {
                        if let Ok(png_bytes) = ButtonComposer::to_png_bytes(&canvas) {
                            if let Some(dev) = self.state.device_manager.get_device(device_id) {
                                let _ = dev.set_key_image(key_idx, &png_bytes).await;
                            }
                            self.state.send_image_to_device(device_id, key_idx, &png_bytes);
                        }
                    }
                }
            }
        }
    }

    async fn show_indicator_overlay(&self, device_id: &str, key_idx: u8, is_alert: bool) {
        if let Some(active) = self.state.active_profiles.get(device_id) {
            if let Some(Some(ref key)) = active.1.keys.get(key_idx as usize) {
                if let Some(state) = key.states.get(key.current_state) {
                    if let Ok(mut canvas) = self.state.composer.compose_state(
                        state,
                        Some(&self.state.config_manager.base_dir()),
                    ) {
                        if is_alert {
                            self.state.composer.overlay_alert(&mut canvas);
                        } else {
                            self.state.composer.overlay_ok(&mut canvas);
                        }
                        if let Ok(png_bytes) = ButtonComposer::to_png_bytes(&canvas) {
                            if let Some(dev) = self.state.device_manager.get_device(device_id) {
                                let _ = dev.set_key_image(key_idx, &png_bytes).await;
                                self.state.send_image_to_device(device_id, key_idx, &png_bytes);

                                // Reset after 1.5 seconds
                                let state_clone = self.state.clone();
                                let dev_id = device_id.to_string();
                                let state_def = state.clone();
                                tokio::spawn(async move {
                                    tokio::time::sleep(Duration::from_millis(1500)).await;
                                    if let Ok(normal_canvas) = state_clone.composer.compose_state(
                                        &state_def,
                                        Some(&state_clone.config_manager.base_dir()),
                                    ) {
                                        if let Ok(normal_png) = ButtonComposer::to_png_bytes(&normal_canvas) {
                                            if let Some(d) = state_clone.device_manager.get_device(&dev_id) {
                                                let _ = d.set_key_image(key_idx, &normal_png).await;
                                            }
                                            state_clone.send_image_to_device(&dev_id, key_idx, &normal_png);
                                        }
                                    }
                                });
                            }
                        }
                    }
                }
            }
        }
    }
}

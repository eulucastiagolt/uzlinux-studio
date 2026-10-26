use std::sync::Arc;
use serde::{Deserialize, Serialize};
use tauri::State;

use uzlinux_core::models::{AppSettings, DeviceInfo, Profile, StateDefinition};
use uzlinux_core::{OpenDeckScanResult, OpenDeckImportOptions, OpenDeckImportReport};
use uzlinux_plugin_manager::PluginDiscovery;
use uzlinux_protocol::{AppearanceMessage, AppearancePayload, Coordinates, OutboundEvent};
use uzlinux_server::ServerPorts;

use crate::state::AppState;

#[derive(Serialize)]
pub struct CategoryInfo {
    pub name: String,
    pub icon: Option<String>,
    pub actions: Vec<ActionInfo>,
}

#[derive(Serialize)]
pub struct ActionInfo {
    pub uuid: String,
    pub name: String,
    pub tooltip: Option<String>,
    pub icon: Option<String>,
    pub plugin: String,
    pub property_inspector: String,
    pub controllers: Vec<String>,
    pub states: Vec<StateDefinition>,
}

#[tauri::command]
pub async fn get_devices(state: State<'_, Arc<AppState>>) -> Result<Vec<DeviceInfo>, String> {
    let mut list = Vec::new();
    let ids = state.device_manager.list_device_ids();
    for id in ids {
        if let Some(dev) = state.device_manager.get_device(&id) {
            list.push(dev.info().clone());
        }
    }
    Ok(list)
}

#[tauri::command]
pub async fn get_port_base(state: State<'_, Arc<AppState>>) -> Result<ServerPorts, String> {
    Ok(state.ports)
}

#[tauri::command]
pub async fn get_profiles(
    device_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<String>, String> {
    state.config_manager.list_profiles(&device_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_selected_profile(
    device_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<String, String> {
    if let Some((name, _)) = state.get_active_profile(&device_id) {
        return Ok(name);
    }
    state.config_manager.get_device_selected_profile(&device_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_selected_profile(
    device_id: String,
    profile_name: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    // Notify plugins that old keys will disappear
    if let Some((_, old_profile)) = state.get_active_profile(&device_id) {
        for (idx, old_key_opt) in old_profile.keys.iter().enumerate() {
            if let Some(old_key) = old_key_opt {
                let ev = OutboundEvent::WillDisappear(AppearanceMessage {
                    action: old_key.action.uuid.clone(),
                    context: old_key.context.clone(),
                    device: device_id.to_string(),
                    payload: AppearancePayload {
                        settings: old_key.settings.clone(),
                        coordinates: Coordinates {
                            column: (idx as u8) % 5,
                            row: (idx as u8) / 5,
                        },
                        state: old_key.current_state,
                        is_in_multi_action: false,
                        controller: "Keypad".to_string(),
                    },
                });
                state.plugin_hub.send_to_plugin(&old_key.action.plugin, &ev);
            }
        }
    }

    // Load and switch profile
    let profile = state.config_manager.load_profile(&device_id, &profile_name)
        .map_err(|e| e.to_string())?;
    let _ = state.config_manager.set_device_selected_profile(&device_id, &profile_name);
    state.set_active_profile(&device_id, &profile_name, profile.clone());

    // Re-render all keys on virtual deck and physical hardware
    state.render_all_keys_for_device(&device_id, &profile).await;

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
            state.plugin_hub.send_to_plugin(&key.action.plugin, &ev);
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn get_profile_data(
    device_id: String,
    profile_name: String,
    state: State<'_, Arc<AppState>>,
) -> Result<Profile, String> {
    state.config_manager.load_profile(&device_id, &profile_name).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_profile_data(
    device_id: String,
    profile_name: String,
    mut profile: Profile,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    // Preserve any existing settings populated by plugins/PI if frontend sent an empty settings object
    if let Some((curr_name, ref old_profile)) = state.get_active_profile(&device_id) {
        if curr_name == profile_name {
            for (idx, new_key_opt) in profile.keys.iter_mut().enumerate() {
                if let Some(new_key) = new_key_opt {
                    if let Some(Some(old_key)) = old_profile.keys.get(idx) {
                        if old_key.context == new_key.context
                            && old_key.action.uuid == new_key.action.uuid
                        {
                            let is_native = new_key.action.plugin == "uzlinux.native"
                                || new_key.action.plugin.starts_with("com.amansprojects.starterpack")
                                || new_key.action.uuid.starts_with("com.amansprojects.starterpack.");

                            let is_new_empty = match &new_key.settings {
                                serde_json::Value::Null => true,
                                serde_json::Value::Object(map) => map.is_empty(),
                                _ => false,
                            };
                            let is_old_non_empty = match &old_key.settings {
                                serde_json::Value::Object(map) => !map.is_empty(),
                                serde_json::Value::Null => false,
                                _ => true,
                            };

                            if !is_native {
                                if is_old_non_empty {
                                    if is_new_empty {
                                        new_key.settings = old_key.settings.clone();
                                    } else if let (serde_json::Value::Object(old_map), serde_json::Value::Object(new_map)) = (&old_key.settings, &mut new_key.settings) {
                                        // Merge settings to ensure no keys configured in Property Inspector are lost
                                        for (k, v) in old_map {
                                            if !new_map.contains_key(k) {
                                                new_map.insert(k.clone(), v.clone());
                                            }
                                        }
                                    }
                                }
                            } else if is_new_empty && is_old_non_empty {
                                new_key.settings = old_key.settings.clone();
                            }
                        }
                    }
                }
            }
        }
    }

    state.config_manager.save_profile(&device_id, &profile_name, &profile)
        .map_err(|e| e.to_string())?;

    if let Some((curr_name, old_profile)) = state.get_active_profile(&device_id) {
        if curr_name == profile_name {
            // Check for keys that disappeared or changed action/context
            for (idx, old_key_opt) in old_profile.keys.iter().enumerate() {
                if let Some(old_key) = old_key_opt {
                    let is_gone = match profile.keys.get(idx).and_then(|k| k.as_ref()) {
                        Some(new_key) => new_key.context != old_key.context || new_key.action.uuid != old_key.action.uuid,
                        None => true,
                    };
                    if is_gone {
                        let ev = OutboundEvent::WillDisappear(AppearanceMessage {
                            action: old_key.action.uuid.clone(),
                            context: old_key.context.clone(),
                            device: device_id.to_string(),
                            payload: AppearancePayload {
                                settings: old_key.settings.clone(),
                                coordinates: Coordinates {
                                    column: (idx as u8) % 5,
                                    row: (idx as u8) / 5,
                                },
                                state: old_key.current_state,
                                is_in_multi_action: false,
                                controller: "Keypad".to_string(),
                            },
                        });
                        state.plugin_hub.send_to_plugin(&old_key.action.plugin, &ev);
                    }
                }
            }

            state.set_active_profile(&device_id, &profile_name, profile.clone());
            state.render_all_keys_for_device(&device_id, &profile).await;

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
                    state.plugin_hub.send_to_plugin(&key.action.plugin, &ev);
                }
            }
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn create_profile(
    device_id: String,
    profile_name: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let empty_profile = Profile {
        infobars: Vec::new(),
        keys: vec![None; 15],
    };
    state.config_manager.save_profile(&device_id, &profile_name, &empty_profile)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn delete_profile(
    device_id: String,
    profile_name: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    state.config_manager.delete_profile(&device_id, &profile_name).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn rename_profile(
    device_id: String,
    old_name: String,
    new_name: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    state.config_manager.rename_profile(&device_id, &old_name, &new_name).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_categories(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<CategoryInfo>, String> {
    let plugins = PluginDiscovery::scan_directory(state.config_manager.plugins_dir())
        .unwrap_or_default();

    let settings = state.config_manager.load_settings().unwrap_or_default();
    let mut categories_map: std::collections::HashMap<String, Vec<ActionInfo>> = std::collections::HashMap::new();

    for plugin in plugins {
        if plugin.uuid == "com.amansprojects.starterpack" || plugin.uuid == "com.glmagalhaes.ulanzi.d200" {
            // Handled natively by pre-installed Uzlinux plugins
            continue;
        }
        if settings.disabled_plugins.contains(&plugin.uuid) {
            // Disabled plugin
            continue;
        }
        let cat_name = plugin.manifest.category.unwrap_or_else(|| plugin.manifest.name.clone());
        let plugin_uuid = plugin.uuid.clone();

        for action_m in plugin.manifest.actions {
            let states = if action_m.states.is_empty() {
                vec![StateDefinition::default()]
            } else {
                action_m.states.iter().map(|s| {
                    let mut st = StateDefinition::default();
                    if let Some(ref img) = s.image {
                        st.image = format!("plugins/{}/{}", plugin_uuid, img);
                    }
                    if let Some(ref name) = s.name {
                        st.name = name.clone();
                    }
                    if let Some(ref title) = s.title {
                        st.text = title.clone();
                    }
                    st
                }).collect()
            };

            let pi_path = action_m.property_inspector_path
                .or(plugin.manifest.property_inspector_path.clone())
                .unwrap_or_default();

            let full_pi_path = if !pi_path.is_empty() {
                format!("plugins/{}/{}", plugin_uuid, pi_path)
            } else {
                String::new()
            };

            let act_info = ActionInfo {
                uuid: action_m.uuid,
                name: action_m.name,
                tooltip: action_m.tooltip,
                icon: action_m.icon.map(|i| format!("plugins/{}/{}", plugin_uuid, i)),
                plugin: plugin_uuid.clone(),
                property_inspector: full_pi_path,
                controllers: action_m.controllers,
                states,
            };

            categories_map.entry(cat_name.clone()).or_default().push(act_info);
        }
    }

    let mut plugin_categories = Vec::new();
    for (name, actions) in categories_map {
        plugin_categories.push(CategoryInfo {
            name,
            icon: None,
            actions,
        });
    }
    plugin_categories.sort_by(|a, b| a.name.cmp(&b.name));

    let mut result = build_native_categories(&settings.disabled_plugins);
    result.extend(plugin_categories);
    Ok(result)
}

fn build_native_categories(disabled: &[String]) -> Vec<CategoryInfo> {
    let mut cats = Vec::new();

    if !disabled.contains(&"com.uzlinux.system-starterpack".to_string()) {
        cats.push(CategoryInfo {
            name: "⚡ Sistema & Atalhos (Pré-instalado)".to_string(),
            icon: None,
            actions: vec![
                ActionInfo {
                    uuid: "com.uzlinux.system-starterpack.launchapp".to_string(),
                    name: "Abrir Aplicativo".to_string(),
                    tooltip: Some("Inicia qualquer aplicativo instalado no Linux".to_string()),
                    icon: None,
                    plugin: "com.uzlinux.system-starterpack".to_string(),
                    property_inspector: "native:launchapp".to_string(),
                    controllers: vec!["Keypad".to_string()],
                    states: vec![StateDefinition {
                        text: "App".to_string(),
                        ..Default::default()
                    }],
                },
                ActionInfo {
                    uuid: "com.uzlinux.system-starterpack.openurl".to_string(),
                    name: "Abrir URL / Site".to_string(),
                    tooltip: Some("Abre uma página web no navegador padrão".to_string()),
                    icon: None,
                    plugin: "com.uzlinux.system-starterpack".to_string(),
                    property_inspector: "native:openurl".to_string(),
                    controllers: vec!["Keypad".to_string()],
                    states: vec![StateDefinition {
                        text: "URL".to_string(),
                        ..Default::default()
                    }],
                },
                ActionInfo {
                    uuid: "com.uzlinux.system-starterpack.command".to_string(),
                    name: "Executar Comando Shell".to_string(),
                    tooltip: Some("Executa um comando bash/shell no sistema".to_string()),
                    icon: None,
                    plugin: "com.uzlinux.system-starterpack".to_string(),
                    property_inspector: "native:command".to_string(),
                    controllers: vec!["Keypad".to_string()],
                    states: vec![StateDefinition {
                        text: "Bash".to_string(),
                        ..Default::default()
                    }],
                },
                ActionInfo {
                    uuid: "com.uzlinux.system-starterpack.simulateinput".to_string(),
                    name: "Simular Entrada (Teclas / Mouse)".to_string(),
                    tooltip: Some("Simula toques de teclas, atalhos, cliques de mouse ou texto digitado".to_string()),
                    icon: None,
                    plugin: "com.uzlinux.system-starterpack".to_string(),
                    property_inspector: "native:simulateinput".to_string(),
                    controllers: vec!["Keypad".to_string()],
                    states: vec![StateDefinition {
                        text: "⌨️ Macro".to_string(),
                        ..Default::default()
                    }],
                },
                ActionInfo {
                    uuid: "com.uzlinux.system-starterpack.switchprofile".to_string(),
                    name: "Trocar Perfil".to_string(),
                    tooltip: Some("Alterna para outro perfil do deck".to_string()),
                    icon: None,
                    plugin: "com.uzlinux.system-starterpack".to_string(),
                    property_inspector: "native:switchprofile".to_string(),
                    controllers: vec!["Keypad".to_string()],
                    states: vec![StateDefinition {
                        text: "Perfil".to_string(),
                        ..Default::default()
                    }],
                },
                ActionInfo {
                    uuid: "com.uzlinux.system-starterpack.brightness".to_string(),
                    name: "Definir Brilho".to_string(),
                    tooltip: Some("Configura ou ajusta o nível de brilho do deck".to_string()),
                    icon: None,
                    plugin: "com.uzlinux.system-starterpack".to_string(),
                    property_inspector: "native:brightness".to_string(),
                    controllers: vec!["Keypad".to_string()],
                    states: vec![StateDefinition {
                        text: "🔆 Brilho".to_string(),
                        ..Default::default()
                    }],
                },
                ActionInfo {
                    uuid: "com.uzlinux.system-starterpack.brightness.up".to_string(),
                    name: "Brilho +10%".to_string(),
                    tooltip: Some("Aumenta o brilho do deck em 10%".to_string()),
                    icon: None,
                    plugin: "com.uzlinux.system-starterpack".to_string(),
                    property_inspector: String::new(),
                    controllers: vec!["Keypad".to_string()],
                    states: vec![StateDefinition {
                        text: "Brilho +".to_string(),
                        ..Default::default()
                    }],
                },
                ActionInfo {
                    uuid: "com.uzlinux.system-starterpack.brightness.down".to_string(),
                    name: "Brilho -10%".to_string(),
                    tooltip: Some("Diminui o brilho do deck em 10%".to_string()),
                    icon: None,
                    plugin: "com.uzlinux.system-starterpack".to_string(),
                    property_inspector: String::new(),
                    controllers: vec!["Keypad".to_string()],
                    states: vec![StateDefinition {
                        text: "Brilho -".to_string(),
                        ..Default::default()
                    }],
                },
            ],
        });
    }

    if !disabled.contains(&"com.uzlinux.mpris-media".to_string()) {
        cats.push(CategoryInfo {
            name: "🎵 Controle de Mídia (MPRIS / Pré-instalado)".to_string(),
            icon: None,
            actions: vec![
                ActionInfo {
                    uuid: "com.uzlinux.mpris-media.playpause".to_string(),
                    name: "Play / Pause".to_string(),
                    tooltip: Some("Reproduz ou pausa a mídia ativa (Spotify, YouTube, VLC)".to_string()),
                    icon: None,
                    plugin: "com.uzlinux.mpris-media".to_string(),
                    property_inspector: "native:media".to_string(),
                    controllers: vec!["Keypad".to_string()],
                    states: vec![StateDefinition {
                        text: "⏯️ Play".to_string(),
                        ..Default::default()
                    }],
                },
                ActionInfo {
                    uuid: "com.uzlinux.mpris-media.next".to_string(),
                    name: "Próxima Faixa".to_string(),
                    tooltip: Some("Avança para a próxima música/faixa".to_string()),
                    icon: None,
                    plugin: "com.uzlinux.mpris-media".to_string(),
                    property_inspector: String::new(),
                    controllers: vec!["Keypad".to_string()],
                    states: vec![StateDefinition {
                        text: "⏭️ Next".to_string(),
                        ..Default::default()
                    }],
                },
                ActionInfo {
                    uuid: "com.uzlinux.mpris-media.prev".to_string(),
                    name: "Faixa Anterior".to_string(),
                    tooltip: Some("Retorna para a música anterior".to_string()),
                    icon: None,
                    plugin: "com.uzlinux.mpris-media".to_string(),
                    property_inspector: String::new(),
                    controllers: vec!["Keypad".to_string()],
                    states: vec![StateDefinition {
                        text: "⏮️ Prev".to_string(),
                        ..Default::default()
                    }],
                },
                ActionInfo {
                    uuid: "com.uzlinux.mpris-media.stop".to_string(),
                    name: "Parar Mídia".to_string(),
                    tooltip: Some("Interrompe a reprodução de mídia".to_string()),
                    icon: None,
                    plugin: "com.uzlinux.mpris-media".to_string(),
                    property_inspector: String::new(),
                    controllers: vec!["Keypad".to_string()],
                    states: vec![StateDefinition {
                        text: "⏹️ Stop".to_string(),
                        ..Default::default()
                    }],
                },
                ActionInfo {
                    uuid: "com.uzlinux.mpris-media.volup".to_string(),
                    name: "Volume do Player +5%".to_string(),
                    tooltip: Some("Aumenta o volume do player ativo em 5%".to_string()),
                    icon: None,
                    plugin: "com.uzlinux.mpris-media".to_string(),
                    property_inspector: String::new(),
                    controllers: vec!["Keypad".to_string()],
                    states: vec![StateDefinition {
                        text: "🔊 Player +".to_string(),
                        ..Default::default()
                    }],
                },
                ActionInfo {
                    uuid: "com.uzlinux.mpris-media.voldown".to_string(),
                    name: "Volume do Player -5%".to_string(),
                    tooltip: Some("Diminui o volume do player ativo em 5%".to_string()),
                    icon: None,
                    plugin: "com.uzlinux.mpris-media".to_string(),
                    property_inspector: String::new(),
                    controllers: vec!["Keypad".to_string()],
                    states: vec![StateDefinition {
                        text: "🔉 Player -".to_string(),
                        ..Default::default()
                    }],
                },
                ActionInfo {
                    uuid: "com.uzlinux.mpris-media.seekforward".to_string(),
                    name: "Avançar 10s".to_string(),
                    tooltip: Some("Avança 10 segundos na faixa atual".to_string()),
                    icon: None,
                    plugin: "com.uzlinux.mpris-media".to_string(),
                    property_inspector: String::new(),
                    controllers: vec!["Keypad".to_string()],
                    states: vec![StateDefinition {
                        text: "⏩ +10s".to_string(),
                        ..Default::default()
                    }],
                },
                ActionInfo {
                    uuid: "com.uzlinux.mpris-media.seekbackward".to_string(),
                    name: "Retroceder 10s".to_string(),
                    tooltip: Some("Retrocede 10 segundos na faixa atual".to_string()),
                    icon: None,
                    plugin: "com.uzlinux.mpris-media".to_string(),
                    property_inspector: String::new(),
                    controllers: vec!["Keypad".to_string()],
                    states: vec![StateDefinition {
                        text: "⏪ -10s".to_string(),
                        ..Default::default()
                    }],
                },
                ActionInfo {
                    uuid: "com.uzlinux.mpris-media.shuffle".to_string(),
                    name: "Alternar Aleatório".to_string(),
                    tooltip: Some("Ativa ou desativa a reprodução aleatória".to_string()),
                    icon: None,
                    plugin: "com.uzlinux.mpris-media".to_string(),
                    property_inspector: String::new(),
                    controllers: vec!["Keypad".to_string()],
                    states: vec![StateDefinition {
                        text: "🔀 Shuffle".to_string(),
                        ..Default::default()
                    }],
                },
                ActionInfo {
                    uuid: "com.uzlinux.mpris-media.repeat".to_string(),
                    name: "Alternar Repetição".to_string(),
                    tooltip: Some("Alterna modo de repetição (Desativado / Playlist / Faixa)".to_string()),
                    icon: None,
                    plugin: "com.uzlinux.mpris-media".to_string(),
                    property_inspector: String::new(),
                    controllers: vec!["Keypad".to_string()],
                    states: vec![StateDefinition {
                        text: "🔁 Repeat".to_string(),
                        ..Default::default()
                    }],
                },
            ],
        });
    }

    if !disabled.contains(&"com.uzlinux.pipewire-audio".to_string()) {
        cats.push(CategoryInfo {
            name: "🔊 Áudio Linux (PipeWire / Pré-instalado)".to_string(),
            icon: None,
            actions: vec![
                ActionInfo {
                    uuid: "com.uzlinux.pipewire-audio.volume".to_string(),
                    name: "Volume Master".to_string(),
                    tooltip: Some("Controla o volume master do sistema (Aumentar, Diminuir ou Mudo)".to_string()),
                    icon: None,
                    plugin: "com.uzlinux.pipewire-audio".to_string(),
                    property_inspector: "native:pipewire:volume".to_string(),
                    controllers: vec!["Keypad".to_string(), "Encoder".to_string()],
                    states: vec![StateDefinition {
                        text: "🔊 Master".to_string(),
                        ..Default::default()
                    }],
                },
                ActionInfo {
                    uuid: "com.uzlinux.pipewire-audio.micvolume".to_string(),
                    name: "Volume do Microfone".to_string(),
                    tooltip: Some("Controla o volume do microfone padrão (Aumentar, Diminuir ou Mudo)".to_string()),
                    icon: None,
                    plugin: "com.uzlinux.pipewire-audio".to_string(),
                    property_inspector: "native:pipewire:micvolume".to_string(),
                    controllers: vec!["Keypad".to_string(), "Encoder".to_string()],
                    states: vec![StateDefinition {
                        text: "🎤 Mic Vol".to_string(),
                        ..Default::default()
                    }],
                },
                ActionInfo {
                    uuid: "com.uzlinux.pipewire-audio.outputvolume".to_string(),
                    name: "Volume de Saída Específica".to_string(),
                    tooltip: Some("Controla o volume de um dispositivo de saída selecionado".to_string()),
                    icon: None,
                    plugin: "com.uzlinux.pipewire-audio".to_string(),
                    property_inspector: "native:pipewire:outputvolume".to_string(),
                    controllers: vec!["Keypad".to_string(), "Encoder".to_string()],
                    states: vec![StateDefinition {
                        text: "🎧 Saída Vol".to_string(),
                        ..Default::default()
                    }],
                },
                ActionInfo {
                    uuid: "com.uzlinux.pipewire-audio.inputvolume".to_string(),
                    name: "Volume de Entrada Específica".to_string(),
                    tooltip: Some("Controla o volume de um microfone/entrada selecionado".to_string()),
                    icon: None,
                    plugin: "com.uzlinux.pipewire-audio".to_string(),
                    property_inspector: "native:pipewire:inputvolume".to_string(),
                    controllers: vec!["Keypad".to_string(), "Encoder".to_string()],
                    states: vec![StateDefinition {
                        text: "🎙️ Entrada Vol".to_string(),
                        ..Default::default()
                    }],
                },
                ActionInfo {
                    uuid: "com.uzlinux.pipewire-audio.appvolume".to_string(),
                    name: "Volume de Aplicativo".to_string(),
                    tooltip: Some("Controla o volume de um aplicativo específico em execução".to_string()),
                    icon: None,
                    plugin: "com.uzlinux.pipewire-audio".to_string(),
                    property_inspector: "native:pipewire:appvolume".to_string(),
                    controllers: vec!["Keypad".to_string(), "Encoder".to_string()],
                    states: vec![StateDefinition {
                        text: "🎵 App Vol".to_string(),
                        ..Default::default()
                    }],
                },
                ActionInfo {
                    uuid: "com.uzlinux.pipewire-audio.switchoutput".to_string(),
                    name: "Alternar Dispositivo de Saída".to_string(),
                    tooltip: Some("Alterna para um dispositivo de áudio de saída ou circula entre eles".to_string()),
                    icon: None,
                    plugin: "com.uzlinux.pipewire-audio".to_string(),
                    property_inspector: "native:pipewire:switchoutput".to_string(),
                    controllers: vec!["Keypad".to_string(), "Encoder".to_string()],
                    states: vec![StateDefinition {
                        text: "🎧 Saída".to_string(),
                        ..Default::default()
                    }],
                },
                ActionInfo {
                    uuid: "com.uzlinux.pipewire-audio.switchinput".to_string(),
                    name: "Alternar Dispositivo de Entrada".to_string(),
                    tooltip: Some("Alterna para um microfone específico ou circula entre eles".to_string()),
                    icon: None,
                    plugin: "com.uzlinux.pipewire-audio".to_string(),
                    property_inspector: "native:pipewire:switchinput".to_string(),
                    controllers: vec!["Keypad".to_string(), "Encoder".to_string()],
                    states: vec![StateDefinition {
                        text: "🎙️ Entrada".to_string(),
                        ..Default::default()
                    }],
                },
                ActionInfo {
                    uuid: "com.uzlinux.pipewire-audio.pushtotalk".to_string(),
                    name: "Push to Talk (Pressione para Falar)".to_string(),
                    tooltip: Some("Desmuta o microfone apenas enquanto segura a tecla".to_string()),
                    icon: None,
                    plugin: "com.uzlinux.pipewire-audio".to_string(),
                    property_inspector: "native:pipewire:pushtotalk".to_string(),
                    controllers: vec!["Keypad".to_string()],
                    states: vec![StateDefinition {
                        text: "📻 PTT".to_string(),
                        ..Default::default()
                    }],
                },
            ],
        });
    }

    // Ulanzi D200 Hardware Driver (Parte integrante do sistema, não é plugin e não pode ser desativado)
    cats.push(CategoryInfo {
        name: "🎛️ Ulanzi D200 (Hardware Integrado)".to_string(),
        icon: None,
        actions: vec![
            ActionInfo {
                uuid: "com.uzlinux.ulanzi-d200.display_sleep".to_string(),
                name: "Desligar / Ligar Telas do Deck".to_string(),
                tooltip: Some("Alterna o estado de repouso das telas e LEDs do Ulanzi D200".to_string()),
                icon: None,
                plugin: "com.uzlinux.ulanzi-d200".to_string(),
                property_inspector: String::new(),
                controllers: vec!["Keypad".to_string()],
                states: vec![StateDefinition {
                    text: "Tela Sleep".to_string(),
                    ..Default::default()
                }],
            },
            ActionInfo {
                uuid: "com.uzlinux.ulanzi-d200.cycle_brightness".to_string(),
                name: "Alternar Nível de Brilho".to_string(),
                tooltip: Some("Alterna rapidamente os níveis de brilho do dispositivo".to_string()),
                icon: None,
                plugin: "com.uzlinux.ulanzi-d200".to_string(),
                property_inspector: String::new(),
                controllers: vec!["Keypad".to_string()],
                states: vec![StateDefinition {
                    text: "Brilho Ciclo".to_string(),
                    ..Default::default()
                }],
            },
        ],
    });

    cats
}

#[tauri::command]
pub async fn get_installed_applications() -> Result<Vec<crate::desktop_apps::DesktopApp>, String> {
    Ok(crate::desktop_apps::get_installed_desktop_apps())
}

#[tauri::command]
pub async fn get_opendeck_scan_info(state: State<'_, Arc<AppState>>) -> Result<OpenDeckScanResult, String> {
    state.config_manager.scan_opendeck().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn import_opendeck_custom(
    options: OpenDeckImportOptions,
    state: State<'_, Arc<AppState>>,
) -> Result<OpenDeckImportReport, String> {
    state.config_manager.import_from_opendeck_custom(&options).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn import_opendeck_data(state: State<'_, Arc<AppState>>) -> Result<String, String> {
    let (profiles, plugins) = state.config_manager.import_from_opendeck()
        .map_err(|e| e.to_string())?;
    Ok(format!("Importação concluída! {} perfis e {} plugins copiados do OpenDeck para o Uzlinux Studio.", profiles, plugins))
}

#[tauri::command]
pub async fn trigger_virtual_press(
    device_id: String,
    key_index: u8,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    if let Some(deck) = state.device_manager.get_device(&device_id) {
        // Send key down and key up via event sender
        let cols = deck.info().columns;
        let row = key_index / cols;
        let col = key_index % cols;

        let tx = state.device_manager.event_sender();
        let _ = tx.send((
            device_id.clone(),
            uzlinux_device_driver::DeviceEvent::KeyDown {
                row,
                column: col,
                key_index,
            },
        ));

        tokio::time::sleep(std::time::Duration::from_millis(80)).await;

        let _ = tx.send((
            device_id,
            uzlinux_device_driver::DeviceEvent::KeyUp {
                row,
                column: col,
                key_index,
            },
        ));
    }
    Ok(())
}

#[tauri::command]
pub async fn get_settings(state: State<'_, Arc<AppState>>) -> Result<AppSettings, String> {
    state.config_manager.load_settings().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_settings(
    settings: AppSettings,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    state.config_manager.save_settings(&settings).map_err(|e| e.to_string())?;

    // Apply brightness to all connected devices
    for device_id in state.device_manager.list_device_ids() {
        state.send_brightness_to_device(&device_id, settings.brightness);
    }

    Ok(())
}

#[tauri::command]
pub async fn get_obs_settings(state: State<'_, Arc<AppState>>) -> Result<serde_json::Value, String> {
    let mut val = state.config_manager.load_plugin_settings("dev.theca11.multiobs")
        .unwrap_or(serde_json::json!({}));
    if val.get("ip1").is_none() {
        val["ip1"] = serde_json::Value::String("127.0.0.1".to_string());
    }
    if val.get("port1").is_none() {
        val["port1"] = serde_json::Value::String("4455".to_string());
    }
    if val.get("pwd1").is_none() {
        val["pwd1"] = serde_json::Value::String("".to_string());
    }
    Ok(val)
}

#[tauri::command]
pub async fn save_obs_settings(
    settings: serde_json::Value,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    state.config_manager.save_plugin_settings("dev.theca11.multiobs", &settings)
        .map_err(|e| e.to_string())?;

    // Notify plugin via didReceiveGlobalSettings
    let outbound = OutboundEvent::DidReceiveGlobalSettings(uzlinux_protocol::DidReceiveGlobalSettingsMessage {
        payload: uzlinux_protocol::GlobalSettingsPayload {
            settings: settings.clone(),
        },
    });
    state.plugin_hub.send_to_plugin("dev.theca11.multiobs", &outbound);

    Ok(())
}

#[tauri::command]
pub async fn test_obs_connection(
    host: String,
    port: u16,
) -> Result<String, String> {
    let addr = format!("{}:{}", host, port);
    match tokio::time::timeout(
        std::time::Duration::from_millis(2500),
        tokio::net::TcpStream::connect(&addr),
    ).await {
        Ok(Ok(_)) => Ok(format!("Conexão bem sucedida com o OBS Studio em {}!", addr)),
        Ok(Err(e)) => Err(format!("Não foi possível conectar em {}: {}. Verifique se o OBS Studio está aberto e se o WebSocket está ativado.", addr, e)),
        Err(_) => Err(format!("Tempo limite excedido ao tentar conectar em {}. Verifique o IP e a porta.", addr)),
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PluginStatusInfo {
    pub uuid: String,
    pub name: String,
    pub author: String,
    pub version: String,
    pub description: String,
    pub icon: Option<String>,
    pub category: Option<String>,
    pub dir_path: String,
    pub plugin_type: String, // "native", "node", "html", "builtin"
    pub is_running: bool,
    pub is_connected: bool,
    pub has_executable: bool,
    pub executable_path: Option<String>,
    pub actions_count: usize,
    pub is_enabled: bool,
    pub is_builtin: bool,
}

#[tauri::command]
pub async fn get_plugins_list(state: State<'_, Arc<AppState>>) -> Result<Vec<PluginStatusInfo>, String> {
    let settings = state.config_manager.load_settings().unwrap_or_default();
    let mut list = Vec::new();

    // 1. Built-in Pre-installed Plugins
    list.push(PluginStatusInfo {
        uuid: "com.uzlinux.pipewire-audio".to_string(),
        name: "Áudio Linux (PipeWire / WirePlumber)".to_string(),
        author: "Uzlinux Team".to_string(),
        version: "1.0.0".to_string(),
        description: "Controle nativo de volume master, microfone, mudo, push-to-talk e alternância de dispositivos de som via PipeWire / PulseAudio.".to_string(),
        icon: None,
        category: Some("Áudio Linux".to_string()),
        dir_path: "builtin://pipewire-audio".to_string(),
        plugin_type: "builtin".to_string(),
        is_running: true,
        is_connected: true,
        has_executable: false,
        executable_path: None,
        actions_count: 11,
        is_enabled: !settings.disabled_plugins.contains(&"com.uzlinux.pipewire-audio".to_string()),
        is_builtin: true,
    });

    list.push(PluginStatusInfo {
        uuid: "com.uzlinux.mpris-media".to_string(),
        name: "Player de Mídia (MPRIS / D-Bus)".to_string(),
        author: "Uzlinux Team".to_string(),
        version: "1.0.0".to_string(),
        description: "Controle nativo de tocadores de áudio e vídeo (Spotify, VLC, YouTube, navegadores, mpv, etc.) via protocolo D-Bus MPRIS.".to_string(),
        icon: None,
        category: Some("Controle de Mídia".to_string()),
        dir_path: "builtin://mpris-media".to_string(),
        plugin_type: "builtin".to_string(),
        is_running: true,
        is_connected: true,
        has_executable: false,
        executable_path: None,
        actions_count: 10,
        is_enabled: !settings.disabled_plugins.contains(&"com.uzlinux.mpris-media".to_string()),
        is_builtin: true,
    });

    list.push(PluginStatusInfo {
        uuid: "com.uzlinux.system-starterpack".to_string(),
        name: "Ferramentas do Sistema & Atalhos".to_string(),
        author: "Uzlinux Team".to_string(),
        version: "1.0.0".to_string(),
        description: "Ações essenciais nativas: abrir aplicativos, comandos de terminal, URLs, atalhos de teclado simulados, troca de perfil e controle de brilho.".to_string(),
        icon: None,
        category: Some("Ferramentas do Sistema".to_string()),
        dir_path: "builtin://system-starterpack".to_string(),
        plugin_type: "builtin".to_string(),
        is_running: true,
        is_connected: true,
        has_executable: false,
        executable_path: None,
        actions_count: 8,
        is_enabled: !settings.disabled_plugins.contains(&"com.uzlinux.system-starterpack".to_string()),
        is_builtin: true,
    });

    let plugins = PluginDiscovery::scan_directory(state.config_manager.plugins_dir())
        .map_err(|e| e.to_string())?;

    for plugin in plugins {
        if plugin.uuid == "com.amansprojects.starterpack" || plugin.uuid == "com.glmagalhaes.ulanzi.d200" {
            // Handled natively by pre-installed Uzlinux plugins / core hardware driver
            continue;
        }

        let exec_rel = plugin.manifest.get_linux_executable();
        let plugin_type = match &exec_rel {
            Some(path) if path.ends_with(".html") || path.ends_with(".htm") => "html",
            Some(path) if path.ends_with(".js") => "node",
            Some(_) => "native",
            None => "unknown",
        }.to_string();

        let is_running = state.process_manager.is_running(&plugin.uuid);
        let is_connected = state.plugin_hub.is_plugin_connected(&plugin.uuid);

        let icon_url = plugin.manifest.icon.as_ref().map(|ic| {
            let clean = ic.trim_start_matches('/');
            format!("plugins/{}/{}", plugin.uuid, clean)
        });

        let is_enabled = !settings.disabled_plugins.contains(&plugin.uuid);

        list.push(PluginStatusInfo {
            uuid: plugin.uuid.clone(),
            name: plugin.manifest.name.clone(),
            author: plugin.manifest.author.clone(),
            version: plugin.manifest.version.clone(),
            description: plugin.manifest.description.clone(),
            icon: icon_url,
            category: plugin.manifest.category.clone(),
            dir_path: plugin.dir_path.to_string_lossy().to_string(),
            plugin_type,
            is_running,
            is_connected,
            has_executable: exec_rel.is_some(),
            executable_path: exec_rel,
            actions_count: plugin.manifest.actions.len(),
            is_enabled,
            is_builtin: false,
        });
    }

    Ok(list)
}

#[tauri::command]
pub async fn set_plugin_enabled(
    uuid: String,
    enabled: bool,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    if uuid == "com.uzlinux.ulanzi-d200" {
        return Err("O Ulanzi D200 é o driver de hardware do dispositivo e faz parte integrante do aplicativo, não podendo ser desativado.".to_string());
    }

    let mut settings = state.config_manager.load_settings().unwrap_or_default();
    if enabled {
        settings.disabled_plugins.retain(|id| id != &uuid);
        if !uuid.starts_with("com.uzlinux.") {
            let _ = start_plugin(uuid.clone(), state.clone()).await;
        }
    } else {
        if !settings.disabled_plugins.contains(&uuid) {
            settings.disabled_plugins.push(uuid.clone());
        }
        if !uuid.starts_with("com.uzlinux.") {
            let _ = stop_plugin(uuid.clone(), state.clone()).await;
        }
    }
    state.config_manager.save_settings(&settings).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn start_plugin(uuid: String, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let plugins = PluginDiscovery::scan_directory(state.config_manager.plugins_dir())
        .map_err(|e| e.to_string())?;

    let plugin = plugins.iter().find(|p| p.uuid == uuid || p.uuid.trim_end_matches(".sdPlugin") == uuid.trim_end_matches(".sdPlugin"))
        .ok_or_else(|| format!("Plugin '{}' não encontrado", uuid))?;

    let exec_rel = plugin.manifest.get_linux_executable()
        .ok_or_else(|| format!("Nenhum executável Linux encontrado para o plugin '{}'", uuid))?;

    if exec_rel.ends_with(".html") || exec_rel.ends_with(".htm") {
        return Ok(());
    }

    let plugin_info = uzlinux_protocol::PluginInfo {
        application: uzlinux_protocol::ApplicationInfo {
            font: "Liberation Sans".to_string(),
            language: "pt_BR".to_string(),
            platform: "linux".to_string(),
            platform_version: "6.0".to_string(),
            version: "2.14.0".to_string(),
        },
        plugin: uzlinux_protocol::PluginMeta {
            uuid: plugin.uuid.clone(),
            version: plugin.manifest.version.clone(),
        },
        device_pixel_ratio: 1,
        devices: vec![],
        colors: Default::default(),
    };

    state.process_manager.start_plugin(
        &plugin.uuid,
        &plugin.dir_path,
        &exec_rel,
        state.ports.plugin_ws,
        &plugin_info,
    ).await.map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn stop_plugin(uuid: String, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state.process_manager.stop_plugin(&uuid).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn restart_plugin(uuid: String, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let _ = state.process_manager.stop_plugin(&uuid).await;
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    start_plugin(uuid, state).await
}

#[tauri::command]
pub async fn get_plugin_logs(uuid: String, state: State<'_, Arc<AppState>>) -> Result<Vec<String>, String> {
    Ok(state.process_manager.get_logs(&uuid))
}

#[tauri::command]
pub async fn open_plugin_directory(uuid: Option<String>, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let path = match uuid {
        Some(u) if !u.is_empty() => {
            let plugins_dir = state.config_manager.plugins_dir();
            let candidate1 = plugins_dir.join(&u);
            let candidate2 = plugins_dir.join(format!("{}.sdPlugin", u));
            if candidate1.exists() {
                candidate1
            } else if candidate2.exists() {
                candidate2
            } else {
                plugins_dir
            }
        }
        _ => state.config_manager.plugins_dir(),
    };

    let _ = std::process::Command::new("xdg-open")
        .arg(&path)
        .spawn();

    Ok(())
}

#[tauri::command]
pub async fn install_plugin(file_path: String, state: State<'_, Arc<AppState>>) -> Result<String, String> {
    let path = std::path::Path::new(&file_path);
    if !path.exists() {
        return Err(format!("Arquivo não encontrado: {}", file_path));
    }

    let installed_dir = uzlinux_plugin_manager::PluginInstaller::install_package(
        path,
        state.config_manager.plugins_dir(),
    ).map_err(|e| format!("Erro ao instalar pacote: {}", e))?;

    Ok(format!("Plugin instalado com sucesso em {:?}", installed_dir.file_name().unwrap_or_default()))
}

#[tauri::command]
pub async fn uninstall_plugin(uuid: String, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    if uuid.starts_with("com.uzlinux.") || uuid == "com.glmagalhaes.ulanzi.d200" || uuid == "com.amansprojects.starterpack" {
        return Err("Plugins nativos pré-instalados não podem ser desinstalados, apenas desativados.".to_string());
    }

    let _ = state.process_manager.stop_plugin(&uuid).await;

    let plugins_dir = state.config_manager.plugins_dir();
    let dir = if plugins_dir.join(&uuid).exists() {
        plugins_dir.join(&uuid)
    } else if plugins_dir.join(format!("{}.sdPlugin", uuid)).exists() {
        plugins_dir.join(format!("{}.sdPlugin", uuid))
    } else {
        return Err(format!("Diretório do plugin '{}' não encontrado", uuid));
    };

    uzlinux_plugin_manager::PluginInstaller::uninstall(&dir)
        .map_err(|e| format!("Erro ao remover plugin: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn notify_property_inspector_did_appear(
    device_id: String,
    key_index: u8,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    if let Some((_, profile)) = state.get_active_profile(&device_id) {
        if let Some(Some(ref key)) = profile.keys.get(key_index as usize) {
            let ev = OutboundEvent::PropertyInspectorDidAppear(uzlinux_protocol::ContextActionMessage {
                action: key.action.uuid.clone(),
                context: key.context.clone(),
                device: device_id.clone(),
            });
            state.plugin_hub.send_to_plugin(&key.action.plugin, &ev);

            // Also send willAppear to the plugin! Many OpenAction/OpenDeck plugins (like oadesktopentry)
            // push their initial Property Inspector state (like apps list) during willAppear!
            let will_appear_ev = OutboundEvent::WillAppear(uzlinux_protocol::AppearanceMessage {
                action: key.action.uuid.clone(),
                context: key.context.clone(),
                device: device_id.clone(),
                payload: uzlinux_protocol::AppearancePayload {
                    settings: key.settings.clone(),
                    coordinates: uzlinux_protocol::Coordinates {
                        column: key_index % 5,
                        row: key_index / 5,
                    },
                    state: key.current_state,
                    is_in_multi_action: false,
                    controller: "Keypad".to_string(),
                },
            });
            state.plugin_hub.send_to_plugin(&key.action.plugin, &will_appear_ev);

            // SPECIAL ENHANCEMENT FOR me.amankhanna.oadesktopentry:
            // Ensure the Property Inspector always has the complete, up-to-date system applications list
            if key.action.uuid == "me.amankhanna.oadesktopentry.launchapp" || key.action.plugin.contains("oadesktopentry") {
                let apps = crate::desktop_apps::get_installed_desktop_apps();
                let apps_json: Vec<serde_json::Value> = apps.into_iter().map(|a| {
                    serde_json::json!({
                        "path": a.path,
                        "name": a.name,
                        "exec": a.exec,
                        "icon": a.icon,
                        "terminal": a.terminal,
                    })
                }).collect();

                let pi_apps_msg = serde_json::json!({
                    "action": key.action.uuid,
                    "event": "sendToPropertyInspector",
                    "context": key.context,
                    "payload": {
                        "apps": apps_json
                    }
                });
                state.pi_hub.send_to_property_inspector(&key.context, &pi_apps_msg);
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn notify_property_inspector_did_disappear(
    device_id: String,
    key_index: u8,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    if let Some((_, profile)) = state.get_active_profile(&device_id) {
        if let Some(Some(ref key)) = profile.keys.get(key_index as usize) {
            let ev = OutboundEvent::PropertyInspectorDidDisappear(uzlinux_protocol::ContextActionMessage {
                action: key.action.uuid.clone(),
                context: key.context.clone(),
                device: device_id.clone(),
            });
            state.plugin_hub.send_to_plugin(&key.action.plugin, &ev);
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn open_url(url: String) -> Result<(), String> {
    open::that(&url).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_key_settings(
    device_id: String,
    key_index: u8,
    settings: serde_json::Value,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let mut plugin_to_notify = None;
    let mut action_uuid = String::new();
    let mut context = String::new();

    if let Some(mut active) = state.active_profiles.get_mut(&device_id) {
        if let Some(Some(ref mut key)) = active.1.keys.get_mut(key_index as usize) {
            key.settings = settings.clone();
            plugin_to_notify = Some(key.action.plugin.clone());
            action_uuid = key.action.uuid.clone();
            context = key.context.clone();
            let _ = state.config_manager.save_profile(&device_id, &active.0, &active.1);
        }
    }

    state.emit_to_frontend("key_settings_changed", serde_json::json!({
        "deviceId": device_id,
        "keyIndex": key_index,
        "settings": settings,
    }));

    if let Some(plugin) = plugin_to_notify {
        let is_starterpack = plugin == "com.amansprojects.starterpack.sdPlugin"
            || plugin == "com.amansprojects.starterpack"
            || action_uuid.starts_with("com.amansprojects.starterpack.");
        if plugin != "uzlinux.native" && !is_starterpack {
            let did_receive = OutboundEvent::DidReceiveSettings(uzlinux_protocol::DidReceiveSettingsMessage {
                action: action_uuid.clone(),
                context: context.clone(),
                device: device_id.clone(),
                payload: uzlinux_protocol::SettingsPayload {
                    settings: settings.clone(),
                    coordinates: uzlinux_protocol::Coordinates {
                        column: key_index % 5,
                        row: key_index / 5,
                    },
                    is_in_multi_action: false,
                },
            });
            state.plugin_hub.send_to_plugin(&plugin, &did_receive);
        }
    }

    state.pi_hub.send_to_property_inspector(&context, &serde_json::json!({
        "event": "didReceiveSettings",
        "action": action_uuid,
        "context": context,
        "device": device_id,
        "payload": {
            "settings": settings,
            "coordinates": {
                "column": key_index % 5,
                "row": key_index / 5,
            },
            "isInMultiAction": false
        }
    }));

    if let Some((_, profile)) = state.get_active_profile(&device_id) {
        state.render_all_keys_for_device(&device_id, &profile).await;
    }

    Ok(())
}

#[tauri::command]
pub async fn get_application_profiles(
    state: State<'_, Arc<AppState>>,
) -> Result<std::collections::HashMap<String, std::collections::HashMap<String, String>>, String> {
    state.config_manager.load_applications()
        .map(|apps| apps.map)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_application_profile(
    app_id: String,
    device_id: String,
    profile_name: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let mut apps = state.config_manager.load_applications().unwrap_or_default();
    apps.set_profile_for_app(app_id, device_id, profile_name);
    state.config_manager.save_applications(&apps)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_application_profile(
    app_id: String,
    device_id: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let mut apps = state.config_manager.load_applications().unwrap_or_default();
    if let Some(dev) = device_id {
        apps.remove_profile_for_device(&app_id, &dev);
    } else {
        apps.remove_app(&app_id);
    }
    state.config_manager.save_applications(&apps)
        .map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipeWireDeviceNode {
    pub id: u64,
    pub name: String,
    #[serde(rename = "nodeName")]
    pub node_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipeWireAppNode {
    pub id: u64,
    pub name: String,
    #[serde(rename = "appName")]
    pub app_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipeWireAudioInfo {
    pub sinks: Vec<PipeWireDeviceNode>,
    pub sources: Vec<PipeWireDeviceNode>,
    pub apps: Vec<PipeWireAppNode>,
    #[serde(rename = "defaultSink")]
    pub default_sink: Option<String>,
    #[serde(rename = "defaultSource")]
    pub default_source: Option<String>,
}

#[tauri::command]
pub async fn get_pipewire_audio_devices() -> Result<PipeWireAudioInfo, String> {
    let mut sinks = Vec::new();
    let mut sources = Vec::new();
    let mut apps = Vec::new();

    // 1. Try pw-dump
    if let Ok(output) = std::process::Command::new("pw-dump").output() {
        if output.status.success() {
            if let Ok(json_str) = String::from_utf8(output.stdout) {
                if let Some(start_idx) = json_str.find('[') {
                    if let Ok(nodes) = serde_json::from_str::<serde_json::Value>(&json_str[start_idx..]) {
                        if let Some(arr) = nodes.as_array() {
                            for item in arr {
                                if item.get("type").and_then(|t| t.as_str()) == Some("PipeWire:Interface:Node") {
                                    if let Some(props) = item.get("info").and_then(|i| i.get("props")) {
                                        let media_class = props.get("media.class").and_then(|c| c.as_str()).unwrap_or("");
                                        let id = item.get("id").and_then(|id| id.as_u64()).unwrap_or(0);
                                        let node_name = props.get("node.name").and_then(|n| n.as_str()).unwrap_or("").to_string();

                                        let display_name = props.get("node.nick").and_then(|n| n.as_str())
                                            .or_else(|| props.get("node.description").and_then(|d| d.as_str()))
                                            .or_else(|| props.get("node.name").and_then(|n| n.as_str()))
                                            .unwrap_or("Dispositivo")
                                            .to_string();

                                        if media_class == "Audio/Sink" {
                                            sinks.push(PipeWireDeviceNode {
                                                id,
                                                name: display_name,
                                                node_name,
                                            });
                                        } else if media_class == "Audio/Source" {
                                            sources.push(PipeWireDeviceNode {
                                                id,
                                                name: display_name,
                                                node_name,
                                            });
                                        } else if media_class == "Stream/Output/Audio" {
                                            let app_title = props.get("application.name").and_then(|a| a.as_str())
                                                .or_else(|| props.get("node.name").and_then(|n| n.as_str()))
                                                .unwrap_or("Aplicativo")
                                                .to_string();
                                            let app_name = props.get("application.name").and_then(|a| a.as_str()).map(|s| s.to_string());
                                            apps.push(PipeWireAppNode {
                                                id,
                                                name: app_title,
                                                app_name,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. Fallback via pactl if sinks or sources are empty
    if sinks.is_empty() {
        if let Ok(output) = std::process::Command::new("pactl").args(["list", "short", "sinks"]).output() {
            let out_str = String::from_utf8_lossy(&output.stdout);
            for line in out_str.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let id = parts[0].parse::<u64>().unwrap_or(0);
                    let node_name = parts[1].to_string();
                    let name = node_name.clone();
                    sinks.push(PipeWireDeviceNode { id, name, node_name });
                }
            }
        }
    }

    if sources.is_empty() {
        if let Ok(output) = std::process::Command::new("pactl").args(["list", "short", "sources"]).output() {
            let out_str = String::from_utf8_lossy(&output.stdout);
            for line in out_str.lines() {
                if line.contains(".monitor") { continue; }
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let id = parts[0].parse::<u64>().unwrap_or(0);
                    let node_name = parts[1].to_string();
                    let name = node_name.clone();
                    sources.push(PipeWireDeviceNode { id, name, node_name });
                }
            }
        }
    }

    // Deduplicate apps by app_name
    let mut unique_apps = Vec::new();
    let mut seen_apps = std::collections::HashSet::new();
    for a in apps {
        let key = a.app_name.clone().unwrap_or_else(|| a.name.clone());
        if seen_apps.insert(key) {
            unique_apps.push(a);
        }
    }

    // 3. Current defaults
    let default_sink = std::process::Command::new("pactl")
        .arg("get-default-sink")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let default_source = std::process::Command::new("pactl")
        .arg("get-default-source")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    Ok(PipeWireAudioInfo {
        sinks,
        sources,
        apps: unique_apps,
        default_sink,
        default_source,
    })
}

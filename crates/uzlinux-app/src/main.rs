use std::sync::Arc;
use std::time::Duration;
use anyhow::{Context, Result};
use log::{info, warn};

use uzlinux_app::state::AppState;
use uzlinux_app::router::EventRouter;
use uzlinux_core::models::DeviceInfo;
use uzlinux_protocol::{ApplicationInfo, DeviceDimensions, DeviceRegistrationInfo, PluginInfo, PluginMeta};
use uzlinux_core::storage::ConfigManager;
use uzlinux_device_driver::DeviceManager;
use uzlinux_plugin_manager::PluginDiscovery;
use uzlinux_server::{AssetServer, PluginHub, PropertyInspectorHub, ServerPorts};
use uzlinux_window_watcher::WindowWatcher;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

fn main() -> Result<()> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    info!("=======================================================");
    info!("       Uzlinux Studio - High-Performance Rust Deck Core ");
    info!("=======================================================");

    // 1. Discover configuration directory (seamless interop with OpenDeck)
    let config_manager = ConfigManager::discover();
    config_manager.ensure_directories()
        .context("Failed to ensure config directories")?;
    info!("Loaded config from: {:?}", config_manager.base_dir());

    let settings = config_manager.load_settings()
        .unwrap_or_default();
    info!("Settings loaded (language='{}', brightness={})", settings.language, settings.brightness);

    // 2. Allocate server ports dynamically
    let ports = ServerPorts::allocate(57116)
        .context("Failed to allocate server ports")?;

    // 3. Initialize WebSocket Hubs and HTTP WebServer
    let (plugin_hub, plugin_rx) = PluginHub::new(ports.plugin_ws);
    let (pi_hub, pi_rx) = PropertyInspectorHub::new(ports.pi_ws);
    let asset_server = AssetServer::new(ports.http_assets, config_manager.base_dir());

    // 4. Initialize Device Manager
    let (device_manager, device_rx) = DeviceManager::new();

    // Register primary target device: Ulanzi D200 (e9-02C53A015U3671380)
    let ulanzi_info = DeviceInfo::new_ulanzi_d200("02C53A015U3671380");
    let _ulanzi_deck = device_manager.add_virtual_deck(ulanzi_info.clone());
    info!("Device registered: {} ({})", ulanzi_info.name, ulanzi_info.id);

    // 5. Initialize Shared Application State
    let app_state = Arc::new(AppState::new(
        config_manager.clone(),
        ports,
        plugin_hub.clone(),
        pi_hub.clone(),
        asset_server.clone(),
        device_manager,
    ));

    // 6. Load Initial Profile for the Device
    let initial_profile_name = config_manager.get_device_selected_profile(&ulanzi_info.id)
        .unwrap_or_else(|_| "Default".to_string());
    info!("Initial profile for {}: '{}'", ulanzi_info.id, initial_profile_name);

    if let Ok(profile) = config_manager.load_profile(&ulanzi_info.id, &initial_profile_name) {
        app_state.set_active_profile(&ulanzi_info.id, &initial_profile_name, profile);
    } else {
        warn!("Could not load initial profile '{}', loading fallback.", initial_profile_name);
    }

    // Pre-warm desktop applications & icon index in background for instant UI response
    std::thread::spawn(|| {
        let _ = uzlinux_app::desktop_apps::get_installed_desktop_apps();
    });

    // 7. Spawn Asynchronous Background Runtime (Servers, Plugins, Routers)
    let async_state = app_state.clone();
    let async_ulanzi_info = ulanzi_info.clone();
    let async_settings = settings.clone();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create Tokio runtime");

        rt.block_on(async move {
            if let Err(e) = async_state.plugin_hub.start().await {
                warn!("Plugin Hub error: {}", e);
            }
            if let Err(e) = async_state.pi_hub.start().await {
                warn!("PI Hub error: {}", e);
            }
            if let Err(e) = async_state.asset_server.start() {
                warn!("Asset Server error: {}", e);
            }

            // Window Watcher
            let window_watcher = WindowWatcher::new(Duration::from_millis(500));
            let window_rx = window_watcher.start();

            // Event Router
            let router = Arc::new(EventRouter::new(async_state.clone()));
            router.clone().start(plugin_rx, pi_rx, device_rx, window_rx);

            if let Some((_, ref profile)) = async_state.get_active_profile(&async_ulanzi_info.id) {
                router.render_all_keys(&async_ulanzi_info.id, profile).await;
            }

            // Plugins
            let installed_plugins = PluginDiscovery::scan_directory(async_state.config_manager.plugins_dir())
                .unwrap_or_default();
            info!("Discovered {} installed plugin(s)", installed_plugins.len());

            let plugin_info_template = PluginInfo {
                application: ApplicationInfo {
                    font: "Liberation Sans".to_string(),
                    language: async_settings.language.clone(),
                    platform: "linux".to_string(),
                    platform_version: "6.x".to_string(),
                    version: "2.14.0".to_string(),
                },
                plugin: PluginMeta {
                    uuid: String::new(),
                    version: "1.0.0".to_string(),
                },
                device_pixel_ratio: 1,
                devices: vec![
                    DeviceRegistrationInfo {
                        id: async_ulanzi_info.id.clone(),
                        name: async_ulanzi_info.name.clone(),
                        size: DeviceDimensions {
                            columns: async_ulanzi_info.columns,
                            rows: async_ulanzi_info.rows,
                        },
                        device_type: 0,
                    }
                ],
                colors: Default::default(),
            };

            let disabled_plugins = async_state.config_manager.load_settings().map(|s| s.disabled_plugins).unwrap_or_default();

            for plugin in &installed_plugins {
                if disabled_plugins.contains(&plugin.uuid) {
                    info!("Skipping disabled plugin '{}' during startup", plugin.uuid);
                    continue;
                }
                // Ignore external OpenDeck plugins that are now 100% NATIVE in Rust!
                if plugin.uuid == "com.amansprojects.starterpack" || plugin.uuid == "com.glmagalhaes.ulanzi.d200" {
                    info!("Skipping external OpenDeck plugin '{}': Handled NATIVELY in Rust core!", plugin.uuid);
                    continue;
                }
                if let Some(exec_rel) = plugin.manifest.get_linux_executable() {
                    if exec_rel.ends_with(".html") || exec_rel.ends_with(".htm") {
                        info!("Plugin '{}' uses HTML Webview runtime ({}), skipping process spawn", plugin.uuid, exec_rel);
                        continue;
                    }

                    let mut info_copy = plugin_info_template.clone();
                    info_copy.plugin.uuid = plugin.uuid.clone();
                    info_copy.plugin.version = plugin.manifest.version.clone();

                    let res = async_state.process_manager.start_plugin(
                        &plugin.uuid,
                        &plugin.dir_path,
                        &exec_rel,
                        async_state.ports.plugin_ws,
                        &info_copy,
                    ).await;

                    match res {
                        Ok(_) => info!("Started plugin process for '{}'", plugin.uuid),
                        Err(e) => warn!("Failed to start plugin process for '{}': {}", plugin.uuid, e),
                    }
                }
            }

            // Keep runtime alive
            loop {
                tokio::time::sleep(Duration::from_secs(3600)).await;
            }
        });
    });

    info!("Launching Desktop Graphical Interface (Tauri v2)...");

    let cleanup_state = app_state.clone();

    // 8. Launch Tauri v2 Desktop GUI with System Tray
    tauri::Builder::default()
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .manage(app_state)
        .setup(|app| {
            let handle = app.handle().clone();
            if let Some(state) = app.try_state::<Arc<AppState>>() {
                state.set_app_handle(handle);
            }

            let show_item = MenuItem::with_id(app, "show", "Abrir Uzlinux Studio", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Sair", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            let icon = app.default_window_icon().cloned().ok_or_else(|| {
                anyhow::anyhow!("No default window icon found")
            })?;

            let _tray = TrayIconBuilder::new()
                .icon(icon)
                .tooltip("Uzlinux Studio")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| {
                    match event.id.as_ref() {
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button, button_state, .. } = event {
                        if button == MouseButton::Left && button_state == MouseButtonState::Up {
                            let app = tray.app_handle();
                            if let Some(window) = app.get_webview_window("main") {
                                if window.is_visible().unwrap_or(false) {
                                    let _ = window.hide();
                                } else {
                                    let _ = window.show();
                                    let _ = window.set_focus();
                                }
                            }
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                if let Some(state) = app.try_state::<Arc<AppState>>() {
                    if let Ok(settings) = state.config_manager.load_settings() {
                        if settings.background {
                            api.prevent_close();
                            let _ = window.hide();
                            info!("Window hidden to system tray");
                        }
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            uzlinux_app::commands::get_devices,
            uzlinux_app::commands::get_port_base,
            uzlinux_app::commands::get_profiles,
            uzlinux_app::commands::get_selected_profile,
            uzlinux_app::commands::set_selected_profile,
            uzlinux_app::commands::get_profile_data,
            uzlinux_app::commands::save_profile_data,
            uzlinux_app::commands::create_profile,
            uzlinux_app::commands::delete_profile,
            uzlinux_app::commands::rename_profile,
            uzlinux_app::commands::get_categories,
            uzlinux_app::commands::trigger_virtual_press,
            uzlinux_app::commands::get_settings,
            uzlinux_app::commands::set_settings,
            uzlinux_app::commands::get_obs_settings,
            uzlinux_app::commands::save_obs_settings,
            uzlinux_app::commands::test_obs_connection,
            uzlinux_app::commands::get_plugins_list,
            uzlinux_app::commands::start_plugin,
            uzlinux_app::commands::stop_plugin,
            uzlinux_app::commands::restart_plugin,
            uzlinux_app::commands::get_plugin_logs,
            uzlinux_app::commands::open_plugin_directory,
            uzlinux_app::commands::install_plugin,
            uzlinux_app::commands::uninstall_plugin,
            uzlinux_app::commands::set_plugin_enabled,
            uzlinux_app::commands::notify_property_inspector_did_appear,
            uzlinux_app::commands::notify_property_inspector_did_disappear,
            uzlinux_app::commands::get_installed_applications,
            uzlinux_app::commands::get_opendeck_scan_info,
            uzlinux_app::commands::import_opendeck_custom,
            uzlinux_app::commands::import_opendeck_data,
            uzlinux_app::commands::open_url,
            uzlinux_app::commands::save_key_settings,
            uzlinux_app::commands::get_application_profiles,
            uzlinux_app::commands::set_application_profile,
            uzlinux_app::commands::remove_application_profile,
            uzlinux_app::commands::get_pipewire_audio_devices,
        ])
        .run(tauri::generate_context!())
        .expect("Error while running Tauri application");

    // Cleanup on window exit
    info!("Tauri window closed. Stopping plugins...");
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        cleanup_state.process_manager.stop_all().await;
    });

    info!("Uzlinux Studio shutdown complete.");
    Ok(())
}

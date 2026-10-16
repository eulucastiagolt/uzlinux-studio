mod actions;
mod apps;
mod config;
mod device;

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use slint::{ComponentHandle, Model, SharedString, VecModel};

use crate::config::ProfileConfig;
use crate::device::{DeckEvent, UlanziDeck};

slint::include_modules!();

/// Helper to load a Slint image from a file path
fn load_slint_image(path_str: &str) -> (bool, slint::Image) {
    if path_str.trim().is_empty() {
        return (false, slint::Image::default());
    }
    let p = Path::new(path_str);
    if p.exists() {
        if let Ok(img) = slint::Image::load_from_path(p) {
            return (true, img);
        }
    }
    (false, slint::Image::default())
}

/// Prepares an icon for the Ulanzi Deck LCD:
/// - If SVG, rasterizes via `rsvg-convert`
/// - Loads via `image` crate, resizes to exactly 196x196 pixels
/// - Encodes to baseline RGB JPEG format (expected by hardware screen)
fn prepare_deck_icon(path_str: &str) -> Result<Vec<u8>, String> {
    let p = Path::new(path_str);
    if !p.exists() {
        return Err(format!("Arquivo de imagem não encontrado: {}", path_str));
    }

    // If it is an SVG, try rasterizing using rsvg-convert first
    let raw_bytes = if path_str.to_lowercase().ends_with(".svg") {
        if let Ok(output) = std::process::Command::new("rsvg-convert")
            .arg("-w")
            .arg("196")
            .arg("-h")
            .arg("196")
            .arg("-f")
            .arg("png")
            .arg(p)
            .output()
        {
            if output.status.success() && !output.stdout.is_empty() {
                output.stdout
            } else {
                std::fs::read(p).map_err(|e| format!("Erro ao ler SVG: {}", e))?
            }
        } else {
            std::fs::read(p).map_err(|e| format!("Erro ao ler SVG: {}", e))?
        }
    } else {
        std::fs::read(p).map_err(|e| format!("Erro ao ler arquivo: {}", e))?
    };

    // Decode and resize with image crate
    if let Ok(img) = image::load_from_memory(&raw_bytes) {
        let resized = img.resize_exact(196, 196, image::imageops::FilterType::Lanczos3);
        let rgb_img = resized.to_rgb8();
        let mut jpeg_buf = Vec::new();
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg_buf, 92);
        encoder
            .encode_image(&rgb_img)
            .map_err(|e| format!("Erro ao codificar JPEG: {}", e))?;
        return Ok(jpeg_buf);
    }

    // Fallback: return raw bytes
    Ok(raw_bytes)
}

/// Helper to open an icon file dialog in a separate thread
fn open_file_dialog(
    initial_dir: Option<PathBuf>,
    title: &str,
    ui_weak: slint::Weak<AppWindow>,
    profile_arc: Arc<Mutex<ProfileConfig>>,
) {
    let title_owned = title.to_string();
    std::thread::spawn(move || {
        let mut dialog = rfd::FileDialog::new()
            .set_title(&title_owned)
            .add_filter(
                "Imagens / Ícones",
                &["png", "jpg", "jpeg", "svg", "webp", "ico", "bmp"],
            );

        if let Some(dir) = initial_dir {
            if dir.exists() {
                dialog = dialog.set_directory(dir);
            }
        }

        if let Some(path) = dialog.pick_file() {
            let path_str = path.to_string_lossy().to_string();
            let file_name = path
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| path_str.clone());

            let profile_arc = profile_arc.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_edit_icon_path(SharedString::from(&path_str));
                    let (has_icon, img) = load_slint_image(&path_str);
                    ui.set_edit_has_icon(has_icon);
                    ui.set_edit_icon_preview(img.clone());

                    // Immediate sync to selected key in 5x3 pad and memory profile
                    let sel_id = ui.get_selected_key_index() as usize;
                    let keys = ui.get_keys();
                    if let Some(mut item) = keys.row_data(sel_id) {
                        item.icon_path = SharedString::from(&path_str);
                        item.has_icon = has_icon;
                        item.icon = img;
                        keys.set_row_data(sel_id, item);
                    }

                    {
                        let mut guard = profile_arc.lock().unwrap();
                        if sel_id < guard.keys.len() {
                            guard.keys[sel_id].icon_path = path_str.clone();
                        }
                    }

                    ui.set_status_message(SharedString::from(format!(
                        "Ícone selecionado: {}",
                        file_name
                    )));
                }
            });
        }
    });
}

/// Updates the filtered system applications in the Slint model
fn update_filtered_apps_model(
    model: &Rc<VecModel<SystemAppItem>>,
    all_apps: &[apps::AppInfo],
    filter_query: &str,
) {
    let q = filter_query.trim().to_lowercase();
    let mut items = Vec::new();

    for app in all_apps {
        if q.is_empty()
            || app.name.to_lowercase().contains(&q)
            || app.exec_cmd.to_lowercase().contains(&q)
        {
            let (has_icon, icon) = load_slint_image(&app.icon_path);
            items.push(SystemAppItem {
                name: SharedString::from(&app.name),
                exec_cmd: SharedString::from(&app.exec_cmd),
                icon_path: SharedString::from(&app.icon_path),
                has_icon,
                icon,
            });

            // Limit results to 75 items for instantaneous scrolling
            if items.len() >= 75 {
                break;
            }
        }
    }

    model.set_vec(items);
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let main_window = AppWindow::new()?;
    let ui_weak = main_window.as_weak();

    // Shared thread-safe persistent profile configuration
    let profile = Arc::new(Mutex::new(ProfileConfig::load()));

    // Populate keys in the Slint Model
    let keys_model = Rc::new(VecModel::<KeyItem>::default());
    {
        let guard = profile.lock().unwrap();
        for k in &guard.keys {
            let (has_icon, icon) = load_slint_image(&k.icon_path);
            keys_model.push(KeyItem {
                id: k.id as i32,
                col: k.col as i32,
                row: k.row as i32,
                pos_str: SharedString::from(&k.pos_str),
                title: SharedString::from(&k.title),
                action_type: SharedString::from(&k.action_type),
                action_value: SharedString::from(&k.action_value),
                icon_path: SharedString::from(&k.icon_path),
                has_icon,
                icon,
                is_pressed: false,
                is_selected: k.id == 0,
            });
        }

        // Set initial properties on UI
        main_window.set_brightness_val(guard.brightness as i32);
        main_window.set_sleep_val(guard.sleep_seconds as i32);
        main_window.set_selected_key_index(0);

        if let Some(first_key) = guard.keys.first() {
            main_window.set_edit_title(SharedString::from(&first_key.title));
            main_window.set_edit_action_type(SharedString::from(&first_key.action_type));
            main_window.set_edit_action_value(SharedString::from(&first_key.action_value));
            main_window.set_edit_icon_path(SharedString::from(&first_key.icon_path));
            let (has_icon, img) = load_slint_image(&first_key.icon_path);
            main_window.set_edit_has_icon(has_icon);
            main_window.set_edit_icon_preview(img);
        }
    }
    main_window.set_keys(keys_model.clone().into());

    // Scan installed system applications
    let system_apps = Arc::new(apps::scan_system_apps());
    let filtered_apps_model = Rc::new(VecModel::<SystemAppItem>::default());
    update_filtered_apps_model(&filtered_apps_model, &system_apps, "");
    main_window.set_filtered_apps(filtered_apps_model.clone().into());

    // Initialize Ulanzi Deck Hardware Controller
    let deck = UlanziDeck::new();
    let (tx, rx) = mpsc::channel::<DeckEvent>();

    // Connect to hardware
    match deck.connect(tx) {
        Ok(path) => {
            main_window.set_device_connected(true);
            main_window.set_device_node(SharedString::from(path.to_string_lossy().to_string()));
            main_window.set_status_message(SharedString::from(format!(
                "Conectado com sucesso em {}",
                path.display()
            )));

            let b = profile.lock().unwrap().brightness;
            let _ = deck.set_brightness(b);
        }
        Err(err) => {
            main_window.set_device_connected(false);
            main_window.set_status_message(SharedString::from(format!("Aviso: {}", err)));
        }
    }

    // Handle UI Callbacks

    // 1. Key Selected in 5x3 Grid
    {
        let keys_model = keys_model.clone();
        let ui_handle = ui_weak.clone();
        main_window.on_key_selected(move |id| {
            if let Some(ui) = ui_handle.upgrade() {
                ui.set_selected_key_index(id);

                for i in 0..keys_model.row_count() {
                    if let Some(mut item) = keys_model.row_data(i) {
                        item.is_selected = item.id == id;
                        if item.id == id {
                            ui.set_edit_title(item.title.clone());
                            ui.set_edit_action_type(item.action_type.clone());
                            ui.set_edit_action_value(item.action_value.clone());
                            ui.set_edit_icon_path(item.icon_path.clone());
                            let (has_icon, img) = load_slint_image(item.icon_path.as_str());
                            ui.set_edit_has_icon(has_icon);
                            ui.set_edit_icon_preview(img);
                        }
                        keys_model.set_row_data(i, item);
                    }
                }
            }
        });
    }

    // 2. Direct text editing of icon path
    {
        let ui_handle = ui_weak.clone();
        let keys_model = keys_model.clone();
        let profile_arc = profile.clone();
        main_window.on_icon_path_edited(move |txt| {
            if let Some(ui) = ui_handle.upgrade() {
                let (has_icon, img) = load_slint_image(txt.as_str());
                ui.set_edit_has_icon(has_icon);
                ui.set_edit_icon_preview(img.clone());

                let sel_id = ui.get_selected_key_index() as usize;
                if let Some(mut item) = keys_model.row_data(sel_id) {
                    item.icon_path = txt.clone();
                    item.has_icon = has_icon;
                    item.icon = img;
                    keys_model.set_row_data(sel_id, item);
                }

                let mut guard = profile_arc.lock().unwrap();
                if sel_id < guard.keys.len() {
                    guard.keys[sel_id].icon_path = txt.to_string();
                }
            }
        });
    }

    // 3. Clear key icon
    {
        let ui_handle = ui_weak.clone();
        let keys_model = keys_model.clone();
        let profile_arc = profile.clone();
        main_window.on_clear_key_icon(move || {
            if let Some(ui) = ui_handle.upgrade() {
                ui.set_edit_icon_path(SharedString::default());
                ui.set_edit_has_icon(false);
                ui.set_edit_icon_preview(slint::Image::default());

                let sel_id = ui.get_selected_key_index() as usize;
                if let Some(mut item) = keys_model.row_data(sel_id) {
                    item.icon_path = SharedString::default();
                    item.has_icon = false;
                    item.icon = slint::Image::default();
                    keys_model.set_row_data(sel_id, item);
                }

                let mut guard = profile_arc.lock().unwrap();
                if sel_id < guard.keys.len() {
                    guard.keys[sel_id].icon_path = String::new();
                    guard.save();
                }

                ui.set_status_message(SharedString::from("Ícone removido da tecla"));
            }
        });
    }

    // 4. Browse Custom File Icon
    {
        let ui_handle = ui_weak.clone();
        let profile_arc = profile.clone();
        main_window.on_browse_icon_custom(move || {
            let initial = dirs::picture_dir().or_else(dirs::home_dir);
            open_file_dialog(
                initial,
                "Selecionar Ícone / Imagem Customizada",
                ui_handle.clone(),
                profile_arc.clone(),
            );
        });
    }

    // 5. Browse System Icons
    {
        let ui_handle = ui_weak.clone();
        let profile_arc = profile.clone();
        main_window.on_browse_icon_system(move || {
            let initial = if Path::new("/usr/share/pixmaps").exists() {
                Some(PathBuf::from("/usr/share/pixmaps"))
            } else if Path::new("/usr/share/icons").exists() {
                Some(PathBuf::from("/usr/share/icons"))
            } else {
                dirs::home_dir()
            };
            open_file_dialog(
                initial,
                "Selecionar Ícone do Sistema (/usr/share)",
                ui_handle.clone(),
                profile_arc.clone(),
            );
        });
    }

    // 6. Browse Ulanzi Extracted Icons
    {
        let ui_handle = ui_weak.clone();
        let profile_arc = profile.clone();
        main_window.on_browse_icon_ulanzi(move || {
            let mut ulanzi_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
            ulanzi_dir.push("uzlinux-studio/icons/ulanzi_defaults");
            open_file_dialog(
                Some(ulanzi_dir),
                "Selecionar Ícone da Ulanzi",
                ui_handle.clone(),
                profile_arc.clone(),
            );
        });
    }

    // 7. Save Key Configuration
    {
        let keys_model = keys_model.clone();
        let ui_handle = ui_weak.clone();
        let profile_arc = profile.clone();
        main_window.on_save_key_action(move |id, pos_str, title, act_type, act_val, icon_path| {
            let id_usize = id as usize;
            {
                let mut guard = profile_arc.lock().unwrap();
                if id_usize < guard.keys.len() {
                    guard.keys[id_usize].title = title.to_string();
                    guard.keys[id_usize].action_type = act_type.to_string();
                    guard.keys[id_usize].action_value = act_val.to_string();
                    guard.keys[id_usize].icon_path = icon_path.to_string();
                    guard.save();
                }
            }

            let (has_icon, img) = load_slint_image(icon_path.as_str());

            if let Some(mut item) = keys_model.row_data(id_usize) {
                item.title = title.clone();
                item.action_type = act_type.clone();
                item.action_value = act_val.clone();
                item.icon_path = icon_path.clone();
                item.has_icon = has_icon;
                item.icon = img;
                keys_model.set_row_data(id_usize, item);
            }

            if let Some(ui) = ui_handle.upgrade() {
                ui.set_status_message(SharedString::from(format!(
                    "Configuração da tecla [{}] salva com sucesso!",
                    pos_str
                )));
            }
        });
    }

    // 8. Send Image to Deck LCD (BUG FIX: prioritizes active UI input, auto-scales to 196x196 JPEG)
    {
        let deck_clone = deck.clone();
        let ui_handle = ui_weak.clone();
        let profile_arc = profile.clone();
        let keys_model = keys_model.clone();

        main_window.on_send_key_image_to_deck(move |id| {
            let id_usize = id as usize;
            let current_ui = match ui_handle.upgrade() {
                Some(ui) => ui,
                None => return,
            };

            // Retrieve active icon path: FIRST inspect currently edited text in UI, THEN fallback to memory profile
            let ui_icon = current_ui.get_edit_icon_path().to_string();
            let icon_path = if !ui_icon.trim().is_empty() {
                ui_icon.trim().to_string()
            } else {
                let guard = profile_arc.lock().unwrap();
                if id_usize < guard.keys.len() {
                    guard.keys[id_usize].icon_path.clone()
                } else {
                    String::new()
                }
            };

            if icon_path.is_empty() {
                current_ui.set_status_message(SharedString::from(
                    "Informe ou selecione um ícone antes de enviar ao Deck",
                ));
                return;
            }

            // Sync icon to profile & model immediately
            {
                let mut guard = profile_arc.lock().unwrap();
                if id_usize < guard.keys.len() {
                    guard.keys[id_usize].icon_path = icon_path.clone();
                    guard.save();
                }
            }
            if let Some(mut item) = keys_model.row_data(id_usize) {
                let (has_icon, img) = load_slint_image(&icon_path);
                item.icon_path = SharedString::from(&icon_path);
                item.has_icon = has_icon;
                item.icon = img;
                keys_model.set_row_data(id_usize, item);
            }

            // Prepare 196x196 RGB JPEG
            match prepare_deck_icon(&icon_path) {
                Ok(bytes) => {
                    let mut sent_ok = false;

                    // Send to specific slot (0..14)
                    if deck_clone.send_key_image(id as u8, &bytes).is_ok() {
                        sent_ok = true;
                    }
                    // Also dispatch screen data
                    let _ = deck_clone.send_screen_data(&bytes);

                    if sent_ok || deck_clone.is_connected() {
                        current_ui.set_status_message(SharedString::from(format!(
                            "Ícone enviado para a tecla [{}] no Deck com sucesso!",
                            id
                        )));
                    } else {
                        current_ui.set_status_message(SharedString::from(
                            "Dispositivo desconectado. Conecte o cabo USB do Ulanzi Deck.",
                        ));
                    }
                }
                Err(e) => {
                    current_ui.set_status_message(SharedString::from(format!(
                        "Erro ao preparar imagem: {}",
                        e
                    )));
                }
            }
        });
    }

    // 9. System Application Picker Callbacks
    {
        let ui_handle = ui_weak.clone();
        let system_apps = system_apps.clone();
        let filtered_apps_model = filtered_apps_model.clone();

        main_window.on_open_app_picker(move || {
            if let Some(ui) = ui_handle.upgrade() {
                ui.set_app_filter_text(SharedString::default());
                update_filtered_apps_model(&filtered_apps_model, &system_apps, "");
                ui.set_show_app_picker(true);
            }
        });
    }

    {
        let ui_handle = ui_weak.clone();
        main_window.on_close_app_picker(move || {
            if let Some(ui) = ui_handle.upgrade() {
                ui.set_show_app_picker(false);
            }
        });
    }

    {
        let system_apps = system_apps.clone();
        let filtered_apps_model = filtered_apps_model.clone();
        main_window.on_app_filter_changed(move |query| {
            update_filtered_apps_model(&filtered_apps_model, &system_apps, query.as_str());
        });
    }

    {
        let ui_handle = ui_weak.clone();
        let keys_model = keys_model.clone();
        let profile_arc = profile.clone();

        main_window.on_app_selected(move |name, exec_cmd, icon_path| {
            if let Some(ui) = ui_handle.upgrade() {
                ui.set_edit_action_type(SharedString::from("app"));
                ui.set_edit_action_value(exec_cmd.clone());

                if ui.get_edit_title().trim().is_empty() {
                    ui.set_edit_title(name.clone());
                }

                let icon_str = icon_path.as_str();
                if !icon_str.is_empty() {
                    ui.set_edit_icon_path(icon_path.clone());
                    let (has_icon, img) = load_slint_image(icon_str);
                    ui.set_edit_has_icon(has_icon);
                    ui.set_edit_icon_preview(img.clone());

                    let sel_id = ui.get_selected_key_index() as usize;
                    if let Some(mut item) = keys_model.row_data(sel_id) {
                        item.icon_path = icon_path.clone();
                        item.has_icon = has_icon;
                        item.icon = img;
                        item.action_type = SharedString::from("app");
                        item.action_value = exec_cmd.clone();
                        if item.title.is_empty() {
                            item.title = name.clone();
                        }
                        keys_model.set_row_data(sel_id, item);
                    }

                    let mut guard = profile_arc.lock().unwrap();
                    if sel_id < guard.keys.len() {
                        guard.keys[sel_id].icon_path = icon_str.to_string();
                        guard.keys[sel_id].action_type = "app".to_string();
                        guard.keys[sel_id].action_value = exec_cmd.to_string();
                        if guard.keys[sel_id].title.is_empty() {
                            guard.keys[sel_id].title = name.to_string();
                        }
                        guard.save();
                    }
                }

                ui.set_show_app_picker(false);
                ui.set_status_message(SharedString::from(format!(
                    "Aplicativo '{}' selecionado!",
                    name
                )));
            }
        });
    }

    // 10. Browse Application Binary (/usr/bin)
    {
        let ui_handle = ui_weak.clone();
        main_window.on_browse_app_binary(move || {
            let ui_weak = ui_handle.clone();
            std::thread::spawn(move || {
                let dialog = rfd::FileDialog::new()
                    .set_title("Selecionar Executável / Binário (/usr/bin)")
                    .set_directory("/usr/bin");

                if let Some(path) = dialog.pick_file() {
                    let path_str = path.to_string_lossy().to_string();
                    let file_name = path
                        .file_name()
                        .map(|f| f.to_string_lossy().to_string())
                        .unwrap_or_else(|| path_str.clone());

                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak.upgrade() {
                            ui.set_edit_action_type(SharedString::from("app"));
                            ui.set_edit_action_value(SharedString::from(&path_str));
                            if ui.get_edit_title().trim().is_empty() {
                                ui.set_edit_title(SharedString::from(&file_name));
                            }
                            ui.set_status_message(SharedString::from(format!(
                                "Binário selecionado: {}",
                                file_name
                            )));
                        }
                    });
                }
            });
        });
    }

    // 11. Browse Command Script (.sh)
    {
        let ui_handle = ui_weak.clone();
        main_window.on_browse_command_script(move || {
            let ui_weak = ui_handle.clone();
            std::thread::spawn(move || {
                let initial = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
                let dialog = rfd::FileDialog::new()
                    .set_title("Selecionar Script de Comando Linux")
                    .set_directory(initial)
                    .add_filter("Scripts", &["sh", "bash", "py", "pl", "rb", "bin"]);

                if let Some(path) = dialog.pick_file() {
                    let path_str = path.to_string_lossy().to_string();
                    let file_name = path
                        .file_name()
                        .map(|f| f.to_string_lossy().to_string())
                        .unwrap_or_else(|| path_str.clone());

                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak.upgrade() {
                            ui.set_edit_action_type(SharedString::from("command"));
                            ui.set_edit_action_value(SharedString::from(&path_str));
                            if ui.get_edit_title().trim().is_empty() {
                                ui.set_edit_title(SharedString::from(&file_name));
                            }
                            ui.set_status_message(SharedString::from(format!(
                                "Script selecionado: {}",
                                file_name
                            )));
                        }
                    });
                }
            });
        });
    }

    // 12. Brightness change
    {
        let deck_clone = deck.clone();
        let profile_arc = profile.clone();
        main_window.on_brightness_changed(move |val| {
            let mut guard = profile_arc.lock().unwrap();
            guard.brightness = val as u8;
            guard.save();
            let _ = deck_clone.set_brightness(val as u8);
        });
    }

    // 13. Sleep timeout change
    {
        let deck_clone = deck.clone();
        let profile_arc = profile.clone();
        main_window.on_sleep_changed(move |val| {
            let mut guard = profile_arc.lock().unwrap();
            guard.sleep_seconds = val as u32;
            guard.save();
            let _ = deck_clone.set_sleep_time(val as u32);
        });
    }

    // 14. Reconnect callback
    {
        let deck_clone = deck.clone();
        let ui_handle = ui_weak.clone();
        main_window.on_refresh_connection(move || {
            let (new_tx, _new_rx) = mpsc::channel();
            if let Some(ui) = ui_handle.upgrade() {
                match deck_clone.connect(new_tx) {
                    Ok(path) => {
                        ui.set_device_connected(true);
                        ui.set_device_node(SharedString::from(path.to_string_lossy().to_string()));
                        ui.set_status_message(SharedString::from(format!(
                            "Reconectado a {}",
                            path.display()
                        )));
                    }
                    Err(e) => {
                        ui.set_device_connected(false);
                        ui.set_status_message(SharedString::from(e));
                    }
                }
            }
        });
    }

    // 15. Background Thread: Listen for hardware events from channel and dispatch to Slint UI & Actions
    {
        let ui_handle = ui_weak.clone();
        let profile_arc = profile.clone();

        std::thread::Builder::new()
            .name("event-dispatcher".into())
            .spawn(move || {
                while let Ok(event) = rx.recv() {
                    match event {
                        DeckEvent::KeyEvent {
                            col,
                            row,
                            pressed,
                            ..
                        } => {
                            let slot_id = (row * 5 + col) as usize;

                            if pressed {
                                // Execute configured action
                                {
                                    let guard = profile_arc.lock().unwrap();
                                    if slot_id < guard.keys.len() {
                                        let key_cfg = &guard.keys[slot_id];
                                        actions::execute_action(
                                            &key_cfg.action_type,
                                            &key_cfg.action_value,
                                        );
                                    }
                                }

                                // Visual feedback on Slint UI
                                let ui_weak_inner = ui_handle.clone();
                                slint::invoke_from_event_loop(move || {
                                    if let Some(ui) = ui_weak_inner.upgrade() {
                                        ui.set_status_message(SharedString::from(format!(
                                            "Evento: Tecla [{}_{}] pressionada!",
                                            col, row
                                        )));
                                        ui.invoke_set_key_pressed(slot_id as i32, true);
                                    }
                                })
                                .ok();

                                // Un-press visually after 180ms
                                let ui_weak_inner2 = ui_handle.clone();
                                std::thread::spawn(move || {
                                    std::thread::sleep(Duration::from_millis(180));
                                    slint::invoke_from_event_loop(move || {
                                        if let Some(ui) = ui_weak_inner2.upgrade() {
                                            ui.invoke_set_key_pressed(slot_id as i32, false);
                                        }
                                    })
                                    .ok();
                                });
                            }
                        }
                        DeckEvent::Heartbeat => {
                            // Device heartbeat received
                        }
                        DeckEvent::Unknown { proto, cmd, len } => {
                            println!(
                                "[Deck] Pacote recebido: proto=0x{:02x}, cmd=0x{:02x}, len={}",
                                proto, cmd, len
                            );
                        }
                    }
                }
            })
            .expect("Falha ao iniciar thread de despacho de eventos");
    }

    main_window.run()?;
    Ok(())
}

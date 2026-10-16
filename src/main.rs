mod actions;
mod config;
mod device;

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc;
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

/// Helper to open a file dialog in a separate thread
fn open_file_dialog(
    initial_dir: Option<PathBuf>,
    title: &str,
    ui_weak: slint::Weak<AppWindow>,
) {
    let title_owned = title.to_string();
    std::thread::spawn(move || {
        let mut dialog = rfd::FileDialog::new()
            .set_title(&title_owned)
            .add_filter("Imagens / Ícones", &["png", "jpg", "jpeg", "svg", "webp", "ico", "bmp"]);

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

            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_edit_icon_path(SharedString::from(&path_str));
                    let (has_icon, img) = load_slint_image(&path_str);
                    ui.set_edit_has_icon(has_icon);
                    ui.set_edit_icon_preview(img);
                    ui.set_status_message(SharedString::from(format!(
                        "Ícone selecionado: {}",
                        file_name
                    )));
                }
            });
        }
    });
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let main_window = AppWindow::new()?;
    let ui_weak = main_window.as_weak();

    // Load persistent profile configuration
    let profile = ProfileConfig::load();

    // Populate keys in the Slint Model
    let keys_model = Rc::new(VecModel::<KeyItem>::default());
    for k in &profile.keys {
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
    main_window.set_keys(keys_model.clone().into());

    // Set initial properties on UI
    main_window.set_brightness_val(profile.brightness as i32);
    main_window.set_sleep_val(profile.sleep_seconds as i32);
    main_window.set_selected_key_index(0);
    if let Some(first_key) = profile.keys.first() {
        main_window.set_edit_title(SharedString::from(&first_key.title));
        main_window.set_edit_action_type(SharedString::from(&first_key.action_type));
        main_window.set_edit_action_value(SharedString::from(&first_key.action_value));
        main_window.set_edit_icon_path(SharedString::from(&first_key.icon_path));
        let (has_icon, img) = load_slint_image(&first_key.icon_path);
        main_window.set_edit_has_icon(has_icon);
        main_window.set_edit_icon_preview(img);
    }

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

            // Apply initial brightness
            let _ = deck.set_brightness(profile.brightness);
        }
        Err(err) => {
            main_window.set_device_connected(false);
            main_window.set_status_message(SharedString::from(format!(
                "Aviso: {}",
                err
            )));
        }
    }

    // Handle UI Callbacks
    {
        let keys_model = keys_model.clone();
        let ui_handle = ui_weak.clone();
        main_window.on_key_selected(move |id| {
            if let Some(ui) = ui_handle.upgrade() {
                ui.set_selected_key_index(id);

                // Update is_selected in model
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

    // Direct text editing of icon path
    {
        let ui_handle = ui_weak.clone();
        main_window.on_icon_path_edited(move |txt| {
            if let Some(ui) = ui_handle.upgrade() {
                let (has_icon, img) = load_slint_image(txt.as_str());
                ui.set_edit_has_icon(has_icon);
                ui.set_edit_icon_preview(img);
            }
        });
    }

    // Clear key icon
    {
        let ui_handle = ui_weak.clone();
        main_window.on_clear_key_icon(move || {
            if let Some(ui) = ui_handle.upgrade() {
                ui.set_edit_icon_path(SharedString::default());
                ui.set_edit_has_icon(false);
                ui.set_edit_icon_preview(slint::Image::default());
                ui.set_status_message(SharedString::from("Ícone removido da tecla"));
            }
        });
    }

    // Browse Custom File
    {
        let ui_handle = ui_weak.clone();
        main_window.on_browse_icon_custom(move || {
            let initial = dirs::picture_dir().or_else(dirs::home_dir);
            open_file_dialog(initial, "Selecionar Ícone / Imagem Customizada", ui_handle.clone());
        });
    }

    // Browse System Icons
    {
        let ui_handle = ui_weak.clone();
        main_window.on_browse_icon_system(move || {
            let initial = if Path::new("/usr/share/pixmaps").exists() {
                Some(PathBuf::from("/usr/share/pixmaps"))
            } else if Path::new("/usr/share/icons").exists() {
                Some(PathBuf::from("/usr/share/icons"))
            } else {
                dirs::home_dir()
            };
            open_file_dialog(initial, "Selecionar Ícone do Sistema (/usr/share)", ui_handle.clone());
        });
    }

    // Browse Ulanzi Extracted Icons
    {
        let ui_handle = ui_weak.clone();
        main_window.on_browse_icon_ulanzi(move || {
            let mut ulanzi_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
            ulanzi_dir.push("uzlinux-studio/icons/ulanzi_defaults");
            open_file_dialog(Some(ulanzi_dir), "Selecionar Ícone da Ulanzi", ui_handle.clone());
        });
    }

    // Save key configuration
    {
        let keys_model = keys_model.clone();
        let ui_handle = ui_weak.clone();
        let mut profile_clone = profile.clone();
        main_window.on_save_key_action(move |id, pos_str, title, act_type, act_val, icon_path| {
            let id_usize = id as usize;
            if id_usize < profile_clone.keys.len() {
                profile_clone.keys[id_usize].title = title.to_string();
                profile_clone.keys[id_usize].action_type = act_type.to_string();
                profile_clone.keys[id_usize].action_value = act_val.to_string();
                profile_clone.keys[id_usize].icon_path = icon_path.to_string();
                profile_clone.save();

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
            }
        });
    }

    // Send image to deck LCD
    {
        let deck_clone = deck.clone();
        let ui_handle = ui_weak.clone();
        let profile_clone = profile.clone();
        main_window.on_send_key_image_to_deck(move |id| {
            let id_usize = id as usize;
            if id_usize < profile_clone.keys.len() {
                let key = &profile_clone.keys[id_usize];
                if !key.icon_path.is_empty() {
                    match std::fs::read(&key.icon_path) {
                        Ok(bytes) => {
                            match deck_clone.send_image(device::protocol::CMD_SEND_SCREEN_DATA, &bytes) {
                                Ok(_) => {
                                    if let Some(ui) = ui_handle.upgrade() {
                                        ui.set_status_message(SharedString::from(format!(
                                            "Imagem enviada para o Deck na tecla [{}]!",
                                            key.pos_str
                                        )));
                                    }
                                }
                                Err(e) => {
                                    if let Some(ui) = ui_handle.upgrade() {
                                        ui.set_status_message(SharedString::from(format!(
                                            "Erro ao enviar imagem: {}",
                                            e
                                        )));
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            if let Some(ui) = ui_handle.upgrade() {
                                ui.set_status_message(SharedString::from(format!(
                                    "Falha ao abrir imagem {}: {}",
                                    key.icon_path, e
                                )));
                            }
                        }
                    }
                } else if let Some(ui) = ui_handle.upgrade() {
                    ui.set_status_message(SharedString::from(
                        "Informe ou selecione um ícone antes de enviar ao Deck"
                    ));
                }
            }
        });
    }

    // Brightness change
    {
        let deck_clone = deck.clone();
        let mut profile_clone = profile.clone();
        main_window.on_brightness_changed(move |val| {
            profile_clone.brightness = val as u8;
            profile_clone.save();
            let _ = deck_clone.set_brightness(val as u8);
        });
    }

    // Sleep timeout change
    {
        let deck_clone = deck.clone();
        let mut profile_clone = profile.clone();
        main_window.on_sleep_changed(move |val| {
            profile_clone.sleep_seconds = val as u32;
            profile_clone.save();
            let _ = deck_clone.set_sleep_time(val as u32);
        });
    }

    // Reconnect callback
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

    // Background Thread: Listen for hardware events from channel and dispatch to Slint UI & Actions
    {
        let ui_handle = ui_weak.clone();
        let profile_clone = profile.clone();

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
                                if slot_id < profile_clone.keys.len() {
                                    let key_cfg = &profile_clone.keys[slot_id];
                                    actions::execute_action(&key_cfg.action_type, &key_cfg.action_value);
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
                            println!("[Deck] Pacote recebido: proto=0x{:02x}, cmd=0x{:02x}, len={}", proto, cmd, len);
                        }
                    }
                }
            })
            .expect("Falha ao iniciar thread de despacho de eventos");
    }

    main_window.run()?;
    Ok(())
}

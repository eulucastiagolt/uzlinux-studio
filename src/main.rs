mod actions;
mod apps;
mod autostart;
mod config;
mod device;
mod firmware;
mod udev;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use slint::{ComponentHandle, Model, SharedString, VecModel};
use slint::private_unstable_api::re_exports::DataTransfer;

use crate::config::ProfileConfig;
use crate::device::{DeckEvent, UlanziDeck};

slint::include_modules!();

/// Real-time sync message: Key(id) for fast incremental update (~30ms), Full for complete re-send
#[derive(Debug, Clone, Copy)]
pub enum SyncMsg {
    Key(usize),
    Full,
}


/// Helper to load a Slint image from a file path
fn load_slint_image(path_str: &str) -> (bool, slint::Image) {
    if path_str.trim().is_empty() {
        return (false, slint::Image::default());
    }
    let p = Path::new(path_str);
    if p.exists() {
        // Fast path: if recognized extension, try direct load
        if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
            let ext_lower = ext.to_lowercase();
            if ext_lower == "png" || ext_lower == "jpg" || ext_lower == "jpeg" || ext_lower == "svg" {
                if let Ok(img) = slint::Image::load_from_path(p) {
                    return (true, img);
                }
            }
        }

        // Memory load fallback for extensionless images or other formats (avoids Slint console warnings)
        if let Ok(bytes) = std::fs::read(p) {
            if let Ok(dyn_img) = image::load_from_memory(&bytes) {
                let rgba = dyn_img.to_rgba8();
                let width = rgba.width();
                let height = rgba.height();
                let buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                    rgba.as_raw(),
                    width,
                    height,
                );
                return (true, slint::Image::from_rgba8(buffer));
            }
        }
    }
    (false, slint::Image::default())
}

/// Helper to get the official Ulanzi Studio default icon for a given action
fn get_official_icon_for_action(act_type: &str, act_val: &str) -> Option<&'static str> {
    match act_type {
        "media" => match act_val {
            "playpause" | "play_pause" | "play" | "pause" => Some("assets/ulanzi_official/icons/Play.png"),
            "next" => Some("assets/ulanzi_official/icons/Next Track.png"),
            "prev" => Some("assets/ulanzi_official/icons/Previous Track.png"),
            "volup" => Some("assets/ulanzi_official/icons/Volume Up.png"),
            "voldown" => Some("assets/ulanzi_official/icons/Volume Down.png"),
            "mute" => Some("assets/ulanzi_official/icons/Mute.png"),
            "stop" => Some("assets/ulanzi_official/icons/Stop.png"),
            _ => Some("assets/ulanzi_official/icons/Play.png"),
        },
        "app" => Some("assets/ulanzi_official/actions/open.svg"),
        "close" => Some("assets/ulanzi_official/actions/close.svg"),
        "website" => Some("assets/ulanzi_official/actions/website.svg"),
        "hotkey" => Some("assets/ulanzi_official/actions/hotkey.svg"),
        "text" => Some("assets/ulanzi_official/actions/text.svg"),
        "sound" => {
            if act_val == "stop" || act_val == "parar" {
                Some("assets/ulanzi_official/actions/stopAudio.svg")
            } else {
                Some("assets/ulanzi_official/actions/playAudio.svg")
            }
        },
        "multiaction" => Some("assets/ulanzi_official/actions/multiAction.svg"),
        "obs" => match act_val {
            "record" => Some("assets/ulanzi_official/actions/record.svg"),
            "recordpause" => Some("assets/ulanzi_official/actions/recordpause.svg"),
            "stream" => Some("assets/ulanzi_official/actions/stream.svg"),
            "virtualcam" => Some("assets/ulanzi_official/actions/virtualcam.svg"),
            "studiomode" => Some("assets/ulanzi_official/actions/studiomode.svg"),
            _ => Some("assets/ulanzi_official/actions/record.svg"),
        },
        "command" => Some("assets/ulanzi_official/actions/actionTrigger.svg"),
        "page.next" => Some("assets/ulanzi_official/actions/btn_nextPage.png"),
        "page.prev" => Some("assets/ulanzi_official/actions/btn_previousPage.png"),
        "page.goto" => Some("assets/ulanzi_official/actions/btn_goToPage.png"),
        "page.indicator" => Some("assets/ulanzi_official/actions/btn_pageIndicator.png"),
        "page.switch" => Some("assets/ulanzi_official/actions/btn_switchProfile.png"),
        _ => None,
    }
}

/// Helper to synchronize Profile and Page models between Rust config and Slint UI
fn refresh_ui_profiles_and_pages(
    ui: &AppWindow,
    guard: &config::ProfileConfig,
) {
    // 1. Profiles List
    let prof_model = Rc::new(VecModel::<ProfileItem>::default());
    for p in &guard.profiles {
        let is_active = p.name == guard.current_profile || p.id == guard.current_profile;
        prof_model.push(ProfileItem {
            id: SharedString::from(&p.id),
            name: SharedString::from(&p.name),
            is_active,
        });
    }
    ui.set_profiles_list(prof_model.into());
    ui.set_current_profile_name(SharedString::from(&guard.current_profile));

    // 2. Pages List
    let pages_model = Rc::new(VecModel::<PageItem>::default());
    let cur_page_idx = guard.current_page_index();
    if let Some(prof) = guard.active_profile() {
        for (i, p) in prof.pages.iter().enumerate() {
            pages_model.push(PageItem {
                id: i as i32,
                name: SharedString::from(&p.name),
                is_active: i == cur_page_idx,
            });
        }
    }
    ui.set_pages(pages_model.into());
    ui.set_current_page_index(cur_page_idx as i32);
    ui.set_total_pages_count(guard.pages_count() as i32);

    // 3. Keys Model & Inspector for Current Page
    let sel_idx = (ui.get_selected_key_index() as usize).min(14);
    if let Some(keys_model) = ui.get_keys().as_any().downcast_ref::<VecModel<KeyItem>>() {
        for (i, k) in guard.keys.iter().enumerate() {
            let (has_icon, icon) = load_slint_image(&k.icon_path);
            let item = KeyItem {
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
                is_selected: i == sel_idx,
            };
            if i < keys_model.row_count() {
                keys_model.set_row_data(i, item);
            } else {
                keys_model.push(item);
            }
        }
    }

    if let Some(sel_key) = guard.keys.get(sel_idx) {
        ui.set_edit_title(SharedString::from(&sel_key.title));
        ui.set_edit_action_type(SharedString::from(&sel_key.action_type));
        ui.set_edit_action_value(SharedString::from(&sel_key.action_value));
        ui.set_edit_icon_path(SharedString::from(&sel_key.icon_path));
        let (has_icon, img) = load_slint_image(&sel_key.icon_path);
        ui.set_edit_has_icon(has_icon);
        ui.set_edit_icon_preview(img);
    }
}

/// Helper to open an icon file dialog in a separate thread
fn open_file_dialog(
    initial_dir: Option<PathBuf>,
    title: &str,
    ui_weak: slint::Weak<AppWindow>,
    profile_arc: Arc<Mutex<ProfileConfig>>,
    sync_tx: mpsc::Sender<SyncMsg>,
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
            let sync_tx = sync_tx.clone();
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
                            guard.save();
                        }
                    }

                    let _ = sync_tx.send(SyncMsg::Key(sel_id));

                    ui.set_status_message(SharedString::from(format!(
                        "Ícone selecionado: {}",
                        file_name
                    )));
                }
            });
        }
    });
}

/// Helper to convert apps::AppInfo into Slint SystemAppItem
fn app_info_to_item(app: &apps::AppInfo) -> SystemAppItem {
    let (has_icon, icon) = load_slint_image(&app.icon_path);
    SystemAppItem {
        name: SharedString::from(&app.name),
        exec_cmd: SharedString::from(&app.exec_cmd),
        icon_path: SharedString::from(&app.icon_path),
        has_icon,
        icon,
    }
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
            || app.category.to_lowercase().contains(&q)
        {
            items.push(app_info_to_item(app));

            // Limit results to 75 items for instantaneous scrolling
            if items.len() >= 75 {
                break;
            }
        }
    }

    model.set_vec(items);
}

#[derive(Clone, Debug)]
pub struct AppCategoryData {
    pub id: i32,
    pub name: String,
    pub apps: Vec<apps::AppInfo>,
    pub is_expanded: bool,
}

#[derive(Clone, Debug)]
pub struct AppLibraryState {
    pub categories: Vec<AppCategoryData>,
    pub active_category_id: i32,
}

impl AppLibraryState {
    pub fn build(all_apps: &[apps::AppInfo]) -> Self {
        let preferred_order = [
            "Desenvolvimento",
            "Jogos",
            "Multimídia",
            "Gráficos & Design",
            "Internet & Rede",
            "Escritório",
            "Configurações",
            "Sistema",
            "Utilitários",
            "Outros",
        ];

        let mut cat_map: HashMap<String, Vec<apps::AppInfo>> = HashMap::new();
        for app in all_apps {
            let cat_name = if app.category.is_empty() {
                "Outros".to_string()
            } else {
                app.category.clone()
            };
            cat_map.entry(cat_name).or_default().push(app.clone());
        }

        let mut categories = Vec::new();
        for name in &preferred_order {
            if let Some(mut apps) = cat_map.remove(*name) {
                apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                categories.push(AppCategoryData {
                    id: 0,
                    name: name.to_string(),
                    apps,
                    is_expanded: false,
                });
            }
        }

        let mut remaining: Vec<_> = cat_map.into_iter().collect();
        remaining.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
        for (name, mut apps) in remaining {
            apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            categories.push(AppCategoryData {
                id: 0,
                name,
                apps,
                is_expanded: false,
            });
        }

        for (i, cat) in categories.iter_mut().enumerate() {
            cat.id = i as i32;
        }

        if !categories.is_empty() {
            categories[0].is_expanded = true;
        }

        Self {
            categories,
            active_category_id: 0,
        }
    }
}

fn refresh_ui_app_categories(ui: &AppWindow, state: &mut AppLibraryState) {
    let cat_items: Vec<AppCategoryItem> = state
        .categories
        .iter()
        .map(|c| AppCategoryItem {
            id: c.id,
            name: SharedString::from(&c.name),
            count: c.apps.len() as i32,
            is_expanded: c.is_expanded,
        })
        .collect();
    ui.set_app_categories(slint::ModelRc::new(slint::VecModel::from(cat_items)));

    if let Some(active_cat) = state
        .categories
        .iter()
        .find(|c| c.id == state.active_category_id && c.is_expanded)
    {
        let items: Vec<SystemAppItem> = active_cat.apps.iter().map(app_info_to_item).collect();
        ui.set_category_apps(slint::ModelRc::new(slint::VecModel::from(items)));
    } else {
        ui.set_category_apps(slint::ModelRc::new(slint::VecModel::from(Vec::new())));
    }
}

#[derive(Clone, Debug)]
pub struct OfficialIcon {
    pub name: String,
    pub category: String,
    pub path: String,
}

#[derive(Clone, Debug)]
pub struct CategoryData {
    pub id: i32,
    pub name: String,
    pub folder_path: PathBuf,
    pub icons: Vec<OfficialIcon>,
    pub is_expanded: bool,
}

#[derive(Clone, Debug)]
pub struct IconLibraryState {
    pub categories: Vec<CategoryData>,
    pub active_category_id: i32,
}

fn scan_directory_icons(dir: &Path, category_name: &str) -> Vec<OfficialIcon> {
    let mut icons = Vec::new();
    let mut seen = std::collections::HashSet::new();

    fn visit_dir(
        dir: &Path,
        category_name: &str,
        icons: &mut Vec<OfficialIcon>,
        seen: &mut std::collections::HashSet<String>,
    ) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    visit_dir(&p, category_name, icons, seen);
                } else if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                    let ext_lower = ext.to_lowercase();
                    if ["png", "jpg", "jpeg", "svg", "webp", "bmp", "ico"].contains(&ext_lower.as_str()) {
                        let path_str = p.canonicalize().unwrap_or(p.clone()).to_string_lossy().to_string();
                        if seen.insert(path_str.clone()) {
                            let file_stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("icon");
                            let friendly_name = file_stem
                                .trim_start_matches("btn_")
                                .trim_start_matches("user_")
                                .replace('_', " ")
                                .replace('-', " ");
                            icons.push(OfficialIcon {
                                name: friendly_name,
                                category: category_name.to_string(),
                                path: path_str,
                            });
                        }
                    }
                }
            }
        }
    }

    visit_dir(dir, category_name, &mut icons, &mut seen);
    icons.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    icons
}

impl IconLibraryState {
    pub fn scan() -> Self {
        let mut state = Self {
            categories: Vec::new(),
            active_category_id: 0,
        };
        state.rescan();
        state
    }

    pub fn rescan(&mut self) {
        let base_icons_dir = dirs::config_dir()
            .map(|d| d.join("uzlinux-studio/icons"))
            .unwrap_or_else(|| PathBuf::from("icons"));
        let _ = std::fs::create_dir_all(&base_icons_dir);

        let mut category_map: HashMap<String, (PathBuf, Vec<OfficialIcon>)> = HashMap::new();

        // 1. Scan user packs in ~/.config/uzlinux-studio/icons/*
        if let Ok(entries) = std::fs::read_dir(&base_icons_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    let folder_name = p.file_name().and_then(|n| n.to_str()).unwrap_or("Pacote").to_string();
                    let display_name = match folder_name.as_str() {
                        "ulanzi_defaults" => "Ulanzi Padrão".to_string(),
                        "Anaglyph" => "Anaglyph".to_string(),
                        "code" => "code".to_string(),
                        other => other.to_string(),
                    };
                    let icons = scan_directory_icons(&p, &display_name);
                    if !icons.is_empty() {
                        category_map.insert(display_name, (p, icons));
                    }
                }
            }
        }

        // 2. Built-in actions and general icons
        let actions_dir = PathBuf::from("assets/ulanzi_official/actions");
        if actions_dir.exists() {
            let icons = scan_directory_icons(&actions_dir, "Ulanzi Ações");
            if !icons.is_empty() {
                category_map.insert("Ulanzi Ações".to_string(), (actions_dir, icons));
            }
        }

        let icons_dir = PathBuf::from("assets/ulanzi_official/icons");
        if icons_dir.exists() {
            let icons = scan_directory_icons(&icons_dir, "Ulanzi Geral");
            if !icons.is_empty() {
                category_map.insert("Ulanzi Geral".to_string(), (icons_dir, icons));
            }
        }

        // 3. Preferred display ordering (faithful to user screenshot media_1792187430752_6f1ad25c.png)
        let preferred_order = ["Anaglyph", "code", "Ulanzi Padrão", "Ulanzi Ações", "Ulanzi Geral"];
        let mut ordered_cats = Vec::new();

        for name in &preferred_order {
            if let Some((path, icons)) = category_map.remove(*name) {
                ordered_cats.push(CategoryData {
                    id: 0,
                    name: name.to_string(),
                    folder_path: path,
                    icons,
                    is_expanded: false,
                });
            }
        }

        // Append any user imported packs
        let mut remaining: Vec<_> = category_map.into_iter().collect();
        remaining.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
        for (name, (path, icons)) in remaining {
            ordered_cats.push(CategoryData {
                id: 0,
                name,
                folder_path: path,
                icons,
                is_expanded: false,
            });
        }

        for (i, cat) in ordered_cats.iter_mut().enumerate() {
            cat.id = i as i32;
        }

        if !ordered_cats.is_empty() {
            ordered_cats[0].is_expanded = true;
            self.active_category_id = 0;
        }

        self.categories = ordered_cats;
    }

    pub fn get_all_icons(&self) -> Vec<OfficialIcon> {
        let mut all = Vec::new();
        for cat in &self.categories {
            all.extend(cat.icons.clone());
        }
        all
    }
}

fn build_grid_rows(icons: &[OfficialIcon]) -> Vec<IconGridRow> {
    let mut rows = Vec::new();

    for chunk in icons.chunks(8) {
        let mut row = IconGridRow {
            item_0_valid: false, item_0: IconGridItem::default(),
            item_1_valid: false, item_1: IconGridItem::default(),
            item_2_valid: false, item_2: IconGridItem::default(),
            item_3_valid: false, item_3: IconGridItem::default(),
            item_4_valid: false, item_4: IconGridItem::default(),
            item_5_valid: false, item_5: IconGridItem::default(),
            item_6_valid: false, item_6: IconGridItem::default(),
            item_7_valid: false, item_7: IconGridItem::default(),
        };

        let make_item = |ic: &OfficialIcon| -> IconGridItem {
            let (has_img, img) = load_slint_image(&ic.path);
            IconGridItem {
                name: SharedString::from(&ic.name),
                category: SharedString::from(&ic.category),
                path: SharedString::from(&ic.path),
                has_image: has_img,
                image: img,
            }
        };

        if chunk.len() > 0 { row.item_0_valid = true; row.item_0 = make_item(&chunk[0]); }
        if chunk.len() > 1 { row.item_1_valid = true; row.item_1 = make_item(&chunk[1]); }
        if chunk.len() > 2 { row.item_2_valid = true; row.item_2 = make_item(&chunk[2]); }
        if chunk.len() > 3 { row.item_3_valid = true; row.item_3 = make_item(&chunk[3]); }
        if chunk.len() > 4 { row.item_4_valid = true; row.item_4 = make_item(&chunk[4]); }
        if chunk.len() > 5 { row.item_5_valid = true; row.item_5 = make_item(&chunk[5]); }
        if chunk.len() > 6 { row.item_6_valid = true; row.item_6 = make_item(&chunk[6]); }
        if chunk.len() > 7 { row.item_7_valid = true; row.item_7 = make_item(&chunk[7]); }

        rows.push(row);
    }

    rows
}

fn refresh_ui_categories(ui: &AppWindow, state: &mut IconLibraryState) {
    let cat_items: Vec<IconCategoryItem> = state
        .categories
        .iter()
        .map(|c| IconCategoryItem {
            id: c.id,
            name: SharedString::from(&c.name),
            count: c.icons.len() as i32,
            is_expanded: c.is_expanded,
        })
        .collect();
    ui.set_icon_categories(slint::ModelRc::new(slint::VecModel::from(cat_items)));

    if let Some(active_cat) = state
        .categories
        .iter()
        .find(|c| c.id == state.active_category_id && c.is_expanded)
    {
        let rows = build_grid_rows(&active_cat.icons);
        ui.set_category_icon_rows(slint::ModelRc::new(slint::VecModel::from(rows)));
    } else {
        ui.set_category_icon_rows(slint::ModelRc::new(slint::VecModel::from(Vec::new())));
    }
}

fn extract_zip_icon_pack(zip_path: &Path) -> Result<(String, usize), String> {
    let file = std::fs::File::open(zip_path).map_err(|e| format!("Falha ao abrir zip: {}", e))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("Arquivo zip inválido: {}", e))?;

    let file_stem = zip_path.file_stem().and_then(|s| s.to_str()).unwrap_or("Pacote");
    let pack_name = file_stem.replace('_', " ").replace('-', " ");

    let target_dir = dirs::config_dir()
        .map(|d| d.join("uzlinux-studio/icons").join(&pack_name))
        .unwrap_or_else(|| PathBuf::from("icons").join(&pack_name));
    std::fs::create_dir_all(&target_dir).map_err(|e| format!("Falha ao criar diretório: {}", e))?;

    let mut extracted_count = 0;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| format!("Erro no arquivo zip: {}", e))?;
        let entry_name = match file.name() {
            Ok(n) => n.to_string(),
            Err(_) => continue,
        };

        if entry_name.contains("__MACOSX") || entry_name.contains("/.") {
            continue;
        }

        let p = Path::new(&entry_name);
        if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
            let ext_lower = ext.to_lowercase();
            if ["png", "jpg", "jpeg", "svg", "webp", "bmp", "ico"].contains(&ext_lower.as_str()) {
                let file_name = p.file_name().unwrap_or_default();
                let outpath = target_dir.join(file_name);
                if let Ok(mut outfile) = std::fs::File::create(&outpath) {
                    if std::io::copy(&mut file, &mut outfile).is_ok() {
                        extracted_count += 1;
                    }
                }
            }
        }
    }

    Ok((pack_name, extracted_count))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let main_window = AppWindow::new()?;
    let ui_weak = main_window.as_weak();

    // Shared thread-safe persistent profile configuration
    let profile = Arc::new(Mutex::new(ProfileConfig::load()));

    // Migrate keys with old padded icons to crisp full-bleed icons & ensure key 13 clock
    {
        let mut guard = profile.lock().unwrap();
        for k in &mut guard.keys {
            if k.icon_path.contains("btn_media_play_pause") {
                k.icon_path = "assets/ulanzi_official/icons/Play.png".to_string();
            } else if k.icon_path.contains("btn_media_next_track") {
                k.icon_path = "assets/ulanzi_official/icons/Next Track.png".to_string();
            } else if k.icon_path.contains("btn_media_prev_track") {
                k.icon_path = "assets/ulanzi_official/icons/Previous Track.png".to_string();
            } else if k.icon_path.contains("btn_media_volume_up") {
                k.icon_path = "assets/ulanzi_official/icons/Volume Up.png".to_string();
            } else if k.icon_path.contains("btn_media_volume_down") {
                k.icon_path = "assets/ulanzi_official/icons/Volume Down.png".to_string();
            } else if k.icon_path.contains("btn_media_mute") {
                k.icon_path = "assets/ulanzi_official/icons/Mute.png".to_string();
            } else if k.icon_path.contains("btn_media_stop") {
                k.icon_path = "assets/ulanzi_official/icons/Stop.png".to_string();
            }
        }
        if let Some(key13) = guard.keys.get_mut(13) {
            if key13.action_type.is_empty() || (key13.action_type == "hotkey" && key13.title == "Colar") {
                key13.title = "Relógio".to_string();
                key13.action_type = "clock".to_string();
                key13.action_value = "clock".to_string();
                key13.icon_path = "".to_string();
            }
        }
    }

    // Live UI Clock Timer (ticks every second matching physical deck time)
    main_window.set_current_time_str(SharedString::from(
        chrono::Local::now().format("%H : %M").to_string(),
    ));
    let clock_timer = slint::Timer::default();
    let clock_ui_weak = ui_weak.clone();
    clock_timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_secs(1),
        move || {
            if let Some(ui) = clock_ui_weak.upgrade() {
                ui.set_current_time_str(SharedString::from(
                    chrono::Local::now().format("%H : %M").to_string(),
                ));
            }
        },
    );

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
    }
    main_window.set_keys(keys_model.clone().into());
    {
        let guard = profile.lock().unwrap();
        refresh_ui_profiles_and_pages(&main_window, &guard);
    }

    // Scan installed system applications
    let system_apps = Arc::new(apps::scan_system_apps());
    let filtered_apps_model = Rc::new(VecModel::<SystemAppItem>::default());
    update_filtered_apps_model(&filtered_apps_model, &system_apps, "");
    main_window.set_filtered_apps(filtered_apps_model.clone().into());

    // Scan and populate categorized icon library
    let icon_lib_state = Arc::new(Mutex::new(IconLibraryState::scan()));
    {
        let mut state_guard = icon_lib_state.lock().unwrap();
        refresh_ui_categories(&main_window, &mut state_guard);
    }

    // Scan and populate categorized application library
    let app_lib_state = Arc::new(Mutex::new(AppLibraryState::build(&system_apps)));
    {
        let mut app_state_guard = app_lib_state.lock().unwrap();
        refresh_ui_app_categories(&main_window, &mut app_state_guard);
    }

    // Initialize Ulanzi Deck Hardware Controller
    let deck = UlanziDeck::new();
    let (tx, rx) = mpsc::channel::<DeckEvent>();

    // Connect to hardware & check udev permission status
    if !udev::is_udev_rule_installed() {
        main_window.set_udev_missing(true);
    }

    match deck.connect(tx) {
        Ok(path) => {
            main_window.set_device_connected(true);
            main_window.set_udev_missing(false);
            main_window.set_device_node(SharedString::from(path.to_string_lossy().to_string()));
            main_window.set_status_message(SharedString::from(format!(
                "Conectado com sucesso em {}",
                path.display()
            )));

            let b = profile.lock().unwrap().brightness;
            let _ = deck.set_brightness(b);
            let _ = deck.set_label_style(true);
            let _ = deck.query_device_info();
        }
        Err(err) => {
            main_window.set_device_connected(false);
            if err.contains("permissão") || !udev::is_udev_rule_installed() {
                main_window.set_udev_missing(true);
            }
            main_window.set_status_message(SharedString::from(format!("Aviso: {}", err)));
        }
    }

    // Initialize Settings Page State
    main_window.set_autostart_enabled(autostart::is_autostart_enabled());
    main_window.set_device_firmware_version("v1.0.0".into());
    main_window.set_device_serial_number("D200-OFICIAL".into());
    main_window.set_app_version_string("0.1.0".into());
    let cfg_path = config::ProfileConfig::config_path();
    if let Some(parent) = cfg_path.parent() {
        main_window.set_profiles_dir_path(parent.to_string_lossy().to_string().into());
    }
    if let Some(mut icons_p) = dirs::config_dir() {
        icons_p.push("uzlinux-studio");
        icons_p.push("icons");
        main_window.set_icons_dir_path(icons_p.to_string_lossy().to_string().into());
    }

    // Real-time debounced auto-sync channel.
    // SyncMsg::Key(id) = single key changed → fast partial update via CMD_OUT_PARTIALLY_UPDATE_BUTTONS
    // SyncMsg::Full     = full profile re-send needed (initial connect, brightness change, etc.)
    let (sync_tx, sync_rx) = mpsc::channel::<SyncMsg>();
    // Queue initial full sync
    let _ = sync_tx.send(SyncMsg::Full);

    // Spawn background Auto-Sync worker thread for real-time synchronization
    {
        let deck_clone = deck.clone();
        let profile_arc = profile.clone();
        let ui_handle = ui_weak.clone();

        std::thread::Builder::new()
            .name("ulanzi-realtime-sync".into())
            .spawn(move || {
                let mut pending_key: Option<usize> = None;
                let mut pending_full = false;
                let mut last_send = std::time::Instant::now() - Duration::from_secs(10);
                let debounce = Duration::from_millis(80);
                let min_interval_partial = Duration::from_millis(40);
                let min_interval_full = Duration::from_millis(150);

                loop {
                    // Drain rapid burst events within the debounce window
                    loop {
                        match sync_rx.recv_timeout(debounce) {
                            Ok(SyncMsg::Full) => {
                                pending_full = true;
                                pending_key = None;
                            }
                            Ok(SyncMsg::Key(k)) => {
                                if !pending_full {
                                    match pending_key {
                                        None => pending_key = Some(k),
                                        Some(prev) if prev == k => {}
                                        _ => {
                                            // Multiple keys changed rapidly: upgrade to full profile sync
                                            pending_full = true;
                                            pending_key = None;
                                        }
                                    }
                                }
                            }
                            Err(mpsc::RecvTimeoutError::Disconnected) => return,
                            Err(mpsc::RecvTimeoutError::Timeout) => {
                                // Debounce window completed
                                break;
                            }
                        }
                    }

                    if !pending_full && pending_key.is_none() {
                        // Wait for the next event to arrive
                        match sync_rx.recv() {
                            Ok(SyncMsg::Full) => {
                                pending_full = true;
                                pending_key = None;
                            }
                            Ok(SyncMsg::Key(k)) => {
                                pending_key = Some(k);
                            }
                            Err(_) => break,
                        }
                        continue;
                    }

                    if !deck_clone.is_connected() {
                        // Device is not connected: clear pending flags to prevent busy looping
                        pending_full = false;
                        pending_key = None;
                        continue;
                    }

                    // Fast path: Single-key incremental update (CMD_OUT_PARTIALLY_UPDATE_BUTTONS, ~15KB, ~30ms)
                    if !pending_full {
                        if let Some(key_id) = pending_key {
                            let key_info_opt = {
                                let guard = profile_arc.lock().unwrap();
                                guard.keys.get(key_id).map(|k| device::protocol::ProfileKeyInfo {
                                    id: k.id,
                                    title: k.title.clone(),
                                    icon_path: k.icon_path.clone(),
                                    action_type: k.action_type.clone(),
                                })
                            };

                            if let Some(key_info) = key_info_opt {
                                let elapsed = last_send.elapsed();
                                if elapsed < min_interval_partial {
                                    std::thread::sleep(min_interval_partial - elapsed);
                                }

                                let _ = deck_clone.set_label_style(true);
                                match deck_clone.send_key_update(&key_info) {
                                    Ok(_) => {
                                        last_send = std::time::Instant::now();
                                        pending_key = None;
                                        let ui_weak = ui_handle.clone();
                                        let _ = slint::invoke_from_event_loop(move || {
                                            if let Some(ui) = ui_weak.upgrade() {
                                                ui.set_status_message(SharedString::from(format!(
                                                    "Tecla [{}] sincronizada em tempo real ✨",
                                                    key_id
                                                )));
                                            }
                                        });
                                        continue;
                                    }
                                    Err(e) => {
                                        // Fallback to full sync if partial update is rejected (e.g. clock/stats mode)
                                        eprintln!("[Auto-Sync] Atualização parcial caiu para perfil completo: {}", e);
                                    }
                                }
                            }
                        }
                    }

                    // Full sync path: Complete profile bundle upload (CMD_OUT_SET_BUTTONS, ~422KB)
                    let elapsed = last_send.elapsed();
                    if elapsed < min_interval_full {
                        std::thread::sleep(min_interval_full - elapsed);
                    }

                    let keys_info: Vec<device::protocol::ProfileKeyInfo> = {
                        let guard = profile_arc.lock().unwrap();
                        guard.keys.iter().map(|k| device::protocol::ProfileKeyInfo {
                            id: k.id,
                            title: k.title.clone(),
                            icon_path: k.icon_path.clone(),
                            action_type: k.action_type.clone(),
                        }).collect()
                    };

                    match device::protocol::build_d200_profile_zip(&keys_info) {
                        Ok(bundle) => {
                            let _ = deck_clone.set_label_style(true);
                            match deck_clone.send_profile_bundle(&bundle) {
                                Ok(_) => {
                                    last_send = std::time::Instant::now();
                                    pending_full = false;
                                    pending_key = None;
                                    let ui_weak = ui_handle.clone();
                                    let _ = slint::invoke_from_event_loop(move || {
                                        if let Some(ui) = ui_weak.upgrade() {
                                            ui.set_status_message(SharedString::from(
                                                "Perfil sincronizado em tempo real com o Ulanzi D200 ✨",
                                            ));
                                        }
                                    });
                                }
                                Err(e) => {
                                    eprintln!("[Auto-Sync] Erro ao enviar perfil ao D200: {}", e);
                                    pending_full = false;
                                    pending_key = None;
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("[Auto-Sync] Erro ao construir pacote D200: {}", e);
                            pending_full = false;
                            pending_key = None;
                        }
                    }
                }
            })
            .expect("Falha ao iniciar thread de auto-sync em tempo real");
    }


    // Handle UI Callbacks

    // Setup Native Drag and Drop Helper
    {
        let dnd = main_window.global::<DnDHelper>();
        dnd.on_to_transfer(|s| {
            DataTransfer::from(s)
        });
        dnd.on_accept(|dt| {
            dt.has_plain_text() || dt.has_file_paths()
        });
        dnd.on_text_of(|dt| {
            if let Ok(text) = dt.plain_text() {
                return text;
            }
            if let Ok(mut files) = dt.file_paths() {
                if let Some(first) = files.next() {
                    return format!("file://{}", first.display()).into();
                }
            }
            SharedString::default()
        });
    }

    // Key Payload Dropped (Pad button reordering / sidebar action drop / icon drop / file drop)
    {
        let keys_model = keys_model.clone();
        let profile_arc = profile.clone();
        let ui_handle = ui_weak.clone();
        let sync_tx = sync_tx.clone();
        let deck_clone = deck.clone();
        main_window.on_key_payload_dropped(move |dst_id, payload| {
            let dst_usize = dst_id as usize;
            let payload_str = payload.to_string();
            if payload_str.is_empty() {
                return;
            }

            if let Some(src_str) = payload_str.strip_prefix("key:") {
                // --- 1. SWAP / REORDER BUTTONS (UlanziDeck::onItemSwap) ---
                if let Ok(src_usize) = src_str.parse::<usize>() {
                    if src_usize == dst_usize {
                        return;
                    }

                    let mut src_item_data = None;
                    let mut dst_item_data = None;

                    {
                        let mut guard = profile_arc.lock().unwrap();
                        if src_usize < guard.keys.len() && dst_usize < guard.keys.len() {
                            let temp_title = guard.keys[src_usize].title.clone();
                            let temp_act_type = guard.keys[src_usize].action_type.clone();
                            let temp_act_val = guard.keys[src_usize].action_value.clone();
                            let temp_icon_path = guard.keys[src_usize].icon_path.clone();

                            guard.keys[src_usize].title = guard.keys[dst_usize].title.clone();
                            guard.keys[src_usize].action_type = guard.keys[dst_usize].action_type.clone();
                            guard.keys[src_usize].action_value = guard.keys[dst_usize].action_value.clone();
                            guard.keys[src_usize].icon_path = guard.keys[dst_usize].icon_path.clone();

                            guard.keys[dst_usize].title = temp_title;
                            guard.keys[dst_usize].action_type = temp_act_type;
                            guard.keys[dst_usize].action_value = temp_act_val;
                            guard.keys[dst_usize].icon_path = temp_icon_path;

                            guard.save();

                            src_item_data = Some((
                                guard.keys[src_usize].title.clone(),
                                guard.keys[src_usize].action_type.clone(),
                                guard.keys[src_usize].action_value.clone(),
                                guard.keys[src_usize].icon_path.clone(),
                            ));
                            dst_item_data = Some((
                                guard.keys[dst_usize].title.clone(),
                                guard.keys[dst_usize].action_type.clone(),
                                guard.keys[dst_usize].action_value.clone(),
                                guard.keys[dst_usize].icon_path.clone(),
                            ));
                        }
                    }

                    if let (Some((s_title, s_type, s_val, s_icon)), Some((d_title, d_type, d_val, d_icon))) = (src_item_data, dst_item_data) {
                        let (s_has_icon, s_img) = if !s_icon.is_empty() {
                            load_slint_image(&s_icon)
                        } else {
                            (false, slint::Image::default())
                        };
                        if let Some(mut item) = keys_model.row_data(src_usize) {
                            item.title = SharedString::from(&s_title);
                            item.action_type = SharedString::from(&s_type);
                            item.action_value = SharedString::from(&s_val);
                            item.icon_path = SharedString::from(&s_icon);
                            item.has_icon = s_has_icon;
                            item.icon = s_img;
                            keys_model.set_row_data(src_usize, item);
                        }

                        let (d_has_icon, d_img) = if !d_icon.is_empty() {
                            load_slint_image(&d_icon)
                        } else {
                            (false, slint::Image::default())
                        };
                        if let Some(mut item) = keys_model.row_data(dst_usize) {
                            item.title = SharedString::from(&d_title);
                            item.action_type = SharedString::from(&d_type);
                            item.action_value = SharedString::from(&d_val);
                            item.icon_path = SharedString::from(&d_icon);
                            item.has_icon = d_has_icon;
                            item.icon = d_img.clone();
                            keys_model.set_row_data(dst_usize, item);
                        }

                        // Slot 13 clock / stats mode management
                        if src_usize == 13 || dst_usize == 13 {
                            let slot13_type = if dst_usize == 13 { &d_type } else { &s_type };
                            if slot13_type == "clock" {
                                let _ = deck_clone.set_small_window_data(1, 0, 0, &chrono::Local::now().format("%H:%M:%S").to_string(), 0);
                            } else if slot13_type == "system_stats" {
                                let (cpu, mem) = device::get_system_stats();
                                let _ = deck_clone.set_small_window_data(0, cpu, mem, &chrono::Local::now().format("%H:%M:%S").to_string(), 0);
                            }
                        }

                        if let Some(ui) = ui_handle.upgrade() {
                            ui.set_selected_key_index(dst_id);
                            for i in 0..keys_model.row_count() {
                                if let Some(mut item) = keys_model.row_data(i) {
                                    item.is_selected = item.id == dst_id;
                                    keys_model.set_row_data(i, item);
                                }
                            }
                            ui.set_edit_title(SharedString::from(&d_title));
                            ui.set_edit_action_type(SharedString::from(&d_type));
                            ui.set_edit_action_value(SharedString::from(&d_val));
                            ui.set_edit_icon_path(SharedString::from(&d_icon));
                            ui.set_edit_has_icon(d_has_icon);
                            ui.set_edit_icon_preview(d_img);
                            ui.set_status_message(SharedString::from(format!("Teclas {} e {} trocadas!", src_usize + 1, dst_usize + 1)));
                        }

                        let _ = sync_tx.send(SyncMsg::Key(src_usize));
                        let _ = sync_tx.send(SyncMsg::Key(dst_usize));
                    }
                }
            } else if let Some(act_data) = payload_str.strip_prefix("action:") {
                // --- 2. ACTION DROPPED FROM SIDEBAR (UlanziDeck::onItemDropAction) ---
                let parts: Vec<&str> = act_data.splitn(3, ':').collect();
                let act_type = parts.get(0).copied().unwrap_or("");
                let def_val = parts.get(1).copied().unwrap_or("");
                let title = parts.get(2).copied().unwrap_or("");

                if act_type == "none" || act_type == "clear" || act_type.is_empty() {
                    {
                        let mut guard = profile_arc.lock().unwrap();
                        if dst_usize < guard.keys.len() {
                            guard.keys[dst_usize].title = "".to_string();
                            guard.keys[dst_usize].action_type = "".to_string();
                            guard.keys[dst_usize].action_value = "".to_string();
                            guard.keys[dst_usize].icon_path = "".to_string();
                            guard.save();
                        }
                    }
                    if let Some(mut item) = keys_model.row_data(dst_usize) {
                        item.action_type = SharedString::from("");
                        item.action_value = SharedString::from("");
                        item.title = SharedString::from("");
                        item.icon_path = SharedString::from("");
                        item.has_icon = false;
                        item.icon = slint::Image::default();
                        keys_model.set_row_data(dst_usize, item);
                    }
                    if let Some(ui) = ui_handle.upgrade() {
                        ui.set_selected_key_index(dst_id);
                        for i in 0..keys_model.row_count() {
                            if let Some(mut item) = keys_model.row_data(i) {
                                item.is_selected = item.id == dst_id;
                                keys_model.set_row_data(i, item);
                            }
                        }
                        ui.set_edit_title(SharedString::from(""));
                        ui.set_edit_action_type(SharedString::from(""));
                        ui.set_edit_action_value(SharedString::from(""));
                        ui.set_edit_icon_path(SharedString::from(""));
                        ui.set_edit_has_icon(false);
                        ui.set_edit_icon_preview(slint::Image::default());
                        ui.set_status_message(SharedString::from(format!("Tecla {} desativada", dst_usize + 1)));
                    }
                    let _ = sync_tx.send(SyncMsg::Key(dst_usize));
                } else {
                    let auto_icon = if act_type == "clock" || act_type == "system_stats" {
                        Some("")
                    } else {
                        get_official_icon_for_action(act_type, def_val)
                    };

                    let mut final_icon_path = String::new();
                    let mut final_has_icon = false;
                    let mut final_img = slint::Image::default();

                    {
                        let mut guard = profile_arc.lock().unwrap();
                        if dst_usize < guard.keys.len() {
                            guard.keys[dst_usize].action_type = act_type.to_string();
                            guard.keys[dst_usize].action_value = def_val.to_string();
                            guard.keys[dst_usize].title = title.to_string();
                            if let Some(icon_p) = auto_icon {
                                guard.keys[dst_usize].icon_path = icon_p.to_string();
                            } else {
                                guard.keys[dst_usize].icon_path = String::new();
                            }
                            final_icon_path = guard.keys[dst_usize].icon_path.clone();
                            guard.save();
                        }
                    }

                    if !final_icon_path.is_empty() {
                        let (has, img) = load_slint_image(&final_icon_path);
                        final_has_icon = has;
                        final_img = img;
                    }

                    if let Some(mut item) = keys_model.row_data(dst_usize) {
                        item.action_type = SharedString::from(act_type);
                        item.action_value = SharedString::from(def_val);
                        item.title = SharedString::from(title);
                        item.icon_path = SharedString::from(&final_icon_path);
                        item.has_icon = final_has_icon;
                        item.icon = final_img.clone();
                        keys_model.set_row_data(dst_usize, item);
                    }

                    if dst_usize == 13 {
                        if act_type == "clock" {
                            let _ = deck_clone.set_small_window_data(1, 0, 0, &chrono::Local::now().format("%H:%M:%S").to_string(), 0);
                        } else if act_type == "system_stats" {
                            let (cpu, mem) = device::get_system_stats();
                            let _ = deck_clone.set_small_window_data(0, cpu, mem, &chrono::Local::now().format("%H:%M:%S").to_string(), 0);
                        }
                    }

                    if let Some(ui) = ui_handle.upgrade() {
                        ui.set_selected_key_index(dst_id);
                        for i in 0..keys_model.row_count() {
                            if let Some(mut item) = keys_model.row_data(i) {
                                item.is_selected = item.id == dst_id;
                                keys_model.set_row_data(i, item);
                            }
                        }
                        ui.set_edit_action_type(SharedString::from(act_type));
                        ui.set_edit_action_value(SharedString::from(def_val));
                        ui.set_edit_title(SharedString::from(title));
                        ui.set_edit_icon_path(SharedString::from(&final_icon_path));
                        ui.set_edit_has_icon(final_has_icon);
                        ui.set_edit_icon_preview(final_img);
                        ui.set_status_message(SharedString::from(format!("Ação '{}' atribuída à Tecla {}", title, dst_usize + 1)));
                    }

                    let _ = sync_tx.send(SyncMsg::Key(dst_usize));
                }
            } else if let Some(app_data) = payload_str.strip_prefix("app:") {
                // --- 2b. APPLICATION DROPPED FROM APPLICATION PICKER ---
                let parts: Vec<&str> = app_data.split('\n').collect();
                let name = parts.get(0).copied().unwrap_or("App");
                let exec_cmd = parts.get(1).copied().unwrap_or("");
                let icon_path = parts.get(2).copied().unwrap_or("");

                let final_icon_str = if !icon_path.trim().is_empty() {
                    icon_path.trim().to_string()
                } else {
                    "assets/ulanzi_official/actions/btn_open.png".to_string()
                };

                let (has_icon, img) = load_slint_image(&final_icon_str);

                {
                    let mut guard = profile_arc.lock().unwrap();
                    if dst_usize < guard.keys.len() {
                        guard.keys[dst_usize].action_type = "app".to_string();
                        guard.keys[dst_usize].action_value = exec_cmd.to_string();
                        guard.keys[dst_usize].title = name.to_string();
                        guard.keys[dst_usize].icon_path = final_icon_str.clone();
                        guard.save();
                    }
                }

                if let Some(mut item) = keys_model.row_data(dst_usize) {
                    item.action_type = SharedString::from("app");
                    item.action_value = SharedString::from(exec_cmd);
                    item.title = SharedString::from(name);
                    item.icon_path = SharedString::from(&final_icon_str);
                    item.has_icon = has_icon;
                    item.icon = img.clone();
                    keys_model.set_row_data(dst_usize, item);
                }

                if let Some(ui) = ui_handle.upgrade() {
                    let sel_id = ui.get_selected_key_index() as usize;
                    if sel_id == dst_usize {
                        ui.set_edit_action_type(SharedString::from("app"));
                        ui.set_edit_action_value(SharedString::from(exec_cmd));
                        ui.set_edit_title(SharedString::from(name));
                        ui.set_edit_icon_path(SharedString::from(&final_icon_str));
                        ui.set_edit_has_icon(has_icon);
                        ui.set_edit_icon_preview(img);
                    }
                    ui.set_status_message(SharedString::from(format!("Aplicativo '{}' atribuído à Tecla {}!", name, dst_usize + 1)));
                }

                let _ = sync_tx.send(SyncMsg::Key(dst_usize));
            } else if let Some(icon_path) = payload_str.strip_prefix("icon:") {
                // --- 3. ICON DROPPED FROM ICON LIBRARY ---
                let mut final_img = slint::Image::default();
                let mut final_has_icon = false;

                {
                    let mut guard = profile_arc.lock().unwrap();
                    if dst_usize < guard.keys.len() {
                        guard.keys[dst_usize].icon_path = icon_path.to_string();
                        guard.save();
                    }
                }

                if !icon_path.is_empty() {
                    let (has, img) = load_slint_image(icon_path);
                    final_has_icon = has;
                    final_img = img;
                }

                if let Some(mut item) = keys_model.row_data(dst_usize) {
                    item.icon_path = SharedString::from(icon_path);
                    item.has_icon = final_has_icon;
                    item.icon = final_img.clone();
                    keys_model.set_row_data(dst_usize, item);
                }

                if let Some(ui) = ui_handle.upgrade() {
                    ui.set_selected_key_index(dst_id);
                    for i in 0..keys_model.row_count() {
                        if let Some(mut item) = keys_model.row_data(i) {
                            item.is_selected = item.id == dst_id;
                            keys_model.set_row_data(i, item);
                        }
                    }
                    ui.set_edit_icon_path(SharedString::from(icon_path));
                    ui.set_edit_has_icon(final_has_icon);
                    ui.set_edit_icon_preview(final_img);
                    ui.set_status_message(SharedString::from(format!("Ícone atualizado na Tecla {}", dst_usize + 1)));
                }

                let _ = sync_tx.send(SyncMsg::Key(dst_usize));
            } else if payload_str.starts_with("file://") || payload_str.starts_with('/') {
                // --- 4. FILE DROPPED FROM DESKTOP/FILE MANAGER (handleUrlDrop) ---
                let path_str = if let Some(stripped) = payload_str.strip_prefix("file://") {
                    stripped.trim()
                } else {
                    payload_str.trim()
                };
                let file_path = Path::new(path_str);
                let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();

                if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "svg" | "webp" | "bmp" | "gif") {
                    {
                        let mut guard = profile_arc.lock().unwrap();
                        if dst_usize < guard.keys.len() {
                            guard.keys[dst_usize].icon_path = path_str.to_string();
                            guard.save();
                        }
                    }
                    let (has, img) = load_slint_image(path_str);
                    if let Some(mut item) = keys_model.row_data(dst_usize) {
                        item.icon_path = SharedString::from(path_str);
                        item.has_icon = has;
                        item.icon = img.clone();
                        keys_model.set_row_data(dst_usize, item);
                    }
                    if let Some(ui) = ui_handle.upgrade() {
                        ui.set_selected_key_index(dst_id);
                        for i in 0..keys_model.row_count() {
                            if let Some(mut item) = keys_model.row_data(i) {
                                item.is_selected = item.id == dst_id;
                                keys_model.set_row_data(i, item);
                            }
                        }
                        ui.set_edit_icon_path(SharedString::from(path_str));
                        ui.set_edit_has_icon(has);
                        ui.set_edit_icon_preview(img);
                        ui.set_status_message(SharedString::from(format!("Ícone atribuído à Tecla {}", dst_usize + 1)));
                    }
                    let _ = sync_tx.send(SyncMsg::Key(dst_usize));
                } else if ext == "desktop" {
                    if let Some(app) = crate::apps::scan_desktop_file(file_path) {
                        {
                            let mut guard = profile_arc.lock().unwrap();
                            if dst_usize < guard.keys.len() {
                                guard.keys[dst_usize].action_type = "app".to_string();
                                guard.keys[dst_usize].action_value = app.exec_cmd.clone();
                                guard.keys[dst_usize].title = app.name.clone();
                                guard.keys[dst_usize].icon_path = app.icon_path.clone();
                                guard.save();
                            }
                        }
                        let (has, img) = load_slint_image(&app.icon_path);
                        if let Some(mut item) = keys_model.row_data(dst_usize) {
                            item.action_type = SharedString::from("app");
                            item.action_value = SharedString::from(&app.exec_cmd);
                            item.title = SharedString::from(&app.name);
                            item.icon_path = SharedString::from(&app.icon_path);
                            item.has_icon = has;
                            item.icon = img.clone();
                            keys_model.set_row_data(dst_usize, item);
                        }
                        if let Some(ui) = ui_handle.upgrade() {
                            ui.set_selected_key_index(dst_id);
                            for i in 0..keys_model.row_count() {
                                if let Some(mut item) = keys_model.row_data(i) {
                                    item.is_selected = item.id == dst_id;
                                    keys_model.set_row_data(i, item);
                                }
                            }
                            ui.set_edit_action_type(SharedString::from("app"));
                            ui.set_edit_action_value(SharedString::from(&app.exec_cmd));
                            ui.set_edit_title(SharedString::from(&app.name));
                            ui.set_edit_icon_path(SharedString::from(&app.icon_path));
                            ui.set_edit_has_icon(has);
                            ui.set_edit_icon_preview(img);
                            ui.set_status_message(SharedString::from(format!("App '{}' atribuído à Tecla {}", app.name, dst_usize + 1)));
                        }
                        let _ = sync_tx.send(SyncMsg::Key(dst_usize));
                    }
                } else if matches!(ext.as_str(), "mp3" | "wav" | "ogg" | "flac" | "aac") {
                    let file_stem = file_path.file_stem().and_then(|s| s.to_str()).unwrap_or("Áudio");
                    let icon_str = "assets/ulanzi_official/actions/playAudio.svg";
                    {
                        let mut guard = profile_arc.lock().unwrap();
                        if dst_usize < guard.keys.len() {
                            guard.keys[dst_usize].action_type = "sound".to_string();
                            guard.keys[dst_usize].action_value = path_str.to_string();
                            guard.keys[dst_usize].title = file_stem.to_string();
                            guard.keys[dst_usize].icon_path = icon_str.to_string();
                            guard.save();
                        }
                    }
                    let (has, img) = load_slint_image(icon_str);
                    if let Some(mut item) = keys_model.row_data(dst_usize) {
                        item.action_type = SharedString::from("sound");
                        item.action_value = SharedString::from(path_str);
                        item.title = SharedString::from(file_stem);
                        item.icon_path = SharedString::from(icon_str);
                        item.has_icon = has;
                        item.icon = img.clone();
                        keys_model.set_row_data(dst_usize, item);
                    }
                    if let Some(ui) = ui_handle.upgrade() {
                        ui.set_selected_key_index(dst_id);
                        for i in 0..keys_model.row_count() {
                            if let Some(mut item) = keys_model.row_data(i) {
                                item.is_selected = item.id == dst_id;
                                keys_model.set_row_data(i, item);
                            }
                        }
                        ui.set_edit_action_type(SharedString::from("sound"));
                        ui.set_edit_action_value(SharedString::from(path_str));
                        ui.set_edit_title(SharedString::from(file_stem));
                        ui.set_edit_icon_path(SharedString::from(icon_str));
                        ui.set_edit_has_icon(has);
                        ui.set_edit_icon_preview(img);
                        ui.set_status_message(SharedString::from(format!("Áudio '{}' atribuído à Tecla {}", file_stem, dst_usize + 1)));
                    }
                    let _ = sync_tx.send(SyncMsg::Key(dst_usize));
                } else {
                    let file_name = file_path.file_name().and_then(|s| s.to_str()).unwrap_or("Script");
                    let icon_str = "assets/ulanzi_official/actions/actionTrigger.svg";
                    {
                        let mut guard = profile_arc.lock().unwrap();
                        if dst_usize < guard.keys.len() {
                            guard.keys[dst_usize].action_type = "command".to_string();
                            guard.keys[dst_usize].action_value = path_str.to_string();
                            guard.keys[dst_usize].title = file_name.to_string();
                            guard.keys[dst_usize].icon_path = icon_str.to_string();
                            guard.save();
                        }
                    }
                    let (has, img) = load_slint_image(icon_str);
                    if let Some(mut item) = keys_model.row_data(dst_usize) {
                        item.action_type = SharedString::from("command");
                        item.action_value = SharedString::from(path_str);
                        item.title = SharedString::from(file_name);
                        item.icon_path = SharedString::from(icon_str);
                        item.has_icon = has;
                        item.icon = img.clone();
                        keys_model.set_row_data(dst_usize, item);
                    }
                    if let Some(ui) = ui_handle.upgrade() {
                        ui.set_selected_key_index(dst_id);
                        for i in 0..keys_model.row_count() {
                            if let Some(mut item) = keys_model.row_data(i) {
                                item.is_selected = item.id == dst_id;
                                keys_model.set_row_data(i, item);
                            }
                        }
                        ui.set_edit_action_type(SharedString::from("command"));
                        ui.set_edit_action_value(SharedString::from(path_str));
                        ui.set_edit_title(SharedString::from(file_name));
                        ui.set_edit_icon_path(SharedString::from(icon_str));
                        ui.set_edit_has_icon(has);
                        ui.set_edit_icon_preview(img);
                        ui.set_status_message(SharedString::from(format!("Script '{}' atribuído à Tecla {}", file_name, dst_usize + 1)));
                    }
                    let _ = sync_tx.send(SyncMsg::Key(dst_usize));
                }
            }
        });
    }

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
        let sync_tx = sync_tx.clone();
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
                    guard.save();
                }

                let _ = sync_tx.send(SyncMsg::Key(sel_id));
            }
        });
    }

    // 3. Clear key icon
    {
        let ui_handle = ui_weak.clone();
        let keys_model = keys_model.clone();
        let profile_arc = profile.clone();
        let sync_tx = sync_tx.clone();
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

                let _ = sync_tx.send(SyncMsg::Key(sel_id));
                ui.set_status_message(SharedString::from("Ícone removido da tecla"));
            }
        });
    }

    // 4. Browse Custom File Icon
    {
        let ui_handle = ui_weak.clone();
        let profile_arc = profile.clone();
        let sync_tx = sync_tx.clone();
        main_window.on_browse_icon_custom(move || {
            let initial = dirs::picture_dir().or_else(dirs::home_dir);
            open_file_dialog(
                initial,
                "Selecionar Ícone / Imagem Customizada",
                ui_handle.clone(),
                profile_arc.clone(),
                sync_tx.clone(),
            );
        });
    }

    // 5. Browse System Icons
    {
        let ui_handle = ui_weak.clone();
        let profile_arc = profile.clone();
        let sync_tx = sync_tx.clone();
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
                sync_tx.clone(),
            );
        });
    }

    // 6. Browse Ulanzi Extracted Icons (Opens built-in Icon Library modal)
    {
        let ui_handle = ui_weak.clone();
        let icon_lib_state = icon_lib_state.clone();
        main_window.on_browse_icon_ulanzi(move || {
            if let Some(ui) = ui_handle.upgrade() {
                ui.set_icon_filter_text(SharedString::from(""));
                ui.set_is_searching_icons(false);
                let mut state_guard = icon_lib_state.lock().unwrap();
                refresh_ui_categories(&ui, &mut state_guard);
                ui.set_show_icon_library(true);
            }
        });
    }

    // 7. Save Key Configuration
    {
        let keys_model = keys_model.clone();
        let ui_handle = ui_weak.clone();
        let profile_arc = profile.clone();
        let sync_tx = sync_tx.clone();
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

            let _ = sync_tx.send(SyncMsg::Key(id_usize));

            if let Some(ui) = ui_handle.upgrade() {
                ui.set_status_message(SharedString::from(format!(
                    "Configuração da tecla [{}] salva com sucesso!",
                    pos_str
                )));
            }
        });
    }

    // Action Type Changed Callback (Instant UI sync and model update)
    {
        let keys_model = keys_model.clone();
        let profile_arc = profile.clone();
        let ui_handle = ui_weak.clone();
        let sync_tx = sync_tx.clone();
        let deck_clone = deck.clone();
        main_window.on_action_type_changed(move |id, act_type| {
            let id_usize = id as usize;
            let act_type_str = act_type.to_string();

            let (def_title, def_val, def_icon) = match act_type_str.as_str() {
                "app" => ("App", "", "assets/ulanzi_official/actions/btn_open.png"),
                "hotkey" => ("Atalho", "", "assets/ulanzi_official/actions/btn_hotkey.png"),
                "website" => ("Website", "https://", "assets/ulanzi_official/actions/btn_website.png"),
                "text" => ("Texto", "", "assets/ulanzi_official/actions/btn_text.png"),
                "command" => ("Comando", "", "assets/ulanzi_official/actions/btn_open.png"),
                "sound" => ("Áudio", "", "assets/ulanzi_official/actions/btn_playAudio.png"),
                "multiaction" => ("Multi-Ação", "", "assets/ulanzi_official/actions/btn_multiAction.png"),
                "obs" => ("OBS", "record", "assets/ulanzi_official/actions/btn_record_off.png"),
                "close" => ("Fechar", "window", "assets/ulanzi_official/actions/btn_close.png"),
                "media" => ("Play/Pause", "playpause", "assets/ulanzi_official/icons/Play.png"),
                "clock" => ("Relógio", "clock", ""),
                "system_stats" => ("Monitor", "cpu", ""),
                "page.next" => ("Próx. Pág", "next", "assets/ulanzi_official/actions/btn_nextPage.png"),
                "page.prev" => ("Pág. Ant", "prev", "assets/ulanzi_official/actions/btn_previousPage.png"),
                "page.goto" => ("Página 1", "1", "assets/ulanzi_official/actions/btn_goToPage.png"),
                "page.switch" => ("Mudar Perfil", "", "assets/ulanzi_official/actions/btn_switchProfile.png"),
                "" | "none" | "clear" => ("", "", ""),
                _ => ("", "", ""),
            };

            let mut final_icon_path = def_icon.to_string();
            let mut final_title = def_title.to_string();
            let mut final_val = def_val.to_string();

            {
                let mut guard = profile_arc.lock().unwrap();
                if id_usize < guard.keys.len() {
                    let old_type = guard.keys[id_usize].action_type.clone();
                    if old_type != act_type_str {
                        guard.keys[id_usize].action_type = act_type_str.clone();
                        guard.keys[id_usize].title = def_title.to_string();
                        guard.keys[id_usize].action_value = def_val.to_string();
                        guard.keys[id_usize].icon_path = def_icon.to_string();
                        guard.save();
                    } else {
                        final_icon_path = guard.keys[id_usize].icon_path.clone();
                        final_title = guard.keys[id_usize].title.clone();
                        final_val = guard.keys[id_usize].action_value.clone();
                    }
                }
            }

            let (has_icon, img) = if !final_icon_path.is_empty() {
                load_slint_image(&final_icon_path)
            } else {
                (false, slint::Image::default())
            };

            if let Some(mut item) = keys_model.row_data(id_usize) {
                item.action_type = act_type.clone();
                item.title = SharedString::from(&final_title);
                item.action_value = SharedString::from(&final_val);
                item.icon_path = SharedString::from(&final_icon_path);
                item.has_icon = has_icon;
                item.icon = img.clone();
                keys_model.set_row_data(id_usize, item);
            }

            if id_usize == 13 {
                if act_type_str == "clock" {
                    let _ = deck_clone.set_small_window_data(1, 0, 0, &chrono::Local::now().format("%H:%M:%S").to_string(), 0);
                } else if act_type_str == "system_stats" {
                    let (cpu, mem) = device::get_system_stats();
                    let _ = deck_clone.set_small_window_data(0, cpu, mem, &chrono::Local::now().format("%H:%M:%S").to_string(), 0);
                }
            }

            if let Some(ui) = ui_handle.upgrade() {
                ui.set_edit_action_type(act_type.clone());
                ui.set_edit_title(SharedString::from(&final_title));
                ui.set_edit_action_value(SharedString::from(&final_val));
                ui.set_edit_icon_path(SharedString::from(&final_icon_path));
                ui.set_edit_has_icon(has_icon);
                ui.set_edit_icon_preview(img);
                ui.set_status_message(SharedString::from(format!(
                    "Tipo de ação da tecla [{}] alterado para '{}'",
                    id, act_type
                )));
            }

            let _ = sync_tx.send(SyncMsg::Key(id_usize));
        });
    }

    // Action Value Changed Callback (Real-time sync while typing)
    {
        let keys_model = keys_model.clone();
        let profile_arc = profile.clone();
        let sync_tx = sync_tx.clone();
        main_window.on_action_value_changed(move |id, act_val| {
            let id_usize = id as usize;
            {
                let mut guard = profile_arc.lock().unwrap();
                if id_usize < guard.keys.len() {
                    guard.keys[id_usize].action_value = act_val.to_string();
                    guard.save();
                }
            }
            if let Some(mut item) = keys_model.row_data(id_usize) {
                item.action_value = act_val.clone();
                keys_model.set_row_data(id_usize, item);
            }
            let _ = sync_tx.send(SyncMsg::Key(id_usize));
        });
    }

    // Title Changed Callback (Real-time sync on deck pad while typing)
    {
        let keys_model = keys_model.clone();
        let profile_arc = profile.clone();
        let sync_tx = sync_tx.clone();
        main_window.on_title_changed(move |id, title| {
            let id_usize = id as usize;
            {
                let mut guard = profile_arc.lock().unwrap();
                if id_usize < guard.keys.len() {
                    guard.keys[id_usize].title = title.to_string();
                    guard.save();
                }
            }
            if let Some(mut item) = keys_model.row_data(id_usize) {
                item.title = title.clone();
                keys_model.set_row_data(id_usize, item);
            }
            let _ = sync_tx.send(SyncMsg::Key(id_usize));
        });
    }

    // Quick Preset Applied Callback (Instant one-click presets)
    {
        let keys_model = keys_model.clone();
        let profile_arc = profile.clone();
        let ui_handle = ui_weak.clone();
        let sync_tx = sync_tx.clone();
        let deck_clone = deck.clone();
        main_window.on_apply_preset(move |id, act_type, act_val, title| {
            let id_usize = id as usize;

            let auto_icon = if act_type == "clock" || act_type == "system_stats" {
                Some("")
            } else {
                get_official_icon_for_action(&act_type, &act_val)
            };

            let mut final_icon_path = String::new();
            let mut final_has_icon = false;
            let mut final_img = slint::Image::default();

            {
                let mut guard = profile_arc.lock().unwrap();
                if id_usize < guard.keys.len() {
                    guard.keys[id_usize].action_type = act_type.to_string();
                    guard.keys[id_usize].action_value = act_val.to_string();
                    guard.keys[id_usize].title = title.to_string();
                    if let Some(icon_p) = auto_icon {
                        guard.keys[id_usize].icon_path = icon_p.to_string();
                    } else {
                        guard.keys[id_usize].icon_path = String::new();
                    }
                    final_icon_path = guard.keys[id_usize].icon_path.clone();
                    guard.save();
                }
            }

            if !final_icon_path.is_empty() {
                let (has, img) = load_slint_image(&final_icon_path);
                final_has_icon = has;
                final_img = img;
            }

            if let Some(mut item) = keys_model.row_data(id_usize) {
                item.action_type = act_type.clone();
                item.action_value = act_val.clone();
                item.title = title.clone();
                item.icon_path = SharedString::from(&final_icon_path);
                item.has_icon = final_has_icon;
                item.icon = final_img.clone();
                keys_model.set_row_data(id_usize, item);
            }

            if id_usize == 13 {
                if act_type == "clock" {
                    let _ = deck_clone.set_small_window_data(1, 0, 0, &chrono::Local::now().format("%H:%M:%S").to_string(), 0);
                } else if act_type == "system_stats" {
                    let (cpu, mem) = device::get_system_stats();
                    let _ = deck_clone.set_small_window_data(0, cpu, mem, &chrono::Local::now().format("%H:%M:%S").to_string(), 0);
                }
            }

            let _ = sync_tx.send(SyncMsg::Key(id_usize));

            if let Some(ui) = ui_handle.upgrade() {
                ui.set_edit_action_type(act_type.clone());
                ui.set_edit_action_value(act_val.clone());
                ui.set_edit_title(title.clone());
                ui.set_edit_icon_path(SharedString::from(&final_icon_path));
                ui.set_edit_has_icon(final_has_icon);
                ui.set_edit_icon_preview(final_img);
                ui.set_status_message(SharedString::from(format!(
                    "Ação pré-definida aplicada à tecla [{}]: {}",
                    id, title
                )));
            }
        });
    }

    // Clear Key Action Callback
    {
        let keys_model = keys_model.clone();
        let profile_arc = profile.clone();
        let ui_handle = ui_weak.clone();
        let sync_tx = sync_tx.clone();

        main_window.on_clear_key_action(move |id| {
            let id_usize = id as usize;
            {
                let mut guard = profile_arc.lock().unwrap();
                if id_usize < guard.keys.len() {
                    guard.keys[id_usize].title = "".to_string();
                    guard.keys[id_usize].action_type = "".to_string();
                    guard.keys[id_usize].action_value = "".to_string();
                    guard.keys[id_usize].icon_path = "".to_string();
                    guard.save();
                }
            }

            if let Some(mut item) = keys_model.row_data(id_usize) {
                item.title = SharedString::from("");
                item.action_type = SharedString::from("");
                item.action_value = SharedString::from("");
                item.icon_path = SharedString::from("");
                item.has_icon = false;
                item.icon = slint::Image::default();
                keys_model.set_row_data(id_usize, item);
            }

            if let Some(ui) = ui_handle.upgrade() {
                ui.set_edit_title(SharedString::from(""));
                ui.set_edit_action_type(SharedString::from(""));
                ui.set_edit_action_value(SharedString::from(""));
                ui.set_edit_icon_path(SharedString::from(""));
                ui.set_edit_has_icon(false);
                ui.set_edit_icon_preview(slint::Image::default());
                ui.set_status_message(SharedString::from(format!("Tecla [{}] desativada e limpa (vazia)", id)));
            }

            let _ = sync_tx.send(SyncMsg::Key(id_usize));
        });
    }

    // 8. Send Profile & Icons to Deck LCD (streams valid D200 ZIP profile bundle over HID)
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

            // Sync currently edited values from UI into memory profile
            let ui_icon = current_ui.get_edit_icon_path().to_string();
            let ui_title = current_ui.get_edit_title().to_string();
            let ui_act_type = current_ui.get_edit_action_type().to_string();
            let ui_act_val = current_ui.get_edit_action_value().to_string();

            {
                let mut guard = profile_arc.lock().unwrap();
                if id_usize < guard.keys.len() {
                    guard.keys[id_usize].icon_path = ui_icon.trim().to_string();
                    guard.keys[id_usize].title = ui_title.trim().to_string();
                    guard.keys[id_usize].action_type = ui_act_type.trim().to_string();
                    guard.keys[id_usize].action_value = ui_act_val.trim().to_string();
                    guard.save();
                }
            }

            if let Some(mut item) = keys_model.row_data(id_usize) {
                let (has_icon, img) = load_slint_image(&ui_icon);
                item.icon_path = SharedString::from(&ui_icon);
                item.has_icon = has_icon;
                item.icon = img;
                item.title = SharedString::from(&ui_title);
                item.action_type = SharedString::from(&ui_act_type);
                item.action_value = SharedString::from(&ui_act_val);
                keys_model.set_row_data(id_usize, item);
            }

            // Build full profile bundle containing manifest.json and 196x196 PNGs
            let keys_info: Vec<device::protocol::ProfileKeyInfo> = {
                let guard = profile_arc.lock().unwrap();
                guard.keys.iter().map(|k| device::protocol::ProfileKeyInfo {
                    id: k.id,
                    title: k.title.clone(),
                    icon_path: k.icon_path.clone(),
                    action_type: k.action_type.clone(),
                }).collect()
            };

            match device::protocol::build_d200_profile_zip(&keys_info) {
                Ok(zip_bytes) => {
                    let _ = deck_clone.set_label_style(true);
                    match deck_clone.send_profile_bundle(&zip_bytes) {
                        Ok(_) => {
                            current_ui.set_status_message(SharedString::from(format!(
                                "Perfil e ícones enviados ao Ulanzi D200 com sucesso! (Tecla [{}])",
                                id
                            )));
                        }
                        Err(e) => {
                            current_ui.set_status_message(SharedString::from(format!(
                                "Erro ao transmitir pacote ao Deck: {}",
                                e
                            )));
                        }
                    }
                }
                Err(e) => {
                    current_ui.set_status_message(SharedString::from(format!(
                        "Erro ao gerar pacote ZIP para o Deck: {}",
                        e
                    )));
                }
            }
        });
    }

    // 9. System Application Picker Callbacks (Categorized Accordion & Search)
    {
        let ui_handle = ui_weak.clone();
        let app_lib_state = app_lib_state.clone();

        main_window.on_open_app_picker(move || {
            if let Some(ui) = ui_handle.upgrade() {
                ui.set_app_filter_text(SharedString::default());
                ui.set_is_searching_apps(false);
                let mut state_guard = app_lib_state.lock().unwrap();
                refresh_ui_app_categories(&ui, &mut state_guard);
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
        let ui_handle = ui_weak.clone();
        let app_lib_state = app_lib_state.clone();
        main_window.on_toggle_app_category_expanded(move |cat_id| {
            if let Some(ui) = ui_handle.upgrade() {
                let mut state_guard = app_lib_state.lock().unwrap();
                if state_guard.active_category_id == cat_id {
                    if let Some(cat) = state_guard.categories.iter_mut().find(|c| c.id == cat_id) {
                        cat.is_expanded = !cat.is_expanded;
                    }
                } else {
                    state_guard.active_category_id = cat_id;
                    for cat in &mut state_guard.categories {
                        cat.is_expanded = cat.id == cat_id;
                    }
                }
                ui.set_active_app_category_id(state_guard.active_category_id);
                refresh_ui_app_categories(&ui, &mut state_guard);
            }
        });
    }

    {
        let ui_handle = ui_weak.clone();
        let system_apps = system_apps.clone();
        let filtered_apps_model = filtered_apps_model.clone();
        main_window.on_app_filter_changed(move |query| {
            if let Some(ui) = ui_handle.upgrade() {
                let q = query.trim().to_lowercase();
                if q.is_empty() {
                    ui.set_is_searching_apps(false);
                } else {
                    ui.set_is_searching_apps(true);
                    let mut matches = Vec::new();
                    for app in system_apps.iter() {
                        if app.name.to_lowercase().contains(&q)
                            || app.exec_cmd.to_lowercase().contains(&q)
                            || app.category.to_lowercase().contains(&q)
                        {
                            matches.push(app_info_to_item(app));
                            if matches.len() >= 80 {
                                break;
                            }
                        }
                    }
                    ui.set_app_search_result_count(matches.len() as i32);
                    filtered_apps_model.set_vec(matches);
                }
            }
        });
    }

    {
        let ui_handle = ui_weak.clone();
        let keys_model = keys_model.clone();
        let profile_arc = profile.clone();
        let sync_tx = sync_tx.clone();

        main_window.on_app_selected(move |name, exec_cmd, icon_path| {
            if let Some(ui) = ui_handle.upgrade() {
                let sel_id = ui.get_selected_key_index() as usize;
                let icon_str = icon_path.as_str().trim();

                let final_icon_str = if !icon_str.is_empty() {
                    icon_str.to_string()
                } else {
                    "assets/ulanzi_official/actions/btn_open.png".to_string()
                };

                let (has_icon, img) = load_slint_image(&final_icon_str);

                ui.set_edit_action_type(SharedString::from("app"));
                ui.set_edit_action_value(exec_cmd.clone());
                ui.set_edit_title(name.clone());
                ui.set_edit_icon_path(SharedString::from(&final_icon_str));
                ui.set_edit_has_icon(has_icon);
                ui.set_edit_icon_preview(img.clone());

                {
                    let mut guard = profile_arc.lock().unwrap();
                    if sel_id < guard.keys.len() {
                        guard.keys[sel_id].action_type = "app".to_string();
                        guard.keys[sel_id].action_value = exec_cmd.to_string();
                        guard.keys[sel_id].title = name.to_string();
                        guard.keys[sel_id].icon_path = final_icon_str.clone();
                        guard.save();
                    }
                }

                if let Some(mut item) = keys_model.row_data(sel_id) {
                    item.action_type = SharedString::from("app");
                    item.action_value = exec_cmd.clone();
                    item.title = name.clone();
                    item.icon_path = SharedString::from(&final_icon_str);
                    item.has_icon = has_icon;
                    item.icon = img;
                    keys_model.set_row_data(sel_id, item);
                }

                let _ = sync_tx.send(SyncMsg::Key(sel_id));

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
        let profile_arc = profile.clone();
        let sync_tx = sync_tx.clone();
        main_window.on_browse_app_binary(move || {
            let ui_weak = ui_handle.clone();
            let profile_arc = profile_arc.clone();
            let sync_tx = sync_tx.clone();
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
                    let default_icon = "assets/ulanzi_official/actions/btn_open.png".to_string();

                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak.upgrade() {
                            let sel_id = ui.get_selected_key_index() as usize;
                            let (has_icon, img) = load_slint_image(&default_icon);

                            ui.set_edit_action_type(SharedString::from("app"));
                            ui.set_edit_action_value(SharedString::from(&path_str));
                            ui.set_edit_title(SharedString::from(&file_name));
                            ui.set_edit_icon_path(SharedString::from(&default_icon));
                            ui.set_edit_has_icon(has_icon);
                            ui.set_edit_icon_preview(img.clone());

                            {
                                let mut guard = profile_arc.lock().unwrap();
                                if sel_id < guard.keys.len() {
                                    guard.keys[sel_id].action_type = "app".to_string();
                                    guard.keys[sel_id].action_value = path_str.clone();
                                    guard.keys[sel_id].title = file_name.clone();
                                    guard.keys[sel_id].icon_path = default_icon.clone();
                                    guard.save();
                                }
                            }

                            let keys = ui.get_keys();
                            if let Some(mut item) = keys.row_data(sel_id) {
                                item.action_type = SharedString::from("app");
                                item.action_value = SharedString::from(&path_str);
                                item.title = SharedString::from(&file_name);
                                item.icon_path = SharedString::from(&default_icon);
                                item.has_icon = has_icon;
                                item.icon = img;
                                keys.set_row_data(sel_id, item);
                            }

                            let _ = sync_tx.send(SyncMsg::Key(sel_id));

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
        let profile_arc = profile.clone();
        let sync_tx = sync_tx.clone();
        main_window.on_browse_command_script(move || {
            let ui_weak = ui_handle.clone();
            let profile_arc = profile_arc.clone();
            let sync_tx = sync_tx.clone();
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
                    let default_icon = "assets/ulanzi_official/actions/btn_open.png".to_string();

                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak.upgrade() {
                            let sel_id = ui.get_selected_key_index() as usize;
                            let (has_icon, img) = load_slint_image(&default_icon);

                            ui.set_edit_action_type(SharedString::from("command"));
                            ui.set_edit_action_value(SharedString::from(&path_str));
                            ui.set_edit_title(SharedString::from(&file_name));
                            ui.set_edit_icon_path(SharedString::from(&default_icon));
                            ui.set_edit_has_icon(has_icon);
                            ui.set_edit_icon_preview(img.clone());

                            {
                                let mut guard = profile_arc.lock().unwrap();
                                if sel_id < guard.keys.len() {
                                    guard.keys[sel_id].action_type = "command".to_string();
                                    guard.keys[sel_id].action_value = path_str.clone();
                                    guard.keys[sel_id].title = file_name.clone();
                                    guard.keys[sel_id].icon_path = default_icon.clone();
                                    guard.save();
                                }
                            }

                            let keys = ui.get_keys();
                            if let Some(mut item) = keys.row_data(sel_id) {
                                item.action_type = SharedString::from("command");
                                item.action_value = SharedString::from(&path_str);
                                item.title = SharedString::from(&file_name);
                                item.icon_path = SharedString::from(&default_icon);
                                item.has_icon = has_icon;
                                item.icon = img;
                                keys.set_row_data(sel_id, item);
                            }

                            let _ = sync_tx.send(SyncMsg::Key(sel_id));

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

    // 11b. Browse Soundboard Audio File
    {
        let ui_handle = ui_weak.clone();
        let profile_arc = profile.clone();
        let sync_tx = sync_tx.clone();

        main_window.on_browse_audio_file(move || {
            let ui_weak = ui_handle.clone();
            let profile_arc = profile_arc.clone();
            let sync_tx = sync_tx.clone();

            std::thread::spawn(move || {
                let initial = dirs::audio_dir().or_else(dirs::home_dir).unwrap_or_else(|| PathBuf::from("."));
                let dialog = rfd::FileDialog::new()
                    .set_title("Selecionar Arquivo de Áudio para Soundboard")
                    .set_directory(initial)
                    .add_filter("Áudio", &["wav", "mp3", "ogg", "flac", "m4a", "aac", "opus"]);

                if let Some(path) = dialog.pick_file() {
                    let path_str = path.to_string_lossy().to_string();
                    let file_stem = path
                        .file_stem()
                        .map(|f| f.to_string_lossy().to_string())
                        .unwrap_or_else(|| "Áudio".to_string());
                    let default_icon = "assets/ulanzi_official/actions/btn_playAudio.png".to_string();

                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak.upgrade() {
                            let sel_id = ui.get_selected_key_index() as usize;
                            let (has_icon, img) = load_slint_image(&default_icon);

                            {
                                let mut guard = profile_arc.lock().unwrap();
                                if sel_id < guard.keys.len() {
                                    guard.keys[sel_id].action_type = "sound".to_string();
                                    guard.keys[sel_id].action_value = path_str.clone();
                                    guard.keys[sel_id].title = file_stem.clone();
                                    guard.keys[sel_id].icon_path = default_icon.clone();
                                    guard.save();
                                }
                            }

                            let keys = ui.get_keys();
                            if let Some(mut item) = keys.row_data(sel_id) {
                                item.action_type = SharedString::from("sound");
                                item.action_value = SharedString::from(&path_str);
                                item.title = SharedString::from(&file_stem);
                                item.icon_path = SharedString::from(&default_icon);
                                item.has_icon = has_icon;
                                item.icon = img.clone();
                                keys.set_row_data(sel_id, item);
                            }

                            ui.set_edit_action_type(SharedString::from("sound"));
                            ui.set_edit_action_value(SharedString::from(&path_str));
                            ui.set_edit_title(SharedString::from(&file_stem));
                            ui.set_edit_icon_path(SharedString::from(&default_icon));
                            ui.set_edit_has_icon(has_icon);
                            ui.set_edit_icon_preview(img);
                            ui.set_status_message(SharedString::from(format!(
                                "Áudio adicionado à tecla [{}]: {}",
                                sel_id, file_stem
                            )));

                            let _ = sync_tx.send(SyncMsg::Key(sel_id));
                        }
                    });
                }
            });
        });
    }

    // 11c. Icon Library Modal Callbacks
    {
        let ui_handle = ui_weak.clone();
        let icon_lib_state = icon_lib_state.clone();
        main_window.on_open_icon_library(move || {
            if let Some(ui) = ui_handle.upgrade() {
                ui.set_icon_filter_text(SharedString::from(""));
                ui.set_is_searching_icons(false);
                let mut state_guard = icon_lib_state.lock().unwrap();
                refresh_ui_categories(&ui, &mut state_guard);
                ui.set_show_icon_library(true);
            }
        });
    }

    {
        let ui_handle = ui_weak.clone();
        main_window.on_close_icon_library(move || {
            if let Some(ui) = ui_handle.upgrade() {
                ui.set_show_icon_library(false);
            }
        });
    }

    {
        let ui_handle = ui_weak.clone();
        let icon_lib_state = icon_lib_state.clone();
        main_window.on_toggle_category_expanded(move |cat_id| {
            if let Some(ui) = ui_handle.upgrade() {
                let mut state_guard = icon_lib_state.lock().unwrap();
                if state_guard.active_category_id == cat_id {
                    if let Some(cat) = state_guard.categories.iter_mut().find(|c| c.id == cat_id) {
                        cat.is_expanded = !cat.is_expanded;
                    }
                } else {
                    state_guard.active_category_id = cat_id;
                    for cat in &mut state_guard.categories {
                        cat.is_expanded = cat.id == cat_id;
                    }
                }
                ui.set_active_category_id(state_guard.active_category_id);
                refresh_ui_categories(&ui, &mut state_guard);
            }
        });
    }

    {
        let ui_handle = ui_weak.clone();
        let icon_lib_state = icon_lib_state.clone();
        main_window.on_icon_filter_changed(move |query| {
            if let Some(ui) = ui_handle.upgrade() {
                let q_str = query.trim().to_lowercase();
                if q_str.is_empty() {
                    ui.set_is_searching_icons(false);
                } else {
                    ui.set_is_searching_icons(true);
                    let all_icons = icon_lib_state.lock().unwrap().get_all_icons();
                    let filtered: Vec<OfficialIcon> = all_icons
                        .into_iter()
                        .filter(|ic| {
                            ic.name.to_lowercase().contains(&q_str)
                                || ic.category.to_lowercase().contains(&q_str)
                        })
                        .collect();

                    ui.set_search_result_count(filtered.len() as i32);
                    let rows = build_grid_rows(&filtered);
                    ui.set_search_icon_rows(slint::ModelRc::new(slint::VecModel::from(rows)));
                }
            }
        });
    }

    {
        let ui_handle = ui_weak.clone();
        main_window.on_open_marketplace(move || {
            let _ = open::that("http://ugc.ulanzistudio.com");
            if let Some(ui) = ui_handle.upgrade() {
                ui.set_status_message(SharedString::from(
                    "Abrindo Marketplace oficial da Ulanzi no navegador..."
                ));
            }
        });
    }

    {
        let ui_handle = ui_weak.clone();
        let icon_lib_state = icon_lib_state.clone();
        main_window.on_import_icon_pack(move || {
            let ui_weak = ui_handle.clone();
            let icon_lib_state = icon_lib_state.clone();

            std::thread::spawn(move || {
                let dialog = rfd::FileDialog::new()
                    .set_title("Selecionar Pacote de Ícones (.zip)")
                    .add_filter("Icon Pack (*.zip)", &["zip"]);

                if let Some(path) = dialog.pick_file() {
                    match extract_zip_icon_pack(&path) {
                        Ok((pack_name, count)) => {
                            let mut state_guard = icon_lib_state.lock().unwrap();
                            state_guard.rescan();

                            if let Some(cat) = state_guard.categories.iter_mut().find(|c| c.name == pack_name) {
                                let new_id = cat.id;
                                for c in &mut state_guard.categories {
                                    c.is_expanded = c.id == new_id;
                                }
                                state_guard.active_category_id = new_id;
                            }

                            let msg = format!(
                                "Pacote de ícones '{}' importado com sucesso! ({} ícones)",
                                pack_name, count
                            );

                            let state_clone = icon_lib_state.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = ui_weak.upgrade() {
                                    let mut sg = state_clone.lock().unwrap();
                                    ui.set_active_category_id(sg.active_category_id);
                                    ui.set_is_searching_icons(false);
                                    ui.set_icon_filter_text(SharedString::from(""));
                                    refresh_ui_categories(&ui, &mut sg);
                                    ui.set_status_message(SharedString::from(msg));
                                }
                            });
                        }
                        Err(err) => {
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = ui_weak.upgrade() {
                                    ui.set_status_message(SharedString::from(format!(
                                        "Erro ao importar pacote: {}",
                                        err
                                    )));
                                }
                            });
                        }
                    }
                }
            });
        });
    }

    {
        let ui_handle = ui_weak.clone();
        let keys_model = keys_model.clone();
        let profile_arc = profile.clone();
        let sync_tx = sync_tx.clone();

        main_window.on_icon_chosen(move |icon_path| {
            if let Some(ui) = ui_handle.upgrade() {
                ui.set_show_icon_library(false);
                let icon_str = icon_path.as_str();
                ui.set_edit_icon_path(icon_path.clone());
                let (has_icon, img) = load_slint_image(icon_str);
                ui.set_edit_has_icon(has_icon);
                ui.set_edit_icon_preview(img.clone());

                let sel_id = ui.get_selected_key_index() as usize;
                if let Some(mut item) = keys_model.row_data(sel_id) {
                    item.icon_path = icon_path.clone();
                    item.has_icon = has_icon;
                    item.icon = img;
                    keys_model.set_row_data(sel_id, item);
                }

                {
                    let mut guard = profile_arc.lock().unwrap();
                    if sel_id < guard.keys.len() {
                        guard.keys[sel_id].icon_path = icon_str.to_string();
                        guard.save();
                    }
                }

                let _ = sync_tx.send(SyncMsg::Key(sel_id));

                ui.set_status_message(SharedString::from(format!(
                    "Ícone oficial aplicado à tecla [{}]",
                    sel_id
                )));
            }
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
        let profile_arc = profile.clone();
        let sync_tx = sync_tx.clone();
        main_window.on_refresh_connection(move || {
            let (new_tx, _new_rx) = mpsc::channel();
            if let Some(ui) = ui_handle.upgrade() {
                match deck_clone.connect(new_tx) {
                    Ok(path) => {
                        ui.set_device_connected(true);
                        ui.set_udev_missing(false);
                        ui.set_device_node(SharedString::from(path.to_string_lossy().to_string()));
                        ui.set_status_message(SharedString::from(format!(
                            "Reconectado a {}",
                            path.display()
                        )));
                        let b = profile_arc.lock().unwrap().brightness;
                        let _ = deck_clone.set_brightness(b);
                        let _ = deck_clone.set_label_style(true);
                        let _ = deck_clone.query_device_info();
                        let _ = sync_tx.send(SyncMsg::Full);
                    }
                    Err(e) => {
                        ui.set_device_connected(false);
                        if e.contains("permissão") || !udev::is_udev_rule_installed() {
                            ui.set_udev_missing(true);
                        }
                        ui.set_status_message(SharedString::from(e));
                    }
                }
            }
        });
    }

    // 15. Auto-Install Udev Rules via Polkit Desktop GUI
    {
        let deck_clone = deck.clone();
        let ui_handle = ui_weak.clone();
        let profile_arc = profile.clone();
        let sync_tx = sync_tx.clone();
        main_window.on_request_install_udev(move || {
            let ui_weak = ui_handle.clone();
            let deck_clone = deck_clone.clone();
            let profile_arc = profile_arc.clone();
            let sync_tx = sync_tx.clone();

            std::thread::spawn(move || {
                let _ = slint::invoke_from_event_loop({
                    let ui_weak = ui_weak.clone();
                    move || {
                        if let Some(ui) = ui_weak.upgrade() {
                            ui.set_status_message(SharedString::from(
                                "Autenticação solicitada: confirme sua senha na janela do sistema...",
                            ));
                        }
                    }
                });

                match udev::install_udev_rules_pkexec() {
                    Ok(_) => {
                        std::thread::sleep(Duration::from_millis(600));
                        let (new_tx, _new_rx) = mpsc::channel();
                        let conn_res = deck_clone.connect(new_tx);

                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak.upgrade() {
                                ui.set_udev_missing(false);
                                match conn_res {
                                    Ok(path) => {
                                        ui.set_device_connected(true);
                                        ui.set_device_node(SharedString::from(path.to_string_lossy().to_string()));
                                        ui.set_status_message(SharedString::from(
                                            "Regras Udev instaladas! Dispositivo conectado com sucesso."
                                        ));
                                        let b = profile_arc.lock().unwrap().brightness;
                                        let _ = deck_clone.set_brightness(b);
                                        let _ = deck_clone.set_label_style(true);
                                        let _ = sync_tx.send(SyncMsg::Full);
                                    }
                                    Err(e) => {
                                        ui.set_status_message(SharedString::from(format!(
                                            "Regras Udev configuradas! {}", e
                                        )));
                                    }
                                }
                            }
                        });
                    }
                    Err(e) => {
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak.upgrade() {
                                ui.set_status_message(SharedString::from(format!(
                                    "Configuração Udev cancelada: {}", e
                                )));
                            }
                        });
                    }
                }
            });
        });
    }

    // 16. Settings Dialog Callbacks
    {
        let ui_handle = ui_weak.clone();
        main_window.on_open_settings_dialog(move || {
            if let Some(ui) = ui_handle.upgrade() {
                ui.set_show_settings_dialog(true);
            }
        });
    }
    {
        let ui_handle = ui_weak.clone();
        main_window.on_close_settings_dialog(move || {
            if let Some(ui) = ui_handle.upgrade() {
                ui.set_show_settings_dialog(false);
            }
        });
    }
    {
        let ui_handle = ui_weak.clone();
        main_window.on_switch_settings_tab(move |idx| {
            if let Some(ui) = ui_handle.upgrade() {
                ui.set_settings_tab_index(idx);
            }
        });
    }
    {
        let ui_handle = ui_weak.clone();
        main_window.on_check_firmware_updates(move || {
            if let Some(ui) = ui_handle.upgrade() {
                ui.set_is_checking_firmware(true);
                ui.set_firmware_check_message(SharedString::from("Consultando servidores oficiais da Ulanzi..."));
                let sn = ui.get_device_serial_number().to_string();
                let ver_raw = ui.get_device_firmware_version().to_string();
                let ver = ver_raw.trim_start_matches('v').trim_start_matches('V').to_string();

                let ui_async = ui_handle.clone();
                std::thread::spawn(move || {
                    let res = firmware::check_for_firmware_update(&sn, "d200", &ver);
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_async.upgrade() {
                            ui.set_is_checking_firmware(false);
                            match res {
                                Ok(info) => {
                                    ui.set_firmware_has_update(info.need_update);
                                    ui.set_firmware_latest_version(SharedString::from(info.latest_version));
                                    ui.set_firmware_changelog(SharedString::from(info.changelog));
                                    ui.set_firmware_download_url(SharedString::from(info.download_url));
                                    ui.set_firmware_check_message(SharedString::from(info.message));
                                }
                                Err(err) => {
                                    ui.set_firmware_has_update(false);
                                    ui.set_firmware_check_message(SharedString::from(format!("Erro ao verificar: {}", err)));
                                }
                            }
                        }
                    });
                });
            }
        });
    }
    {
        let ui_handle = ui_weak.clone();
        main_window.on_toggle_autostart(move |enable| {
            let res = autostart::set_autostart_enabled(enable);
            if let Some(ui) = ui_handle.upgrade() {
                if res.is_ok() {
                    ui.set_autostart_enabled(enable);
                } else if let Err(e) = res {
                    ui.set_status_message(SharedString::from(format!("Erro autostart: {}", e)));
                }
            }
        });
    }
    {
        main_window.on_open_profiles_folder(move || {
            let path = config::ProfileConfig::config_path();
            if let Some(parent) = path.parent() {
                let _ = open::that(parent);
            }
        });
    }
    {
        main_window.on_open_icons_folder(move || {
            if let Some(mut d) = dirs::config_dir() {
                d.push("uzlinux-studio");
                d.push("icons");
                let _ = std::fs::create_dir_all(&d);
                let _ = open::that(&d);
            }
        });
    }
    {
        let ui_handle = ui_weak.clone();
        main_window.on_open_firmware_download_link(move || {
            if let Some(ui) = ui_handle.upgrade() {
                let url = ui.get_firmware_download_url().to_string();
                if !url.trim().is_empty() {
                    let _ = open::that(&url);
                } else {
                    let _ = open::that("https://www.ulanzi.com");
                }
            }
        });
    }
    {
        main_window.on_open_github_repo(move || {
            let _ = open::that("https://github.com/lucastiago/UzlinuxStudio");
        });
    }

    // Profile & Multi-Page Callbacks
    {
        let ui_handle = ui_weak.clone();
        let profile_arc = profile.clone();
        let sync_tx = sync_tx.clone();
        let deck_clone = deck.clone();

        main_window.on_switch_profile(move |prof_name| {
            let mut guard = profile_arc.lock().unwrap();
            let switched = guard.switch_profile(prof_name.as_str());
            if switched {
                guard.save();
                if let Some(ui) = ui_handle.upgrade() {
                    refresh_ui_profiles_and_pages(&ui, &guard);
                    ui.set_status_message(SharedString::from(format!(
                        "Perfil alterado para: {}",
                        prof_name
                    )));
                }
                if let Some(k13) = guard.keys.get(13) {
                    if k13.action_type == "clock" {
                        let _ = deck_clone.set_small_window_data(1, 0, 0, &chrono::Local::now().format("%H:%M:%S").to_string(), 0);
                    } else if k13.action_type == "system_stats" {
                        let (cpu, mem) = device::get_system_stats();
                        let _ = deck_clone.set_small_window_data(0, cpu, mem, &chrono::Local::now().format("%H:%M:%S").to_string(), 0);
                    }
                }
                let _ = sync_tx.send(SyncMsg::Full);
            }
        });
    }

    {
        let ui_handle = ui_weak.clone();
        let profile_arc = profile.clone();
        let sync_tx = sync_tx.clone();
        let deck_clone = deck.clone();

        main_window.on_create_profile(move |name_str| {
            let mut guard = profile_arc.lock().unwrap();
            let new_name = guard.add_profile(name_str.as_str());
            guard.save();
            if let Some(ui) = ui_handle.upgrade() {
                refresh_ui_profiles_and_pages(&ui, &guard);
                ui.set_status_message(SharedString::from(format!(
                    "Novo perfil criado: {}",
                    new_name
                )));
            }
            if let Some(k13) = guard.keys.get(13) {
                if k13.action_type == "clock" {
                    let _ = deck_clone.set_small_window_data(1, 0, 0, &chrono::Local::now().format("%H:%M:%S").to_string(), 0);
                } else if k13.action_type == "system_stats" {
                    let (cpu, mem) = device::get_system_stats();
                    let _ = deck_clone.set_small_window_data(0, cpu, mem, &chrono::Local::now().format("%H:%M:%S").to_string(), 0);
                }
            }
            let _ = sync_tx.send(SyncMsg::Full);
        });
    }

    {
        let ui_handle = ui_weak.clone();
        let profile_arc = profile.clone();
        let sync_tx = sync_tx.clone();
        let deck_clone = deck.clone();

        main_window.on_duplicate_profile(move |source_name| {
            let mut guard = profile_arc.lock().unwrap();
            if let Some(new_name) = guard.duplicate_profile(source_name.as_str()) {
                guard.save();
                if let Some(ui) = ui_handle.upgrade() {
                    refresh_ui_profiles_and_pages(&ui, &guard);
                    ui.set_status_message(SharedString::from(format!(
                        "Perfil duplicado: {}",
                        new_name
                    )));
                }
                if let Some(k13) = guard.keys.get(13) {
                    if k13.action_type == "clock" {
                        let _ = deck_clone.set_small_window_data(1, 0, 0, &chrono::Local::now().format("%H:%M:%S").to_string(), 0);
                    } else if k13.action_type == "system_stats" {
                        let (cpu, mem) = device::get_system_stats();
                        let _ = deck_clone.set_small_window_data(0, cpu, mem, &chrono::Local::now().format("%H:%M:%S").to_string(), 0);
                    }
                }
                let _ = sync_tx.send(SyncMsg::Full);
            }
        });
    }

    {
        let ui_handle = ui_weak.clone();
        let profile_arc = profile.clone();
        let sync_tx = sync_tx.clone();
        let deck_clone = deck.clone();

        main_window.on_delete_profile(move |prof_name| {
            let mut guard = profile_arc.lock().unwrap();
            let deleted = guard.delete_profile(prof_name.as_str());
            if deleted {
                guard.save();
                if let Some(ui) = ui_handle.upgrade() {
                    refresh_ui_profiles_and_pages(&ui, &guard);
                    ui.set_status_message(SharedString::from(format!(
                        "Perfil excluído. Perfil ativo: {}",
                        guard.current_profile
                    )));
                }
                if let Some(k13) = guard.keys.get(13) {
                    if k13.action_type == "clock" {
                        let _ = deck_clone.set_small_window_data(1, 0, 0, &chrono::Local::now().format("%H:%M:%S").to_string(), 0);
                    } else if k13.action_type == "system_stats" {
                        let (cpu, mem) = device::get_system_stats();
                        let _ = deck_clone.set_small_window_data(0, cpu, mem, &chrono::Local::now().format("%H:%M:%S").to_string(), 0);
                    }
                }
                let _ = sync_tx.send(SyncMsg::Full);
            }
        });
    }

    {
        let ui_handle = ui_weak.clone();
        let profile_arc = profile.clone();
        let sync_tx = sync_tx.clone();
        let deck_clone = deck.clone();

        main_window.on_switch_page(move |page_idx| {
            let mut guard = profile_arc.lock().unwrap();
            let switched = guard.switch_page(page_idx as usize);
            if switched {
                guard.save();
                if let Some(ui) = ui_handle.upgrade() {
                    refresh_ui_profiles_and_pages(&ui, &guard);
                    ui.set_status_message(SharedString::from(format!(
                        "Navegou para Página {}",
                        page_idx + 1
                    )));
                }
                if let Some(k13) = guard.keys.get(13) {
                    if k13.action_type == "clock" {
                        let _ = deck_clone.set_small_window_data(1, 0, 0, &chrono::Local::now().format("%H:%M:%S").to_string(), 0);
                    } else if k13.action_type == "system_stats" {
                        let (cpu, mem) = device::get_system_stats();
                        let _ = deck_clone.set_small_window_data(0, cpu, mem, &chrono::Local::now().format("%H:%M:%S").to_string(), 0);
                    }
                }
                let _ = sync_tx.send(SyncMsg::Full);
            }
        });
    }

    {
        let ui_handle = ui_weak.clone();
        let profile_arc = profile.clone();
        let sync_tx = sync_tx.clone();
        let deck_clone = deck.clone();

        main_window.on_add_page(move || {
            let mut guard = profile_arc.lock().unwrap();
            let new_idx = guard.add_page();
            guard.save();
            if let Some(ui) = ui_handle.upgrade() {
                refresh_ui_profiles_and_pages(&ui, &guard);
                ui.set_status_message(SharedString::from(format!(
                    "Nova Página {} criada!",
                    new_idx + 1
                )));
            }
            if let Some(k13) = guard.keys.get(13) {
                if k13.action_type == "clock" {
                    let _ = deck_clone.set_small_window_data(1, 0, 0, &chrono::Local::now().format("%H:%M:%S").to_string(), 0);
                } else if k13.action_type == "system_stats" {
                    let (cpu, mem) = device::get_system_stats();
                    let _ = deck_clone.set_small_window_data(0, cpu, mem, &chrono::Local::now().format("%H:%M:%S").to_string(), 0);
                }
            }
            let _ = sync_tx.send(SyncMsg::Full);
        });
    }

    {
        let ui_handle = ui_weak.clone();
        let profile_arc = profile.clone();
        let sync_tx = sync_tx.clone();
        let deck_clone = deck.clone();

        main_window.on_delete_page(move |page_idx| {
            let mut guard = profile_arc.lock().unwrap();
            let deleted = guard.delete_page(page_idx as usize);
            if deleted {
                guard.save();
                if let Some(ui) = ui_handle.upgrade() {
                    refresh_ui_profiles_and_pages(&ui, &guard);
                    ui.set_status_message(SharedString::from(format!(
                        "Página {} removida",
                        page_idx + 1
                    )));
                }
                if let Some(k13) = guard.keys.get(13) {
                    if k13.action_type == "clock" {
                        let _ = deck_clone.set_small_window_data(1, 0, 0, &chrono::Local::now().format("%H:%M:%S").to_string(), 0);
                    } else if k13.action_type == "system_stats" {
                        let (cpu, mem) = device::get_system_stats();
                        let _ = deck_clone.set_small_window_data(0, cpu, mem, &chrono::Local::now().format("%H:%M:%S").to_string(), 0);
                    }
                }
                let _ = sync_tx.send(SyncMsg::Full);
            }
        });
    }

    // 15. Background Thread: Listen for hardware events from channel and dispatch to Slint UI & Actions
    {
        let ui_handle = ui_weak.clone();
        let profile_arc = profile.clone();
        let sync_tx = sync_tx.clone();
        let deck_clone = deck.clone();

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
                                let mut handled_page_act = false;
                                {
                                    let mut guard = profile_arc.lock().unwrap();
                                    if slot_id < guard.keys.len() {
                                        let key_cfg = &guard.keys[slot_id];
                                        match key_cfg.action_type.as_str() {
                                            "page.next" => {
                                                guard.next_page();
                                                guard.save();
                                                handled_page_act = true;
                                            }
                                            "page.prev" => {
                                                guard.prev_page();
                                                guard.save();
                                                handled_page_act = true;
                                            }
                                            "page.goto" => {
                                                let target: usize = key_cfg.action_value.parse().unwrap_or(1);
                                                let target_idx = target.saturating_sub(1);
                                                guard.switch_page(target_idx);
                                                guard.save();
                                                handled_page_act = true;
                                            }
                                            "page.switch" => {
                                                if guard.profiles.len() > 1 {
                                                    let cur_idx = guard.profiles.iter().position(|p| p.name == guard.current_profile).unwrap_or(0);
                                                    let next_idx = (cur_idx + 1) % guard.profiles.len();
                                                    let next_name = guard.profiles[next_idx].name.clone();
                                                    guard.switch_profile(&next_name);
                                                    guard.save();
                                                    handled_page_act = true;
                                                }
                                            }
                                            _ => {
                                                actions::execute_action(
                                                    &key_cfg.action_type,
                                                    &key_cfg.action_value,
                                                );
                                            }
                                        }

                                        if handled_page_act {
                                            if let Some(k13) = guard.keys.get(13) {
                                                if k13.action_type == "clock" {
                                                    let _ = deck_clone.set_small_window_data(1, 0, 0, &chrono::Local::now().format("%H:%M:%S").to_string(), 0);
                                                } else if k13.action_type == "system_stats" {
                                                    let (cpu, mem) = device::get_system_stats();
                                                    let _ = deck_clone.set_small_window_data(0, cpu, mem, &chrono::Local::now().format("%H:%M:%S").to_string(), 0);
                                                }
                                            }
                                        }
                                    }
                                }

                                if handled_page_act {
                                    let ui_weak_inner = ui_handle.clone();
                                    let prof_clone = profile_arc.clone();
                                    let _ = slint::invoke_from_event_loop(move || {
                                        if let Some(ui) = ui_weak_inner.upgrade() {
                                            let guard = prof_clone.lock().unwrap();
                                            refresh_ui_profiles_and_pages(&ui, &guard);
                                            ui.set_status_message(SharedString::from(format!(
                                                "Página/Perfil alternado: {} (Página {})",
                                                guard.current_profile,
                                                guard.current_page_index() + 1
                                            )));
                                        }
                                    });
                                    let _ = sync_tx.send(SyncMsg::Full);
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
                        DeckEvent::DeviceInfo(info) => {
                            println!("[Deck] Dispositivo identificou-se: {}", info);
                            let ui_weak_inner = ui_handle.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = ui_weak_inner.upgrade() {
                                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&info) {
                                        if let Some(ver) = v.get("version").and_then(|x| x.as_str()) {
                                            let ver_fmt = if ver.starts_with('v') || ver.starts_with('V') {
                                                ver.to_string()
                                            } else {
                                                format!("v{}", ver)
                                            };
                                            ui.set_device_firmware_version(SharedString::from(ver_fmt));
                                        }
                                        if let Some(sn) = v.get("deviceSN").and_then(|x| x.as_str()) {
                                            ui.set_device_serial_number(SharedString::from(sn));
                                        }
                                        if let Some(model) = v.get("deviceType").and_then(|x| x.as_str()) {
                                            ui.set_device_model(SharedString::from(model));
                                        }
                                    } else if info.contains('.') {
                                        let ver_fmt = if info.starts_with('v') || info.starts_with('V') {
                                            info.trim().to_string()
                                        } else {
                                            format!("v{}", info.trim())
                                        };
                                        ui.set_device_firmware_version(SharedString::from(ver_fmt));
                                    }
                                }
                            });
                        }
                        DeckEvent::Unknown { cmd, len } => {
                            println!(
                                "[Deck] Pacote recebido: cmd=0x{:04x}, len={}",
                                cmd, len
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

#[cfg(test)]
mod icon_library_tests {
    use super::*;

    #[test]
    fn test_icon_library_state_scan() {
        let state = IconLibraryState::scan();
        assert!(!state.categories.is_empty(), "Categorias de ícones devem ser encontradas");
        // Anaglyph and code should be present
        let names: Vec<String> = state.categories.iter().map(|c| c.name.clone()).collect();
        println!("Categorias encontradas: {:?}", names);
        assert!(names.contains(&"Anaglyph".to_string()) || names.contains(&"code".to_string()) || names.contains(&"Ulanzi Ações".to_string()));
    }

    #[test]
    fn test_build_grid_rows_8_columns() {
        let dummy_icons = (0..19)
            .map(|i| OfficialIcon {
                name: format!("Icon {}", i),
                category: "Test".to_string(),
                path: "assets/ulanzi_official/actions/icon.svg".to_string(),
            })
            .collect::<Vec<_>>();

        let rows = build_grid_rows(&dummy_icons);
        // 19 icons chunked into groups of 8: row 0 (8 items), row 1 (8 items), row 2 (3 items) = 3 rows
        assert_eq!(rows.len(), 3);
        assert!(rows[0].item_0_valid);
        assert!(rows[0].item_7_valid);
        assert!(rows[1].item_0_valid);
        assert!(rows[1].item_7_valid);
        assert!(rows[2].item_0_valid);
        assert!(rows[2].item_2_valid);
        assert!(!rows[2].item_3_valid);
        assert!(!rows[2].item_7_valid);
    }
}

#[cfg(test)]
mod app_library_tests {
    use super::*;

    #[test]
    fn test_app_library_state_build() {
        let dummy_apps = vec![
            apps::AppInfo {
                name: "VS Code".to_string(),
                exec_cmd: "code".to_string(),
                icon_path: "".to_string(),
                category: "Desenvolvimento".to_string(),
            },
            apps::AppInfo {
                name: "Steam".to_string(),
                exec_cmd: "steam".to_string(),
                icon_path: "".to_string(),
                category: "Jogos".to_string(),
            },
            apps::AppInfo {
                name: "GIMP".to_string(),
                exec_cmd: "gimp".to_string(),
                icon_path: "".to_string(),
                category: "Gráficos & Design".to_string(),
            },
            apps::AppInfo {
                name: "Rust Rover".to_string(),
                exec_cmd: "rustrover".to_string(),
                icon_path: "".to_string(),
                category: "Desenvolvimento".to_string(),
            },
        ];

        let state = AppLibraryState::build(&dummy_apps);
        assert!(!state.categories.is_empty());
        let dev_cat = state.categories.iter().find(|c| c.name == "Desenvolvimento").unwrap();
        assert_eq!(dev_cat.apps.len(), 2);
        assert_eq!(dev_cat.apps[0].name, "Rust Rover");
        assert_eq!(dev_cat.apps[1].name, "VS Code");

        let games_cat = state.categories.iter().find(|c| c.name == "Jogos").unwrap();
        assert_eq!(games_cat.apps.len(), 1);
        assert_eq!(games_cat.apps[0].name, "Steam");
    }
}


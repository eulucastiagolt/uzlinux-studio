use std::path::{Path, PathBuf};
use std::fs;
use std::io::Write;
use anyhow::{Context, Result, anyhow};
use log::info;
use serde::{Serialize, Deserialize};
use serde_json::Value;

use crate::models::{
    AppSettings, ApplicationProfiles, Profile, ProfileSelection
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenDeckProfileInfo {
    pub device_id: String,
    pub name: String,
    pub file_name: String,
    pub rel_path: String,
    pub keys_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenDeckPluginInfo {
    pub folder_name: String,
    pub uuid: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub is_native: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenDeckScanResult {
    pub found: bool,
    pub path: String,
    pub profiles: Vec<OpenDeckProfileInfo>,
    pub plugins: Vec<OpenDeckPluginInfo>,
    pub has_applications: bool,
    pub has_settings: bool,
    pub has_images: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenDeckImportOptions {
    pub import_all: bool,
    pub import_profiles: bool,
    pub selected_profiles: Vec<String>,
    pub import_plugins: bool,
    pub selected_plugins: Vec<String>,
    pub import_applications: bool,
    pub import_settings: bool,
    pub import_images: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenDeckImportReport {
    pub imported_profiles: usize,
    pub imported_plugins: usize,
    pub imported_applications: bool,
    pub imported_settings: bool,
    pub imported_images: bool,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct ConfigManager {
    base_dir: PathBuf,
}

fn copy_dir_all<P: AsRef<Path>, Q: AsRef<Path>>(src: P, dst: Q) -> Result<()> {
    let src = src.as_ref();
    let dst = dst.as_ref();
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&from, &to)?;
        } else {
            let _ = fs::copy(&from, &to);
        }
    }
    Ok(())
}

impl ConfigManager {
    /// Creates a ConfigManager with an explicitly provided path.
    pub fn new<P: AsRef<Path>>(base_dir: P) -> Self {
        Self {
            base_dir: base_dir.as_ref().to_path_buf(),
        }
    }

    /// Automatically discovers the config directory.
    /// Uzlinux Studio uses its own isolated configuration directory (~/.config/uzlinux-studio),
    /// allowing full customization and deletion of plugins without affecting OpenDeck.
    pub fn discover() -> Self {
        if let Ok(custom) = std::env::var("UZLINUX_CONFIG_DIR") {
            let p = PathBuf::from(custom);
            info!("Using custom UZLINUX_CONFIG_DIR: {:?}", p);
            return Self::new(p);
        }

        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let uzlinux_studio_dir = dirs::config_dir()
            .map(|d| d.join("uzlinux-studio"))
            .unwrap_or_else(|| home.join(".config/uzlinux-studio"));

        // If legacy ~/.config/uzlinux-studiodeck exists, migrate or merge into ~/.config/uzlinux-studio automatically
        let legacy_deck_dir = dirs::config_dir()
            .map(|d| d.join("uzlinux-studiodeck"))
            .unwrap_or_else(|| home.join(".config/uzlinux-studiodeck"));

        if legacy_deck_dir.exists() {
            if !uzlinux_studio_dir.exists() {
                info!("Migrating existing config directory from {:?} to {:?}", legacy_deck_dir, uzlinux_studio_dir);
                let _ = std::fs::rename(&legacy_deck_dir, &uzlinux_studio_dir);
            } else {
                info!("Merging existing legacy config directory from {:?} into {:?}", legacy_deck_dir, uzlinux_studio_dir);
                let _ = copy_dir_all(&legacy_deck_dir, &uzlinux_studio_dir);
            }
        }

        let mgr = Self::new(uzlinux_studio_dir);
        let _ = mgr.ensure_directories();

        // If newly created or has no profiles, do initial one-way import from OpenDeck if present
        let profiles_empty = mgr.profiles_dir().read_dir().map(|mut r| r.next().is_none()).unwrap_or(true);
        if profiles_empty {
            if Self::find_opendeck_dir().is_some() {
                info!("Initializing Uzlinux Studio by importing existing OpenDeck data...");
                let _ = mgr.import_from_opendeck();
            }
        }

        mgr
    }

    /// Finds an existing OpenDeck installation directory on Flatpak or Native.
    pub fn find_opendeck_dir() -> Option<PathBuf> {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let flatpak_opendeck = home.join(".var/app/me.amankhanna.opendeck/config/opendeck");
        if flatpak_opendeck.exists() {
            return Some(flatpak_opendeck);
        }
        let native_opendeck = dirs::config_dir()
            .map(|d| d.join("opendeck"))
            .unwrap_or_else(|| home.join(".config/opendeck"));
        if native_opendeck.exists() {
            return Some(native_opendeck);
        }
        None
    }

    /// Scans an existing OpenDeck installation directory to list available profiles, plugins, and configs.
    pub fn scan_opendeck(&self) -> Result<OpenDeckScanResult> {
        let opendeck = Self::find_opendeck_dir()
            .ok_or_else(|| anyhow!("Diretório do OpenDeck não encontrado no sistema"))?;

        let mut profiles = Vec::new();
        let src_profiles = opendeck.join("profiles");
        if src_profiles.exists() {
            if let Ok(entries) = fs::read_dir(&src_profiles) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        let device_id = entry.file_name().to_string_lossy().to_string();
                        if let Ok(prof_entries) = fs::read_dir(&path) {
                            for prof_entry in prof_entries.flatten() {
                                let ppath = prof_entry.path();
                                if ppath.extension().map(|e| e == "json").unwrap_or(false) {
                                    let fname = prof_entry.file_name().to_string_lossy().to_string();
                                    let prof_name = ppath.file_stem().unwrap_or_default().to_string_lossy().to_string();
                                    let rel_path = format!("{}/{}", device_id, fname);
                                    let mut keys_count = 0;
                                    if let Ok(content) = fs::read_to_string(&ppath) {
                                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                                            if let Some(keys) = val.get("keys").and_then(|k| k.as_array()) {
                                                keys_count = keys.len();
                                            }
                                        }
                                    }
                                    profiles.push(OpenDeckProfileInfo {
                                        device_id: device_id.clone(),
                                        name: prof_name,
                                        file_name: fname,
                                        rel_path,
                                        keys_count,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
        profiles.sort_by(|a, b| a.name.cmp(&b.name));

        let mut plugins = Vec::new();
        let src_plugins = opendeck.join("plugins");
        if src_plugins.exists() {
            if let Ok(entries) = fs::read_dir(&src_plugins) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        let folder_name = entry.file_name().to_string_lossy().to_string();
                        let manifest_path = path.join("manifest.json");
                        let mut name = folder_name.clone();
                        let mut version = String::new();
                        let mut description = String::new();
                        let uuid = folder_name.clone();

                        if manifest_path.exists() {
                            if let Ok(content) = fs::read_to_string(&manifest_path) {
                                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                                    if let Some(n) = val.get("Name").and_then(|v| v.as_str()) {
                                        name = n.to_string();
                                    }
                                    if let Some(v) = val.get("Version").and_then(|v| v.as_str()) {
                                        version = v.to_string();
                                    }
                                    if let Some(d) = val.get("Description").and_then(|v| v.as_str()) {
                                        description = d.to_string();
                                    }
                                }
                            }
                        }

                        let is_native = folder_name == "com.amansprojects.starterpack.sdPlugin"
                            || folder_name == "com.glmagalhaes.ulanzi.d200.sdPlugin"
                            || folder_name == "me.amankhanna.oampris.sdPlugin"
                            || folder_name == "com.sfgrimes.pipewire-audio.sdPlugin";

                        plugins.push(OpenDeckPluginInfo {
                            folder_name,
                            uuid,
                            name,
                            version,
                            description,
                            is_native,
                        });
                    }
                }
            }
        }
        plugins.sort_by(|a, b| a.name.cmp(&b.name));

        let has_applications = opendeck.join("applications.json").exists();
        let has_settings = opendeck.join("settings").exists() || opendeck.join("settings.json").exists();
        let has_images = opendeck.join("images").exists();

        Ok(OpenDeckScanResult {
            found: true,
            path: opendeck.to_string_lossy().to_string(),
            profiles,
            plugins,
            has_applications,
            has_settings,
            has_images,
        })
    }

    /// Performs custom, selective or full import from OpenDeck into UzlinuxStudioDeck without modifying OpenDeck.
    pub fn import_from_opendeck_custom(&self, opts: &OpenDeckImportOptions) -> Result<OpenDeckImportReport> {
        let opendeck = Self::find_opendeck_dir()
            .ok_or_else(|| anyhow!("Diretório do OpenDeck não encontrado no sistema"))?;

        info!("Performing custom import from OpenDeck (opts: {:?})", opts);
        self.ensure_directories()?;

        let mut imported_profiles = 0;
        let mut imported_plugins = 0;
        let mut imported_applications = false;
        let mut imported_settings = false;
        let mut imported_images = false;

        let src_profiles = opendeck.join("profiles");
        let src_plugins = opendeck.join("plugins");
        let src_settings = opendeck.join("settings");
        let src_images = opendeck.join("images");
        let src_settings_json = opendeck.join("settings.json");
        let src_apps_json = opendeck.join("applications.json");

        if opts.import_all {
            // 1. Profiles
            if src_profiles.exists() {
                copy_dir_all(&src_profiles, self.profiles_dir())?;
                if let Ok(entries) = fs::read_dir(&src_profiles) {
                    for e in entries.flatten() {
                        if e.path().is_dir() {
                            if let Ok(sub) = fs::read_dir(e.path()) {
                                imported_profiles += sub.flatten().filter(|f| f.path().extension().map(|x| x == "json").unwrap_or(false)).count();
                            }
                        }
                    }
                }
            }

            // 2. Plugins
            if src_plugins.exists() {
                if let Ok(entries) = fs::read_dir(&src_plugins) {
                    for entry in entries.flatten() {
                        let file_name = entry.file_name();
                        let name_str = file_name.to_string_lossy();
                        if name_str == "com.amansprojects.starterpack.sdPlugin" {
                            continue;
                        }
                        let target = self.plugins_dir().join(&file_name);
                        let _ = copy_dir_all(&entry.path(), &target);
                        imported_plugins += 1;
                    }
                }
            }

            // 3. Settings
            if src_settings.exists() {
                let _ = copy_dir_all(&src_settings, self.settings_dir());
                imported_settings = true;
            }
            if src_settings_json.exists() && !self.settings_file().exists() {
                let _ = fs::copy(&src_settings_json, self.settings_file());
                imported_settings = true;
            }

            // 4. Images
            if src_images.exists() {
                let _ = copy_dir_all(&src_images, self.images_dir());
                imported_images = true;
            }

            // 5. Applications
            if src_apps_json.exists() {
                let _ = fs::copy(&src_apps_json, self.applications_file());
                imported_applications = true;
            }
        } else {
            // Selective import:
            // 1. Profiles
            if opts.import_profiles {
                if opts.selected_profiles.is_empty() {
                    // Import all profiles
                    if src_profiles.exists() {
                        copy_dir_all(&src_profiles, self.profiles_dir())?;
                        if let Ok(entries) = fs::read_dir(&src_profiles) {
                            for e in entries.flatten() {
                                if e.path().is_dir() {
                                    if let Ok(sub) = fs::read_dir(e.path()) {
                                        imported_profiles += sub.flatten().filter(|f| f.path().extension().map(|x| x == "json").unwrap_or(false)).count();
                                    }
                                }
                            }
                        }
                    }
                } else {
                    // Import only selected profiles
                    for rel in &opts.selected_profiles {
                        let src_file = src_profiles.join(rel);
                        let dst_file = self.profiles_dir().join(rel);
                        if src_file.exists() {
                            if let Some(parent) = dst_file.parent() {
                                fs::create_dir_all(parent)?;
                            }
                            fs::copy(&src_file, &dst_file)?;
                            imported_profiles += 1;

                            // Also if device root config e.g. <device_id>.json exists, copy it if missing
                            if let Some(device_dir_name) = rel.split('/').next() {
                                let dev_json = format!("{}.json", device_dir_name);
                                let dev_src = src_profiles.join(&dev_json);
                                let dev_dst = self.profiles_dir().join(&dev_json);
                                if dev_src.exists() && !dev_dst.exists() {
                                    let _ = fs::copy(&dev_src, &dev_dst);
                                }
                            }
                        }
                    }
                }
            }

            // 2. Plugins
            if opts.import_plugins {
                if opts.selected_plugins.is_empty() {
                    // Import all plugins
                    if src_plugins.exists() {
                        if let Ok(entries) = fs::read_dir(&src_plugins) {
                            for entry in entries.flatten() {
                                let file_name = entry.file_name();
                                let name_str = file_name.to_string_lossy();
                                if name_str == "com.amansprojects.starterpack.sdPlugin" {
                                    continue;
                                }
                                let target = self.plugins_dir().join(&file_name);
                                let _ = copy_dir_all(&entry.path(), &target);
                                imported_plugins += 1;
                            }
                        }
                    }
                } else {
                    // Import only selected plugins
                    for plugin_folder in &opts.selected_plugins {
                        let src = src_plugins.join(plugin_folder);
                        let dst = self.plugins_dir().join(plugin_folder);
                        if src.exists() {
                            let _ = copy_dir_all(&src, &dst);
                            imported_plugins += 1;

                            // Also copy associated settings if exists in OpenDeck settings
                            let setting_json = format!("{}.json", plugin_folder);
                            let src_s = src_settings.join(&setting_json);
                            if src_s.exists() {
                                let _ = fs::copy(&src_s, self.settings_dir().join(&setting_json));
                            }
                        }
                    }
                }
            }

            // 3. Applications (Troca automática)
            if opts.import_applications && src_apps_json.exists() {
                let _ = fs::copy(&src_apps_json, self.applications_file());
                imported_applications = true;
            }

            // 4. Settings
            if opts.import_settings {
                if src_settings.exists() {
                    let _ = copy_dir_all(&src_settings, self.settings_dir());
                    imported_settings = true;
                }
                if src_settings_json.exists() && !self.settings_file().exists() {
                    let _ = fs::copy(&src_settings_json, self.settings_file());
                    imported_settings = true;
                }
            }

            // 5. Images
            if opts.import_images && src_images.exists() {
                let _ = copy_dir_all(&src_images, self.images_dir());
                imported_images = true;
            }
        }

        let mut parts = Vec::new();
        if imported_profiles > 0 {
            parts.push(format!("{} perfis", imported_profiles));
        }
        if imported_plugins > 0 {
            parts.push(format!("{} plugins", imported_plugins));
        }
        if imported_applications {
            parts.push("regras de aplicativos".to_string());
        }
        if imported_images {
            parts.push("imagens".to_string());
        }
        if imported_settings {
            parts.push("configurações de plugins".to_string());
        }

        let details = if parts.is_empty() {
            "nenhum item selecionado ou encontrado".to_string()
        } else {
            parts.join(", ")
        };

        let message = format!("Importação concluída com sucesso ({})!", details);
        info!("{}", message);

        Ok(OpenDeckImportReport {
            imported_profiles,
            imported_plugins,
            imported_applications,
            imported_settings,
            imported_images,
            message,
        })
    }

    /// Legacy / full import from OpenDeck into UzlinuxStudioDeck without modifying OpenDeck.
    pub fn import_from_opendeck(&self) -> Result<(usize, usize)> {
        let rep = self.import_from_opendeck_custom(&OpenDeckImportOptions {
            import_all: true,
            import_profiles: true,
            selected_profiles: vec![],
            import_plugins: true,
            selected_plugins: vec![],
            import_applications: true,
            import_settings: true,
            import_images: true,
        })?;
        Ok((rep.imported_profiles, rep.imported_plugins))
    }

    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    pub fn ensure_directories(&self) -> Result<()> {
        fs::create_dir_all(&self.base_dir)
            .with_context(|| format!("Failed to create base config dir {:?}", self.base_dir))?;
        fs::create_dir_all(self.profiles_dir())?;
        fs::create_dir_all(self.plugins_dir())?;
        fs::create_dir_all(self.images_dir())?;
        fs::create_dir_all(self.settings_dir())?;
        self.ensure_bundled_plugins()?;
        Ok(())
    }

    /// Automatically installs bundled default plugins (like Ulanzi D200 driver) so the user never has to download them.
    pub fn ensure_bundled_plugins(&self) -> Result<()> {
        let ulanzi_target = self.plugins_dir().join("com.glmagalhaes.ulanzi.d200.sdPlugin");
        let ulanzi_bin = ulanzi_target.join("rs-ulanzi-d200-linux");

        if !ulanzi_bin.exists() {
            let mut candidates = vec![
                PathBuf::from("bundled_plugins/com.glmagalhaes.ulanzi.d200.sdPlugin"),
                PathBuf::from("../bundled_plugins/com.glmagalhaes.ulanzi.d200.sdPlugin"),
                PathBuf::from("../../bundled_plugins/com.glmagalhaes.ulanzi.d200.sdPlugin"),
                dirs::data_dir().unwrap_or_default().join("uzlinux-studio/plugins/com.glmagalhaes.ulanzi.d200.sdPlugin"),
                dirs::data_dir().unwrap_or_default().join("uzlinux-studiodeck/plugins/com.glmagalhaes.ulanzi.d200.sdPlugin"),
                PathBuf::from("/usr/share/uzlinux-studio/plugins/com.glmagalhaes.ulanzi.d200.sdPlugin"),
                PathBuf::from("/usr/share/uzlinux-studiodeck/plugins/com.glmagalhaes.ulanzi.d200.sdPlugin"),
            ];

            if let Ok(exe_path) = std::env::current_exe() {
                if let Some(parent) = exe_path.parent() {
                    candidates.push(parent.join("bundled_plugins/com.glmagalhaes.ulanzi.d200.sdPlugin"));
                    candidates.push(parent.join("../bundled_plugins/com.glmagalhaes.ulanzi.d200.sdPlugin"));
                    candidates.push(parent.join("../../bundled_plugins/com.glmagalhaes.ulanzi.d200.sdPlugin"));
                    candidates.push(parent.join("../share/uzlinux-studiodeck/plugins/com.glmagalhaes.ulanzi.d200.sdPlugin"));
                }
            }

            let mut source_opt = None;
            for cand in candidates {
                if cand.join("rs-ulanzi-d200-linux").exists() {
                    source_opt = Some(cand);
                    break;
                }
            }

            // Fallback to OpenDeck if present
            if source_opt.is_none() {
                if let Some(opendeck) = Self::find_opendeck_dir() {
                    let od_ulanzi = opendeck.join("plugins/com.glmagalhaes.ulanzi.d200.sdPlugin");
                    if od_ulanzi.join("rs-ulanzi-d200-linux").exists() {
                        source_opt = Some(od_ulanzi);
                    }
                }
            }

            if let Some(source) = source_opt {
                info!("Installing bundled default Ulanzi D200 plugin from {:?}", source);
                let _ = copy_dir_all(&source, &ulanzi_target);

                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(metadata) = fs::metadata(&ulanzi_bin) {
                        let mut perms = metadata.permissions();
                        perms.set_mode(0o755);
                        let _ = fs::set_permissions(&ulanzi_bin, perms);
                    }
                }
            }
        } else {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(metadata) = fs::metadata(&ulanzi_bin) {
                    let mut perms = metadata.permissions();
                    if perms.mode() & 0o111 == 0 {
                        perms.set_mode(0o755);
                        let _ = fs::set_permissions(&ulanzi_bin, perms);
                    }
                }
            }
        }

        Ok(())
    }

    pub fn profiles_dir(&self) -> PathBuf {
        self.base_dir.join("profiles")
    }

    pub fn plugins_dir(&self) -> PathBuf {
        self.base_dir.join("plugins")
    }

    pub fn images_dir(&self) -> PathBuf {
        self.base_dir.join("images")
    }

    pub fn settings_dir(&self) -> PathBuf {
        self.base_dir.join("settings")
    }

    pub fn settings_file(&self) -> PathBuf {
        self.base_dir.join("settings.json")
    }

    pub fn applications_file(&self) -> PathBuf {
        self.base_dir.join("applications.json")
    }

    // Atomic write helper
    fn write_atomic(file_path: &Path, content: &[u8]) -> Result<()> {
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let temp_path = file_path.with_extension(format!("tmp.{}", uuid::Uuid::new_v4()));
        {
            let mut file = fs::File::create(&temp_path)
                .with_context(|| format!("Failed to create temporary file {:?}", temp_path))?;
            file.write_all(content)?;
            file.sync_all()?;
        }
        fs::rename(&temp_path, file_path)
            .with_context(|| format!("Failed to rename {:?} to {:?}", temp_path, file_path))?;
        Ok(())
    }

    // Settings
    pub fn load_settings(&self) -> Result<AppSettings> {
        let path = self.settings_file();
        if !path.exists() {
            let default_settings = AppSettings::default();
            self.save_settings(&default_settings)?;
            return Ok(default_settings);
        }
        let data = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read settings from {:?}", path))?;
        let settings = serde_json::from_str(&data)
            .with_context(|| format!("Failed to parse settings JSON from {:?}", path))?;
        Ok(settings)
    }

    pub fn save_settings(&self, settings: &AppSettings) -> Result<()> {
        let json = serde_json::to_string_pretty(settings)?;
        Self::write_atomic(&self.settings_file(), json.as_bytes())
    }

    // Applications profile map
    pub fn load_applications(&self) -> Result<ApplicationProfiles> {
        let path = self.applications_file();
        if !path.exists() {
            return Ok(ApplicationProfiles::new());
        }
        let data = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read applications from {:?}", path))?;
        let map: std::collections::HashMap<String, std::collections::HashMap<String, String>> =
            serde_json::from_str(&data).unwrap_or_default();
        Ok(ApplicationProfiles { map })
    }

    pub fn save_applications(&self, apps: &ApplicationProfiles) -> Result<()> {
        let json = serde_json::to_string_pretty(&apps.map)?;
        Self::write_atomic(&self.applications_file(), json.as_bytes())
    }

    // Device profile selection: profiles/<device_id>.json
    pub fn get_device_selected_profile(&self, device_id: &str) -> Result<String> {
        let file = self.profiles_dir().join(format!("{}.json", device_id));
        if file.exists() {
            let content = fs::read_to_string(&file)?;
            if let Ok(selection) = serde_json::from_str::<ProfileSelection>(&content) {
                return Ok(selection.selected_profile);
            }
        }
        Ok("Default".to_string())
    }

    pub fn set_device_selected_profile(&self, device_id: &str, profile_name: &str) -> Result<()> {
        let file = self.profiles_dir().join(format!("{}.json", device_id));
        let selection = ProfileSelection {
            selected_profile: profile_name.to_string(),
        };
        let json = serde_json::to_string_pretty(&selection)?;
        Self::write_atomic(&file, json.as_bytes())
    }

    // Profiles for a device: profiles/<device_id>/<profile_name>.json
    pub fn list_profiles(&self, device_id: &str) -> Result<Vec<String>> {
        let device_profiles_dir = self.profiles_dir().join(device_id);
        if !device_profiles_dir.exists() {
            return Ok(vec!["Default".to_string()]);
        }
        let mut profiles = Vec::new();
        for entry in fs::read_dir(device_profiles_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    profiles.push(stem.to_string());
                }
            }
        }
        if profiles.is_empty() {
            profiles.push("Default".to_string());
        }
        profiles.sort();
        Ok(profiles)
    }

    pub fn load_profile(&self, device_id: &str, profile_name: &str) -> Result<Profile> {
        let file = self.profiles_dir().join(device_id).join(format!("{}.json", profile_name));
        if !file.exists() {
            return Err(anyhow!("Profile '{}' not found for device '{}'", profile_name, device_id));
        }
        let content = fs::read_to_string(&file)
            .with_context(|| format!("Failed to read profile file {:?}", file))?;
        let profile = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse profile JSON in {:?}", file))?;
        Ok(profile)
    }

    pub fn save_profile(&self, device_id: &str, profile_name: &str, profile: &Profile) -> Result<()> {
        let dir = self.profiles_dir().join(device_id);
        fs::create_dir_all(&dir)?;
        let file = dir.join(format!("{}.json", profile_name));
        let json = serde_json::to_string_pretty(profile)?;
        Self::write_atomic(&file, json.as_bytes())
    }

    pub fn delete_profile(&self, device_id: &str, profile_name: &str) -> Result<()> {
        let file = self.profiles_dir().join(device_id).join(format!("{}.json", profile_name));
        if file.exists() {
            fs::remove_file(file)?;
        }
        Ok(())
    }

    pub fn rename_profile(&self, device_id: &str, old_name: &str, new_name: &str) -> Result<()> {
        let dir = self.profiles_dir().join(device_id);
        let old_file = dir.join(format!("{}.json", old_name));
        let new_file = dir.join(format!("{}.json", new_name));
        if !old_file.exists() {
            return Err(anyhow!("Profile '{}' does not exist", old_name));
        }
        if new_file.exists() {
            return Err(anyhow!("Profile '{}' already exists", new_name));
        }
        fs::rename(old_file, new_file)?;

        // If this profile was the selected one, update selection
        if let Ok(current) = self.get_device_selected_profile(device_id) {
            if current == old_name {
                let _ = self.set_device_selected_profile(device_id, new_name);
            }
        }
        Ok(())
    }

    // Plugin global settings: settings/<plugin_uuid>.json or settings/<plugin_uuid>.sdPlugin.json
    pub fn load_plugin_settings(&self, plugin_uuid: &str) -> Result<Value> {
        let f1 = self.settings_dir().join(format!("{}.sdPlugin.json", plugin_uuid));
        let f2 = self.settings_dir().join(format!("{}.json", plugin_uuid));
        let file = if f1.exists() {
            f1
        } else if f2.exists() {
            f2
        } else {
            return Ok(Value::Object(serde_json::Map::new()));
        };
        let content = fs::read_to_string(&file)?;
        let val: Value = serde_json::from_str(&content).unwrap_or(Value::Object(serde_json::Map::new()));
        Ok(val)
    }

    pub fn save_plugin_settings(&self, plugin_uuid: &str, settings: &Value) -> Result<()> {
        let f1 = self.settings_dir().join(format!("{}.sdPlugin.json", plugin_uuid));
        let file = if f1.exists() {
            f1
        } else {
            self.settings_dir().join(format!("{}.json", plugin_uuid))
        };
        let json = serde_json::to_string_pretty(settings)?;
        Self::write_atomic(&file, json.as_bytes())
    }
}

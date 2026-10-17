use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyConfig {
    pub id: usize,
    pub col: usize,
    pub row: usize,
    pub pos_str: String,
    pub title: String,
    pub action_type: String,
    pub action_value: String,
    pub icon_path: String,
}

impl Default for KeyConfig {
    fn default() -> Self {
        Self {
            id: 0,
            col: 0,
            row: 0,
            pos_str: "0_0".to_string(),
            title: String::new(),
            action_type: String::new(),
            action_value: String::new(),
            icon_path: String::new(),
        }
    }
}

pub fn default_15_keys() -> Vec<KeyConfig> {
    let mut keys = Vec::with_capacity(15);
    for row in 0..3 {
        for col in 0..5 {
            let id = row * 5 + col;
            let pos_str = format!("{}_{}", col, row);

            // Default sample actions
            let (title, act_type, act_val) = match (col, row) {
                (0, 0) => ("Navegador", "app", "xdg-open https://google.com"),
                (1, 0) => ("Terminal", "app", "kitty"),
                (2, 0) => ("VS Code", "app", "code"),
                (3, 0) => ("Discord", "app", "discord"),
                (4, 0) => ("Steam", "app", "steam"),
                (0, 1) => ("Play/Pause", "media", "playpause"),
                (1, 1) => ("Mute", "media", "mute"),
                (2, 1) => ("Vol -", "media", "voldown"),
                (3, 1) => ("Vol +", "media", "volup"),
                (4, 1) => ("Próxima", "media", "next"),
                (0, 2) => ("YouTube", "website", "https://youtube.com"),
                (1, 2) => ("GitHub", "website", "https://github.com"),
                (2, 2) => ("Copiar", "hotkey", "ctrl+c"),
                (3, 2) => ("Relógio", "clock", "clock"),
                (4, 2) => ("", "", ""),
                _ => ("", "", ""),
            };

            keys.push(KeyConfig {
                id,
                col,
                row,
                pos_str,
                title: title.to_string(),
                action_type: act_type.to_string(),
                action_value: act_val.to_string(),
                icon_path: String::new(),
            });
        }
    }
    keys
}

pub fn create_blank_15_keys() -> Vec<KeyConfig> {
    let mut keys = Vec::with_capacity(15);
    for row in 0..3 {
        for col in 0..5 {
            let id = row * 5 + col;
            let pos_str = format!("{}_{}", col, row);

            let (title, act_type, act_val) = if col == 3 && row == 2 {
                ("Relógio", "clock", "clock")
            } else {
                ("", "", "")
            };

            keys.push(KeyConfig {
                id,
                col,
                row,
                pos_str,
                title: title.to_string(),
                action_type: act_type.to_string(),
                action_value: act_val.to_string(),
                icon_path: String::new(),
            });
        }
    }
    keys
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageConfig {
    pub id: usize,
    pub name: String,
    pub keys: Vec<KeyConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileData {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub current_page: usize,
    pub pages: Vec<PageConfig>,
}

fn default_profile_name() -> String {
    "Padrão".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileConfig {
    #[serde(default = "default_profile_name")]
    pub current_profile: String,
    pub brightness: u8,
    pub sleep_seconds: u32,
    #[serde(default)]
    pub profiles: Vec<ProfileData>,

    // Backward compatibility with legacy profile.json:
    #[serde(default, skip_serializing)]
    pub keys: Vec<KeyConfig>,
    #[serde(default, skip_serializing)]
    pub name: Option<String>,
}

impl Default for ProfileConfig {
    fn default() -> Self {
        let keys = default_15_keys();
        let default_page = PageConfig {
            id: 0,
            name: "Página 1".to_string(),
            keys: keys.clone(),
        };

        let default_profile = ProfileData {
            id: "default".to_string(),
            name: "Padrão".to_string(),
            current_page: 0,
            pages: vec![default_page],
        };

        Self {
            current_profile: "Padrão".to_string(),
            brightness: 70,
            sleep_seconds: 300,
            profiles: vec![default_profile],
            keys,
            name: Some("Padrão".to_string()),
        }
    }
}

impl ProfileConfig {
    pub fn config_path() -> PathBuf {
        let mut dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        dir.push("uzlinux-studio");
        let _ = fs::create_dir_all(&dir);
        dir.push("profile.json");
        dir
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(mut config) = serde_json::from_str::<ProfileConfig>(&content) {
                    // Check if migration is needed from legacy single-page format
                    if config.profiles.is_empty() {
                        let prof_name = config.name.clone().unwrap_or_else(|| "Padrão".to_string());
                        let initial_keys = if config.keys.len() == 15 {
                            config.keys.clone()
                        } else {
                            default_15_keys()
                        };

                        config.profiles = vec![ProfileData {
                            id: "default".to_string(),
                            name: prof_name.clone(),
                            current_page: 0,
                            pages: vec![PageConfig {
                                id: 0,
                                name: "Página 1".to_string(),
                                keys: initial_keys.clone(),
                            }],
                        }];
                        config.current_profile = prof_name;
                        config.keys = initial_keys;
                    } else {
                        // Ensure current_profile is valid
                        let cur_name = config.current_profile.clone();
                        let found = config.profiles.iter().any(|p| p.name == cur_name || p.id == cur_name);
                        if !found && !config.profiles.is_empty() {
                            config.current_profile = config.profiles[0].name.clone();
                        }

                        // Load current page keys into self.keys for seamless compatibility
                        if let Some(prof) = config.profiles.iter().find(|p| p.name == config.current_profile || p.id == config.current_profile) {
                            let p_idx = prof.current_page.min(prof.pages.len().saturating_sub(1));
                            if let Some(page) = prof.pages.get(p_idx) {
                                config.keys = page.keys.clone();
                            }
                        }
                    }

                    return config;
                }
            }
        }
        let mut def = Self::default();
        def.save();
        def
    }

    pub fn save(&mut self) {
        // Sync self.keys into the active page of the active profile before writing to disk
        self.sync_keys_to_active_page();

        let path = Self::config_path();
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = fs::write(path, json);
        }
    }

    pub fn sync_keys_to_active_page(&mut self) {
        let cur_name = self.current_profile.clone();
        if let Some(prof) = self.profiles.iter_mut().find(|p| p.name == cur_name || p.id == cur_name) {
            let p_idx = prof.current_page.min(prof.pages.len().saturating_sub(1));
            if p_idx < prof.pages.len() {
                prof.pages[p_idx].keys = self.keys.clone();
            }
        }
    }

    pub fn active_profile(&self) -> Option<&ProfileData> {
        self.profiles.iter().find(|p| p.name == self.current_profile || p.id == self.current_profile)
    }

    pub fn active_profile_mut(&mut self) -> Option<&mut ProfileData> {
        let cur_name = self.current_profile.clone();
        self.profiles.iter_mut().find(|p| p.name == cur_name || p.id == cur_name)
    }

    pub fn current_page_index(&self) -> usize {
        self.active_profile().map(|p| p.current_page).unwrap_or(0)
    }

    pub fn pages_count(&self) -> usize {
        self.active_profile().map(|p| p.pages.len()).unwrap_or(1)
    }

    pub fn switch_page(&mut self, page_idx: usize) -> bool {
        self.sync_keys_to_active_page();
        if let Some(prof) = self.active_profile_mut() {
            if page_idx < prof.pages.len() {
                prof.current_page = page_idx;
                self.keys = prof.pages[page_idx].keys.clone();
                return true;
            }
        }
        false
    }

    pub fn next_page(&mut self) -> usize {
        self.sync_keys_to_active_page();
        if let Some(prof) = self.active_profile_mut() {
            if prof.pages.is_empty() {
                return 0;
            }
            prof.current_page = (prof.current_page + 1) % prof.pages.len();
            let new_idx = prof.current_page;
            self.keys = prof.pages[new_idx].keys.clone();
            return new_idx;
        }
        0
    }

    pub fn prev_page(&mut self) -> usize {
        self.sync_keys_to_active_page();
        if let Some(prof) = self.active_profile_mut() {
            if prof.pages.is_empty() {
                return 0;
            }
            if prof.current_page == 0 {
                prof.current_page = prof.pages.len() - 1;
            } else {
                prof.current_page -= 1;
            }
            let new_idx = prof.current_page;
            self.keys = prof.pages[new_idx].keys.clone();
            return new_idx;
        }
        0
    }

    pub fn add_page(&mut self) -> usize {
        self.sync_keys_to_active_page();
        if let Some(prof) = self.active_profile_mut() {
            let new_id = prof.pages.len();
            let mut new_keys = create_blank_15_keys();

            // Set back/previous page button on slot 10 (col 0, row 2) by default
            if new_id > 0 {
                new_keys[10] = KeyConfig {
                    id: 10,
                    col: 0,
                    row: 2,
                    pos_str: "0_2".to_string(),
                    title: "Anterior".to_string(),
                    action_type: "page.prev".to_string(),
                    action_value: "prev".to_string(),
                    icon_path: "assets/ulanzi_official/actions/btn_previousPage.png".to_string(),
                };
            }

            let new_page = PageConfig {
                id: new_id,
                name: format!("Página {}", new_id + 1),
                keys: new_keys.clone(),
            };

            // On the previous page, if slot 12 (col 2, row 2) is blank, offer next page
            if let Some(last_page) = prof.pages.last_mut() {
                if last_page.keys.len() > 12 && last_page.keys[12].action_type.is_empty() {
                    last_page.keys[12] = KeyConfig {
                        id: 12,
                        col: 2,
                        row: 2,
                        pos_str: "2_2".to_string(),
                        title: "Próxima".to_string(),
                        action_type: "page.next".to_string(),
                        action_value: "next".to_string(),
                        icon_path: "assets/ulanzi_official/actions/btn_nextPage.png".to_string(),
                    };
                }
            }

            prof.pages.push(new_page);
            prof.current_page = new_id;
            self.keys = new_keys;
            return new_id;
        }
        0
    }

    pub fn delete_page(&mut self, page_idx: usize) -> bool {
        self.sync_keys_to_active_page();
        if let Some(prof) = self.active_profile_mut() {
            if prof.pages.len() <= 1 || page_idx >= prof.pages.len() {
                return false;
            }
            prof.pages.remove(page_idx);
            for (i, p) in prof.pages.iter_mut().enumerate() {
                p.id = i;
                p.name = format!("Página {}", i + 1);
            }
            prof.current_page = prof.current_page.min(prof.pages.len() - 1);
            let cur = prof.current_page;
            self.keys = prof.pages[cur].keys.clone();
            return true;
        }
        false
    }

    pub fn switch_profile(&mut self, profile_name: &str) -> bool {
        self.sync_keys_to_active_page();
        if self.profiles.iter().any(|p| p.name == profile_name || p.id == profile_name) {
            self.current_profile = profile_name.to_string();
            if let Some(prof) = self.active_profile() {
                let p_idx = prof.current_page.min(prof.pages.len().saturating_sub(1));
                if let Some(page) = prof.pages.get(p_idx) {
                    self.keys = page.keys.clone();
                }
            }
            return true;
        }
        false
    }

    pub fn add_profile(&mut self, name: &str) -> String {
        self.sync_keys_to_active_page();
        let trimmed = name.trim();
        let final_name = if trimmed.is_empty() {
            format!("Perfil {}", self.profiles.len() + 1)
        } else {
            trimmed.to_string()
        };

        let blank_keys = create_blank_15_keys();
        let new_profile = ProfileData {
            id: format!("profile_{}", self.profiles.len() + 1),
            name: final_name.clone(),
            current_page: 0,
            pages: vec![PageConfig {
                id: 0,
                name: "Página 1".to_string(),
                keys: blank_keys.clone(),
            }],
        };

        self.profiles.push(new_profile);
        self.current_profile = final_name.clone();
        self.keys = blank_keys;
        final_name
    }

    pub fn duplicate_profile(&mut self, source_name: &str) -> Option<String> {
        self.sync_keys_to_active_page();
        if let Some(src) = self.profiles.iter().find(|p| p.name == source_name || p.id == source_name).cloned() {
            let new_name = format!("{} (Cópia)", src.name);
            let dup = ProfileData {
                id: format!("profile_{}", self.profiles.len() + 1),
                name: new_name.clone(),
                current_page: src.current_page,
                pages: src.pages,
            };
            let keys = if let Some(page) = dup.pages.get(dup.current_page) {
                page.keys.clone()
            } else {
                create_blank_15_keys()
            };
            self.profiles.push(dup);
            self.current_profile = new_name.clone();
            self.keys = keys;
            return Some(new_name);
        }
        None
    }

    pub fn delete_profile(&mut self, profile_name: &str) -> bool {
        self.sync_keys_to_active_page();
        if self.profiles.len() <= 1 {
            return false;
        }
        if let Some(pos) = self.profiles.iter().position(|p| p.name == profile_name || p.id == profile_name) {
            self.profiles.remove(pos);
            if self.current_profile == profile_name {
                self.current_profile = self.profiles[0].name.clone();
            }
            if let Some(prof) = self.active_profile() {
                let p_idx = prof.current_page.min(prof.pages.len().saturating_sub(1));
                if let Some(page) = prof.pages.get(p_idx) {
                    self.keys = page.keys.clone();
                }
            }
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_profile_has_one_page_and_15_keys() {
        let config = ProfileConfig::default();
        assert_eq!(config.profiles.len(), 1);
        assert_eq!(config.current_profile, "Padrão");
        assert_eq!(config.pages_count(), 1);
        assert_eq!(config.current_page_index(), 0);
        assert_eq!(config.keys.len(), 15);
    }

    #[test]
    fn test_add_and_switch_and_delete_page() {
        let mut config = ProfileConfig::default();
        assert_eq!(config.pages_count(), 1);

        // Clear slot 12 on page 1 so add_page can assign "page.next"
        config.keys[12].action_type.clear();
        config.sync_keys_to_active_page();

        // Add page 2
        let p2_idx = config.add_page();
        assert_eq!(p2_idx, 1);
        assert_eq!(config.pages_count(), 2);
        assert_eq!(config.current_page_index(), 1);

        // Page 2 should have button 'Anterior' on slot 10
        assert_eq!(config.keys[10].action_type, "page.prev");

        // Page 1 should have button 'Próxima' on slot 12
        let p1_keys = &config.profiles[0].pages[0].keys;
        assert_eq!(p1_keys[12].action_type, "page.next");

        // Next page cycles back to page 0
        let next_idx = config.next_page();
        assert_eq!(next_idx, 0);
        assert_eq!(config.current_page_index(), 0);

        // Prev page cycles back to page 1
        let prev_idx = config.prev_page();
        assert_eq!(prev_idx, 1);
        assert_eq!(config.current_page_index(), 1);

        // Switch to page 0 directly
        assert!(config.switch_page(0));
        assert_eq!(config.current_page_index(), 0);

        // Delete page 1
        assert!(config.delete_page(1));
        assert_eq!(config.pages_count(), 1);
        assert_eq!(config.current_page_index(), 0);

        // Cannot delete only remaining page
        assert!(!config.delete_page(0));
        assert_eq!(config.pages_count(), 1);
    }

    #[test]
    fn test_profile_create_duplicate_switch_delete() {
        let mut config = ProfileConfig::default();
        assert_eq!(config.profiles.len(), 1);

        // Add new profile "Streaming"
        let stream_name = config.add_profile("Streaming");
        assert_eq!(stream_name, "Streaming");
        assert_eq!(config.current_profile, "Streaming");
        assert_eq!(config.profiles.len(), 2);

        // Modify key in Streaming
        config.keys[0].title = "OBS Live".to_string();
        config.sync_keys_to_active_page();

        // Switch back to "Padrão"
        assert!(config.switch_profile("Padrão"));
        assert_eq!(config.current_profile, "Padrão");
        assert_ne!(config.keys[0].title, "OBS Live");

        // Switch back to "Streaming"
        assert!(config.switch_profile("Streaming"));
        assert_eq!(config.current_profile, "Streaming");
        assert_eq!(config.keys[0].title, "OBS Live");

        // Duplicate "Streaming"
        let dup_name = config.duplicate_profile("Streaming").expect("Duplicate should succeed");
        assert_eq!(dup_name, "Streaming (Cópia)");
        assert_eq!(config.current_profile, "Streaming (Cópia)");
        assert_eq!(config.profiles.len(), 3);
        assert_eq!(config.keys[0].title, "OBS Live");

        // Delete duplicated profile
        assert!(config.delete_profile("Streaming (Cópia)"));
        assert_eq!(config.profiles.len(), 2);
        assert_eq!(config.current_profile, "Padrão");

        // Cannot delete last profile if only 1 left
        assert!(config.delete_profile("Streaming"));
        assert_eq!(config.profiles.len(), 1);
        assert!(!config.delete_profile("Padrão"));
    }

    #[test]
    fn test_legacy_profile_migration() {
        // Legacy single-page JSON format without `profiles` array
        let legacy_json = r#"{
            "brightness": 80,
            "sleep_seconds": 600,
            "name": "Meu Setup Antigo",
            "keys": [
                {
                    "id": 0, "col": 0, "row": 0, "pos_str": "0_0",
                    "title": "Legado 1", "action_type": "app", "action_value": "firefox", "icon_path": ""
                }
            ]
        }"#;

        let mut config = serde_json::from_str::<ProfileConfig>(legacy_json).expect("Deserialization");
        assert!(config.profiles.is_empty());

        // Now test migration during ProfileConfig logic (as done in load())
        let prof_name = config.name.clone().unwrap_or_else(|| "Padrão".to_string());
        config.profiles = vec![ProfileData {
            id: "default".to_string(),
            name: prof_name.clone(),
            current_page: 0,
            pages: vec![PageConfig {
                id: 0,
                name: "Página 1".to_string(),
                keys: default_15_keys(),
            }],
        }];
        config.current_profile = prof_name;
        assert_eq!(config.profiles.len(), 1);
        assert_eq!(config.current_profile, "Meu Setup Antigo");
        assert_eq!(config.pages_count(), 1);
    }
}


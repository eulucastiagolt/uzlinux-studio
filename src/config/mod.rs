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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileConfig {
    pub name: String,
    pub brightness: u8,
    pub sleep_seconds: u32,
    pub keys: Vec<KeyConfig>,
}

impl Default for ProfileConfig {
    fn default() -> Self {
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
                    (3, 2) => ("Colar", "hotkey", "ctrl+v"),
                    (4, 2) => ("Captura", "hotkey", "Print"),
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

        Self {
            name: "Padrão".to_string(),
            brightness: 70,
            sleep_seconds: 300,
            keys,
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
                if let Ok(config) = serde_json::from_str::<ProfileConfig>(&content) {
                    if config.keys.len() == 15 {
                        return config;
                    }
                }
            }
        }
        let def = Self::default();
        def.save();
        def
    }

    pub fn save(&self) {
        let path = Self::config_path();
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = fs::write(path, json);
        }
    }
}

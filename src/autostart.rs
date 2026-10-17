#![allow(dead_code)]

use std::fs;
use std::path::PathBuf;

/// Path to ~/.config/autostart/uzlinux-studio.desktop
pub fn get_autostart_desktop_path() -> Option<PathBuf> {
    dirs::config_dir().map(|mut d| {
        d.push("autostart");
        d.push("uzlinux-studio.desktop");
        d
    })
}

/// Checks if the autostart .desktop file is currently installed in the user's autostart directory
pub fn is_autostart_enabled() -> bool {
    if let Some(path) = get_autostart_desktop_path() {
        path.exists()
    } else {
        false
    }
}

/// Enables or disables automatic startup on Linux session login
pub fn set_autostart_enabled(enable: bool) -> Result<(), String> {
    let path = get_autostart_desktop_path()
        .ok_or_else(|| "Não foi possível resolver o diretório de autostart (~/.config/autostart)".to_string())?;

    if enable {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Falha ao criar diretório autostart: {}", e))?;
        }

        // Determine executable path
        let exec_cmd = std::env::current_exe()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "uzlinux-studio".to_string());

        let desktop_content = format!(
r#"[Desktop Entry]
Type=Application
Name=Uzlinux Studio
GenericName=Ulanzi Deck Controller
Comment=Controle nativo do Ulanzi D200 Stream Deck no Linux
Exec={}
Icon=uzlinux-studio
Terminal=false
Categories=Utility;AudioVideo;HardwareSettings;
StartupNotify=false
X-GNOME-Autostart-enabled=true
"#,
            exec_cmd
        );

        fs::write(&path, desktop_content)
            .map_err(|e| format!("Erro ao escrever ~/.config/autostart/uzlinux-studio.desktop: {}", e))?;
    } else if path.exists() {
        fs::remove_file(&path)
            .map_err(|e| format!("Erro ao remover atalho de autostart: {}", e))?;
    }

    Ok(())
}

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct AppInfo {
    pub name: String,
    pub exec_cmd: String,
    pub icon_path: String,
}

/// Resolves an icon name from .desktop to a valid absolute file path
pub fn resolve_system_icon(icon_name: &str) -> String {
    let name = icon_name.trim();
    if name.is_empty() {
        return String::new();
    }

    // Direct path
    let p = Path::new(name);
    if p.is_absolute() && p.exists() {
        return name.to_string();
    }

    // Check /usr/share/pixmaps
    for ext in &["png", "svg", "xpm", "jpg"] {
        let candidate = PathBuf::from(format!("/usr/share/pixmaps/{}.{}", name, ext));
        if candidate.exists() {
            return candidate.to_string_lossy().to_string();
        }
    }

    // Check common icon themes: hicolor, Papirus, breeze, Adwaita
    let sizes = ["256x256", "128x128", "64x64", "48x48", "32x32", "scalable"];
    let bases = [
        "/usr/share/icons/hicolor",
        "/usr/share/icons/Papirus",
        "/usr/share/icons/breeze",
        "/usr/share/icons/Adwaita",
        "/usr/share/icons/gnome",
    ];

    for base in &bases {
        for size in &sizes {
            for category in &["apps", "categories", "devices", "places", "status"] {
                for ext in &["png", "svg"] {
                    let candidate = PathBuf::from(format!("{}/{}/{}/{}.{}", base, size, category, name, ext));
                    if candidate.exists() {
                        return candidate.to_string_lossy().to_string();
                    }
                }
            }
        }
    }

    String::new()
}

/// Cleans freedesktop field codes (%u, %U, %f, %F, %i, %c, %k) from Exec line
fn clean_exec_line(raw_exec: &str) -> String {
    let mut parts: Vec<&str> = raw_exec.split_whitespace().collect();
    parts.retain(|p| !p.starts_with('%'));
    parts.join(" ")
}

/// Scans installed desktop applications from freedesktop standard locations
pub fn scan_system_apps() -> Vec<AppInfo> {
    let mut app_dirs = vec![
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/var/lib/flatpak/exports/share/applications"),
        PathBuf::from("/var/lib/snapd/desktop/applications"),
    ];

    if let Some(home) = dirs::home_dir() {
        app_dirs.push(home.join(".local/share/applications"));
        app_dirs.push(home.join(".local/share/flatpak/exports/share/applications"));
    }

    let mut result = Vec::new();
    let mut seen_execs = std::collections::HashSet::new();

    for dir in app_dirs {
        if !dir.exists() {
            continue;
        }

        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "desktop") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Some(app) = parse_desktop_entry(&content) {
                            if !seen_execs.contains(&app.exec_cmd) {
                                seen_execs.insert(app.exec_cmd.clone());
                                result.push(app);
                            }
                        }
                    }
                }
            }
        }
    }

    result.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    result
}

/// Parses a single .desktop file content
fn parse_desktop_entry(content: &str) -> Option<AppInfo> {
    let mut in_desktop_entry = false;
    let mut is_application = false;
    let mut no_display = false;
    let mut hidden = false;
    let mut name = String::new();
    let mut exec = String::new();
    let mut icon = String::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_desktop_entry = trimmed == "[Desktop Entry]";
            continue;
        }

        if !in_desktop_entry {
            continue;
        }

        if let Some((k, v)) = trimmed.split_once('=') {
            let key = k.trim();
            let val = v.trim();

            match key {
                "Type" => {
                    is_application = val.eq_ignore_ascii_case("application");
                }
                "NoDisplay" => {
                    no_display = val.eq_ignore_ascii_case("true");
                }
                "Hidden" => {
                    hidden = val.eq_ignore_ascii_case("true");
                }
                "Name" if name.is_empty() => {
                    name = val.to_string();
                }
                "Exec" if exec.is_empty() => {
                    exec = clean_exec_line(val);
                }
                "Icon" if icon.is_empty() => {
                    icon = val.to_string();
                }
                _ => {}
            }
        }
    }

    if is_application && !no_display && !hidden && !name.is_empty() && !exec.is_empty() {
        let icon_path = resolve_system_icon(&icon);
        Some(AppInfo {
            name,
            exec_cmd: exec,
            icon_path,
        })
    } else {
        None
    }
}

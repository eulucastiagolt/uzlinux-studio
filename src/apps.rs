use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct AppInfo {
    pub name: String,
    pub exec_cmd: String,
    pub icon_path: String,
    pub category: String,
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

    // Candidate directories to search
    let mut search_dirs = Vec::new();
    if let Some(home) = dirs::home_dir() {
        search_dirs.push(home.join(".local/share/icons"));
        search_dirs.push(home.join(".icons"));
    }
    search_dirs.push(PathBuf::from("/usr/share/pixmaps"));
    search_dirs.push(PathBuf::from("/usr/share/icons"));

    // 1. Direct match in root of search dirs
    for dir in &search_dirs {
        for ext in &["png", "svg", "xpm", "jpg", "jpeg"] {
            let candidate = dir.join(format!("{}.{}", name, ext));
            if candidate.exists() {
                return candidate.to_string_lossy().to_string();
            }
        }
    }

    // 2. Structured icon themes (hicolor, Papirus, breeze, Adwaita, gnome, etc.)
    let sizes = ["256x256", "128x128", "96x96", "64x64", "48x48", "32x32", "scalable"];
    let categories = ["apps", "categories", "devices", "places", "status", "mimetypes"];

    for dir in &search_dirs {
        if let Ok(theme_entries) = fs::read_dir(dir) {
            for theme_entry in theme_entries.flatten() {
                let theme_path = theme_entry.path();
                if theme_path.is_dir() {
                    for size in &sizes {
                        for cat in &categories {
                            for ext in &["png", "svg"] {
                                let candidate = theme_path.join(size).join(cat).join(format!("{}.{}", name, ext));
                                if candidate.exists() {
                                    return candidate.to_string_lossy().to_string();
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 3. Fallback: case-insensitive match in ~/.local/share/icons and pixmaps
    let name_lower = name.to_lowercase();
    for dir in &search_dirs {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                    if stem.to_lowercase() == name_lower {
                        if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                            if ["png", "svg", "xpm", "jpg", "jpeg"].contains(&ext.to_lowercase().as_str()) {
                                return p.to_string_lossy().to_string();
                            }
                        }
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

/// Parses a desktop file directly from path
pub fn scan_desktop_file(path: &Path) -> Option<AppInfo> {
    if let Ok(content) = fs::read_to_string(path) {
        parse_desktop_entry(&content)
    } else {
        None
    }
}

/// Maps FreeDesktop categories to friendly user-facing category names
pub fn categorize_desktop_categories(raw_categories: &str) -> &'static str {
    let cats = raw_categories.to_lowercase();
    if cats.contains("development") || cats.contains("ide") {
        "Desenvolvimento"
    } else if cats.contains("game") {
        "Jogos"
    } else if cats.contains("audiovideo") || cats.contains("audio") || cats.contains("video") || cats.contains("player") || cats.contains("music") {
        "Multimídia"
    } else if cats.contains("graphics") || cats.contains("photography") || cats.contains("2dgraphics") || cats.contains("rastergraphics") {
        "Gráficos & Design"
    } else if cats.contains("network") || cats.contains("webbrowser") || cats.contains("chat") || cats.contains("instantmessaging") || cats.contains("email") {
        "Internet & Rede"
    } else if cats.contains("office") || cats.contains("wordprocessor") || cats.contains("spreadsheet") || cats.contains("presentation") {
        "Escritório"
    } else if cats.contains("settings") || cats.contains("desktopsettings") || cats.contains("hardwaresettings") {
        "Configurações"
    } else if cats.contains("system") || cats.contains("terminalemulator") || cats.contains("monitor") || cats.contains("packagemanager") {
        "Sistema"
    } else if cats.contains("utility") || cats.contains("filemanager") || cats.contains("archiving") || cats.contains("calculator") || cats.contains("core") {
        "Utilitários"
    } else {
        "Outros"
    }
}

/// Parses a single .desktop file content
pub fn parse_desktop_entry(content: &str) -> Option<AppInfo> {
    let mut in_desktop_entry = false;
    let mut is_application = false;
    let mut no_display = false;
    let mut hidden = false;
    let mut name = String::new();
    let mut exec = String::new();
    let mut icon = String::new();
    let mut raw_categories = String::new();

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
                "Categories" if raw_categories.is_empty() => {
                    raw_categories = val.to_string();
                }
                _ => {}
            }
        }
    }

    if is_application && !no_display && !hidden && !name.is_empty() && !exec.is_empty() {
        let icon_path = resolve_system_icon(&icon);
        let category = categorize_desktop_categories(&raw_categories).to_string();
        Some(AppInfo {
            name,
            exec_cmd: exec,
            icon_path,
            category,
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_system_icon() {
        let p = resolve_system_icon("affinity");
        println!("Resolved affinity icon: {:?}", p);
        assert!(!p.is_empty());
        let p2 = resolve_system_icon("Android_Dex");
        println!("Resolved Android_Dex icon: {:?}", p2);
        assert!(!p2.is_empty());
    }
}

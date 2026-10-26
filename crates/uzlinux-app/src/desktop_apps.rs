use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use log::info;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;

#[cfg(unix)]
use std::os::unix::process::CommandExt;

/// Detaches a command completely from the parent process group and session.
/// This prevents child processes from closing when uzlinux-app is terminated.
pub fn detach_command(cmd: &mut std::process::Command) {
    cmd.stdin(Stdio::null())
       .stdout(Stdio::null())
       .stderr(Stdio::null());

    #[cfg(unix)]
    {
        cmd.process_group(0);
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DesktopApp {
    pub id: String,
    pub name: String,
    pub exec: String,
    pub icon: Option<String>,
    pub icon_data: Option<String>,
    pub terminal: bool,
    pub path: String,
    pub comment: Option<String>,
}

fn icon_quality_score(path: &Path) -> u32 {
    let s = path.to_string_lossy();
    if s.contains("scalable") || s.ends_with(".svg") {
        100
    } else if s.contains("128x128") {
        90
    } else if s.contains("64x64") {
        85
    } else if s.contains("48x48") {
        80
    } else if s.contains("256x256") {
        75
    } else if s.contains("32x32") {
        60
    } else if s.contains("pixmaps") {
        50
    } else if s.contains("22x22") {
        40
    } else if s.contains("16x16") {
        30
    } else {
        50
    }
}

fn index_icons_recursive(dir: &Path, index: &mut HashMap<String, PathBuf>, depth: usize, max_depth: usize) {
    if depth > max_depth {
        return;
    }
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                index_icons_recursive(&path, index, depth + 1, max_depth);
            } else if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    let ext_lower = ext.to_lowercase();
                    if ext_lower == "png" || ext_lower == "svg" || ext_lower == "webp" {
                        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                            let key = stem.to_string();
                            let key_lower = stem.to_lowercase();
                            let score = icon_quality_score(&path);

                            let should_update = match index.get(&key) {
                                Some(existing) => score > icon_quality_score(existing),
                                None => true,
                            };
                            if should_update {
                                index.insert(key, path.clone());
                            }

                            let should_update_lower = match index.get(&key_lower) {
                                Some(existing) => score > icon_quality_score(existing),
                                None => true,
                            };
                            if should_update_lower {
                                index.insert(key_lower, path);
                            }
                        }
                    }
                }
            }
        }
    }
}

fn build_system_icon_index() -> HashMap<String, PathBuf> {
    let mut index = HashMap::new();
    let mut dirs = vec![
        PathBuf::from("/usr/share/pixmaps"),
        PathBuf::from("/usr/share/icons/hicolor"),
        PathBuf::from("/usr/share/icons/breeze-dark"),
        PathBuf::from("/usr/share/icons/breeze"),
        PathBuf::from("/usr/share/icons/bigicons-papient"),
        PathBuf::from("/usr/share/icons/bigicons-papient-dark"),
        PathBuf::from("/usr/share/icons/Adwaita"),
        PathBuf::from("/var/lib/flatpak/exports/share/icons"),
    ];

    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".local/share/icons"));
        dirs.push(home.join(".local/share/flatpak/exports/share/icons"));
    }

    for dir in dirs {
        if dir.exists() {
            index_icons_recursive(&dir, &mut index, 0, 5);
        }
    }

    index
}

fn resolve_icon_data(icon_name_or_path: &str, index: &HashMap<String, PathBuf>) -> Option<String> {
    let p = Path::new(icon_name_or_path);
    let icon_path = if p.is_absolute() && p.exists() {
        Some(p.to_path_buf())
    } else {
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or(icon_name_or_path);
        index.get(stem)
            .or_else(|| index.get(&stem.to_lowercase()))
            .cloned()
    };

    if let Some(actual_path) = icon_path {
        if let Ok(bytes) = fs::read(&actual_path) {
            let ext = actual_path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
            let mime = match ext.as_str() {
                "svg" => "image/svg+xml",
                "png" => "image/png",
                "webp" => "image/webp",
                _ => "image/png",
            };
            let b64 = BASE64.encode(&bytes);
            return Some(format!("data:{};base64,{}", mime, b64));
        }
    }

    None
}

static APPS_CACHE: std::sync::RwLock<Option<Vec<DesktopApp>>> = std::sync::RwLock::new(None);

pub fn get_installed_desktop_apps() -> Vec<DesktopApp> {
    if let Ok(guard) = APPS_CACHE.read() {
        if let Some(ref cached) = *guard {
            return cached.clone();
        }
    }

    let apps = scan_desktop_apps_internal();
    if let Ok(mut guard) = APPS_CACHE.write() {
        *guard = Some(apps.clone());
    }
    apps
}

pub fn refresh_installed_desktop_apps_cache() -> Vec<DesktopApp> {
    let apps = scan_desktop_apps_internal();
    if let Ok(mut guard) = APPS_CACHE.write() {
        *guard = Some(apps.clone());
    }
    apps
}

fn scan_desktop_apps_internal() -> Vec<DesktopApp> {
    let icon_index = build_system_icon_index();

    let mut dirs = vec![
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
        PathBuf::from("/var/lib/flatpak/exports/share/applications"),
    ];

    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".local/share/applications"));
        dirs.push(home.join(".local/share/flatpak/exports/share/applications"));
    }

    let mut apps = Vec::new();
    let mut seen_ids = HashSet::new();

    for dir in dirs {
        if !dir.exists() {
            continue;
        }
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("desktop") {
                    let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                    if seen_ids.contains(file_stem) {
                        continue;
                    }

                    if let Some(mut app) = parse_desktop_file(&path) {
                        if let Some(ref icon_name) = app.icon {
                            app.icon_data = resolve_icon_data(icon_name, &icon_index);
                        }
                        seen_ids.insert(file_stem.to_string());
                        apps.push(app);
                    }
                }
            }
        }
    }

    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps
}

fn parse_desktop_file(path: &Path) -> Option<DesktopApp> {
    let content = fs::read_to_string(path).ok()?;
    let mut is_desktop_entry_section = false;
    let mut name = None;
    let mut exec = None;
    let mut icon = None;
    let mut comment = None;
    let mut terminal = false;
    let mut no_display = false;
    let mut app_type = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            is_desktop_entry_section = trimmed == "[Desktop Entry]";
            continue;
        }

        if !is_desktop_entry_section || trimmed.starts_with('#') {
            continue;
        }

        if let Some((key, val)) = trimmed.split_once('=') {
            let key = key.trim();
            let val = val.trim();

            match key {
                "Type" => app_type = Some(val.to_string()),
                "Name" if name.is_none() => name = Some(val.to_string()),
                "Name[pt_BR]" | "Name[pt]" => name = Some(val.to_string()),
                "Exec" if exec.is_none() => exec = Some(val.to_string()),
                "Icon" => icon = Some(val.to_string()),
                "Comment" if comment.is_none() => comment = Some(val.to_string()),
                "NoDisplay" => no_display = val.eq_ignore_ascii_case("true"),
                "Terminal" => terminal = val.eq_ignore_ascii_case("true"),
                _ => {}
            }
        }
    }

    if no_display {
        return None;
    }

    if let Some(t) = app_type {
        if t != "Application" {
            return None;
        }
    }

    let name = name?;
    let exec = exec?;
    let id = path.file_stem().and_then(|s| s.to_str()).unwrap_or(&name).to_string();

    Some(DesktopApp {
        id,
        name,
        exec,
        icon,
        icon_data: None,
        terminal,
        path: path.to_string_lossy().to_string(),
        comment,
    })
}

pub fn launch_app(app_id_or_exec: &str, custom_args: Option<&str>) -> anyhow::Result<()> {
    info!("Launching desktop application: '{}' (args: {:?})", app_id_or_exec, custom_args);

    // 1. If app_id_or_exec is a path to a .desktop file, try gio launch first
    let p = Path::new(app_id_or_exec);
    if p.exists() && p.extension().and_then(|e| e.to_str()) == Some("desktop") {
        let mut gio_cmd = std::process::Command::new("gio");
        gio_cmd.arg("launch").arg(p);
        if let Some(args) = custom_args {
            for arg in args.split_whitespace() {
                gio_cmd.arg(arg);
            }
        }
        detach_command(&mut gio_cmd);
        if let Ok(mut child) = gio_cmd.spawn() {
            let status = child.wait();
            if let Ok(s) = status {
                if s.success() {
                    return Ok(());
                }
            }
        }
    }

    // 2. Try gtk-launch if app_id_or_exec is a desktop id
    let desktop_id = if app_id_or_exec.ends_with(".desktop") {
        app_id_or_exec.strip_suffix(".desktop").unwrap_or(app_id_or_exec)
    } else {
        app_id_or_exec
    };

    if !desktop_id.contains('/') && !desktop_id.contains(' ') {
        let mut gtk_cmd = std::process::Command::new("gtk-launch");
        gtk_cmd.arg(desktop_id);
        if let Some(args) = custom_args {
            for arg in args.split_whitespace() {
                gtk_cmd.arg(arg);
            }
        }
        detach_command(&mut gtk_cmd);
        if let Ok(mut child) = gtk_cmd.spawn() {
            let status = child.wait();
            if let Ok(s) = status {
                if s.success() {
                    return Ok(());
                }
            }
        }
    }

    // 3. Fallback: clean the exec string and spawn detached
    let mut parts: Vec<&str> = app_id_or_exec
        .split_whitespace()
        .filter(|arg| !arg.starts_with('%'))
        .collect();

    if let Some(args) = custom_args {
        for arg in args.split_whitespace() {
            parts.push(arg);
        }
    }

    if parts.is_empty() {
        anyhow::bail!("Exec string is empty");
    }

    let program = parts[0];
    let args = &parts[1..];

    let mut cmd = std::process::Command::new(program);
    cmd.args(args);
    detach_command(&mut cmd);
    cmd.spawn()?;

    Ok(())
}

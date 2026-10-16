use std::process::Command;
use std::thread;

pub fn execute_action(action_type: &str, action_value: &str) {
    let act_type = action_type.to_lowercase();
    let act_val = action_value.trim().to_string();

    if act_val.is_empty() {
        return;
    }

    thread::spawn(move || match act_type.as_str() {
        "app" => {
            println!("[Action] Executando App: {}", act_val);
            let _ = Command::new("sh")
                .arg("-c")
                .arg(format!("{} &", act_val))
                .spawn();
        }
        "website" | "url" => {
            let url = if !act_val.starts_with("http://") && !act_val.starts_with("https://") {
                format!("https://{}", act_val)
            } else {
                act_val
            };
            println!("[Action] Abrindo Website: {}", url);
            let _ = open::that(url);
        }
        "hotkey" => {
            println!("[Action] Simulando Atalho: {}", act_val);
            simulate_hotkey(&act_val);
        }
        "media" => {
            println!("[Action] Controle de Mídia: {}", act_val);
            handle_media_command(&act_val);
        }
        "text" => {
            println!("[Action] Digitando Texto: {}", act_val);
            type_text(&act_val);
        }
        _ => {
            eprintln!("[Action] Tipo desconhecido: {}", act_type);
        }
    });
}

fn simulate_hotkey(hotkey: &str) {
    // Try ydotool first (Wayland compatible)
    // Convert common names: ctrl+c, alt+tab, super, etc.
    // If ydotool is available:
    let xkey = hotkey.replace("control", "ctrl").replace("command", "super");
    
    // Try xdotool (works on X11 & XWayland)
    let _ = Command::new("xdotool")
        .arg("key")
        .arg(&xkey)
        .status();
}

fn type_text(text: &str) {
    // Try xdotool type
    let _ = Command::new("xdotool")
        .arg("type")
        .arg("--clearmodifiers")
        .arg(text)
        .status();
}

fn handle_media_command(cmd: &str) {
    let normalized = cmd.to_lowercase();
    let (mpris_method, xf86_key) = match normalized.as_str() {
        "play" | "pause" | "playpause" => ("PlayPause", "XF86AudioPlay"),
        "next" | "nexttrack" => ("Next", "XF86AudioNext"),
        "prev" | "prevtrack" | "previous" => ("Previous", "XF86AudioPrev"),
        "mute" => ("", "XF86AudioMute"),
        "volup" | "volumeup" => ("", "XF86AudioRaiseVolume"),
        "voldown" | "volumedown" => ("", "XF86AudioLowerVolume"),
        _ => ("PlayPause", "XF86AudioPlay"),
    };

    // Try MPRIS D-Bus method first (via busctl) for audio players
    if !mpris_method.is_empty() {
        let busctl_cmd = format!(
            "busctl --user call org.mpris.MediaPlayer2.* /org/mpris/MediaPlayer2 org.mpris.MediaPlayer2.Player {}",
            mpris_method
        );
        let _ = Command::new("sh").arg("-c").arg(&busctl_cmd).status();
    }

    // Also trigger the media XF86 key for desktop environment integration (GNOME, KDE, etc.)
    if !xf86_key.is_empty() {
        let _ = Command::new("xdotool").arg("key").arg(xf86_key).status();
    }
}

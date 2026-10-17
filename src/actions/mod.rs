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
        "command" | "comando" | "cmd" | "shell" | "script" => {
            println!("[Action] Executando Comando Linux: {}", act_val);
            let _ = Command::new("sh")
                .arg("-c")
                .arg(&act_val)
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
        "close" => {
            println!("[Action] Fechando Aplicativo/Janela: {}", act_val);
            if act_val == "window" || act_val == "current" || act_val.is_empty() {
                let _ = Command::new("xdotool").args(["getactivewindow", "windowclose"]).status();
            } else {
                let _ = Command::new("killall").arg("-SIGTERM").arg(&act_val).status();
            }
        }
        "sound" | "audio" => {
            println!("[Action] Reproduzindo Áudio / Soundboard: {}", act_val);
            if act_val == "stop" || act_val == "parar" {
                stop_all_sounds();
            } else {
                play_sound_file(&act_val);
            }
        }
        "multiaction" | "macro" => {
            println!("[Action] Executando Macro Multi-Action: {}", act_val);
            execute_multi_action(&act_val);
        }
        "obs" | "obsstudio" => {
            println!("[Action] Controle OBS Studio: {}", act_val);
            handle_obs_command(&act_val);
        }
        _ => {
            eprintln!("[Action] Tipo desconhecido: {}", act_type);
        }
    });
}

fn stop_all_sounds() {
    let _ = Command::new("pkill").arg("paplay").status();
    let _ = Command::new("pkill").arg("pw-play").status();
    let _ = Command::new("pkill").arg("aplay").status();
    let _ = Command::new("pkill").arg("ffplay").status();
}

fn handle_obs_command(cmd: &str) {
    let normalized = cmd.trim().to_lowercase();
    let obs_cli_arg = match normalized.as_str() {
        "record" | "gravar" => Some("recording toggle"),
        "recordpause" | "pausar" => Some("recording toggle-pause"),
        "stream" | "transmissao" | "live" => Some("streaming toggle"),
        "virtualcam" | "camera" => Some("virtual-cam toggle"),
        "studiomode" | "modo_estudio" => Some("studio-mode toggle"),
        _ => None,
    };
    if let Some(arg) = obs_cli_arg {
        let parts: Vec<&str> = arg.split_whitespace().collect();
        if let Ok(status) = Command::new("obs-cli").args(&parts).status() {
            if status.success() {
                return;
            }
        }
    }

    match normalized.as_str() {
        "record" | "gravar" => {
            let _ = Command::new("xdotool").args(["key", "ctrl+shift+r"]).status();
        }
        "recordpause" | "pausar" => {
            let _ = Command::new("xdotool").args(["key", "ctrl+shift+p"]).status();
        }
        "stream" | "transmissao" | "live" => {
            let _ = Command::new("xdotool").args(["key", "ctrl+shift+s"]).status();
        }
        "virtualcam" | "camera" => {
            let _ = Command::new("xdotool").args(["key", "ctrl+shift+v"]).status();
        }
        "studiomode" | "modo_estudio" => {
            let _ = Command::new("xdotool").args(["key", "ctrl+shift+m"]).status();
        }
        _ => {}
    }
}

fn play_sound_file(path_str: &str) {
    let p = std::path::Path::new(path_str);
    if !p.exists() {
        eprintln!("[Sound] Arquivo de áudio não encontrado: {}", path_str);
        return;
    }
    // Try paplay (PipeWire/PulseAudio)
    if let Ok(mut child) = Command::new("paplay").arg(path_str).spawn() {
        let _ = child.wait();
        return;
    }
    // Try pw-play
    if let Ok(mut child) = Command::new("pw-play").arg(path_str).spawn() {
        let _ = child.wait();
        return;
    }
    // Try aplay (ALSA)
    if let Ok(mut child) = Command::new("aplay").arg(path_str).spawn() {
        let _ = child.wait();
        return;
    }
    // Fallback ffplay
    let _ = Command::new("ffplay").args(["-nodisp", "-autoexit", path_str]).status();
}

fn execute_multi_action(macro_str: &str) {
    let steps: Vec<&str> = macro_str.split(';').collect();
    for step in steps {
        let trimmed = step.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("delay ") || trimmed.starts_with("sleep ") {
            let ms_str = trimmed.trim_start_matches("delay ").trim_start_matches("sleep ").trim();
            if let Ok(ms) = ms_str.parse::<u64>() {
                thread::sleep(std::time::Duration::from_millis(ms));
                continue;
            }
        }
        if let Some((stype, sval)) = trimmed.split_once(':') {
            execute_action(stype, sval);
        } else if trimmed.contains('+') {
            simulate_hotkey(trimmed);
        } else {
            let _ = Command::new("sh").arg("-c").arg(trimmed).spawn();
        }
        thread::sleep(std::time::Duration::from_millis(50));
    }
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
    let normalized = cmd.trim().to_lowercase();

    // Volume controls
    match normalized.as_str() {
        "volup" | "volumeup" | "volume_up" | "volume-up" | "vol+" | "vol_+" | "aumentar_volume" => {
            handle_volume_change(true);
            return;
        }
        "voldown" | "volumedown" | "volume_down" | "volume-down" | "vol-" | "vol_-" | "diminuir_volume" => {
            handle_volume_change(false);
            return;
        }
        "mute" | "mutetoggle" | "mute_toggle" | "togglemute" | "mudo" => {
            handle_mute_toggle();
            return;
        }
        _ => {}
    }

    // Playback controls (Play, Pause, Play/Pause, Next, Previous, Stop)
    let (mpris_method, xf86_key) = match normalized.as_str() {
        "play" => ("Play", "XF86AudioPlay"),
        "pause" => ("Pause", "XF86AudioPause"),
        "playpause" | "play_pause" | "play-pause" | "play/pause" => ("PlayPause", "XF86AudioPlay"),
        "next" | "nexttrack" | "next_track" | "next-track" | "proxima" | "próxima" => ("Next", "XF86AudioNext"),
        "prev" | "prevtrack" | "previous" | "previous_track" | "anterior" => ("Previous", "XF86AudioPrev"),
        "stop" => ("Stop", "XF86AudioStop"),
        _ => ("PlayPause", "XF86AudioPlay"),
    };

    handle_player_control(mpris_method, xf86_key);
}

fn handle_player_control(method: &str, xf86_key: &str) {
    // 1. If playerctl is available, try it first
    let playerctl_arg = match method {
        "PlayPause" => Some("play-pause"),
        "Play" => Some("play"),
        "Pause" => Some("pause"),
        "Next" => Some("next"),
        "Previous" => Some("previous"),
        "Stop" => Some("stop"),
        _ => None,
    };

    if let Some(arg) = playerctl_arg {
        if let Ok(status) = Command::new("playerctl").arg(arg).status() {
            if status.success() {
                return;
            }
        }
    }

    // 2. Query active MPRIS services via busctl (no wildcards)
    let players = get_active_mpris_players();
    if !players.is_empty() {
        // If one player is actively Playing, prioritize it
        let mut target_player: Option<String> = None;
        for p in &players {
            if let Some(status) = get_player_status(p) {
                if status == "Playing" {
                    target_player = Some(p.clone());
                    break;
                }
            }
        }

        // If none is Playing, select the first player
        let chosen = target_player.unwrap_or_else(|| players[0].clone());
        println!("[Media] Enviando MPRIS {} para {}", method, chosen);
        let ok = send_mpris_command(&chosen, method);
        if ok {
            return;
        }

        // If chosen failed, try other players
        for p in &players {
            if p != &chosen {
                let _ = send_mpris_command(p, method);
            }
        }
        return;
    }

    // 3. Fallback to desktop XF86 key simulation
    if !xf86_key.is_empty() {
        let _ = Command::new("xdotool").arg("key").arg(xf86_key).status();
    }
}

fn get_active_mpris_players() -> Vec<String> {
    let output = Command::new("busctl")
        .args(["--user", "list", "--no-legend"])
        .output();

    if let Ok(out) = output {
        let text = String::from_utf8_lossy(&out.stdout);
        let players: Vec<String> = text
            .lines()
            .filter_map(|line| {
                let name = line.split_whitespace().next()?;
                if name.starts_with("org.mpris.MediaPlayer2.") {
                    Some(name.to_string())
                } else {
                    None
                }
            })
            .collect();
        return players;
    }
    Vec::new()
}

fn get_player_status(service: &str) -> Option<String> {
    let output = Command::new("busctl")
        .args([
            "--user",
            "get-property",
            service,
            "/org/mpris/MediaPlayer2",
            "org.mpris.MediaPlayer2.Player",
            "PlaybackStatus",
        ])
        .output()
        .ok()?;

    let text = String::from_utf8_lossy(&output.stdout);
    if text.contains("\"Playing\"") {
        Some("Playing".to_string())
    } else if text.contains("\"Paused\"") {
        Some("Paused".to_string())
    } else if text.contains("\"Stopped\"") {
        Some("Stopped".to_string())
    } else {
        None
    }
}

fn send_mpris_command(service: &str, method: &str) -> bool {
    let status = Command::new("busctl")
        .args([
            "--user",
            "--expect-reply=false",
            "call",
            service,
            "/org/mpris/MediaPlayer2",
            "org.mpris.MediaPlayer2.Player",
            method,
        ])
        .status();

    match status {
        Ok(s) => s.success(),
        Err(_) => false,
    }
}

fn handle_volume_change(up: bool) {
    // 1. Try wpctl (PipeWire / WirePlumber)
    let step = if up { "5%+" } else { "5%-" };
    let mut cmd = Command::new("wpctl");
    if up {
        cmd.args(["set-volume", "-l", "1.5", "@DEFAULT_AUDIO_SINK@", step]);
    } else {
        cmd.args(["set-volume", "@DEFAULT_AUDIO_SINK@", step]);
    }
    if let Ok(status) = cmd.status() {
        if status.success() {
            return;
        }
    }

    // 2. Try pactl (PulseAudio / PipeWire Pulse)
    let pa_step = if up { "+5%" } else { "-5%" };
    if let Ok(status) = Command::new("pactl")
        .args(["set-sink-volume", "@DEFAULT_SINK@", pa_step])
        .status()
    {
        if status.success() {
            return;
        }
    }

    // 3. Try amixer (ALSA)
    let alsa_step = if up { "5%+" } else { "5%-" };
    if let Ok(status) = Command::new("amixer")
        .args(["-D", "pulse", "set", "Master", alsa_step])
        .status()
    {
        if status.success() {
            return;
        }
    }

    // 4. Fallback to xdotool
    let key = if up { "XF86AudioRaiseVolume" } else { "XF86AudioLowerVolume" };
    let _ = Command::new("xdotool").arg("key").arg(key).status();
}

fn handle_mute_toggle() {
    // 1. Try wpctl (PipeWire / WirePlumber)
    if let Ok(status) = Command::new("wpctl")
        .args(["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"])
        .status()
    {
        if status.success() {
            return;
        }
    }

    // 2. Try pactl (PulseAudio)
    if let Ok(status) = Command::new("pactl")
        .args(["set-sink-mute", "@DEFAULT_SINK@", "toggle"])
        .status()
    {
        if status.success() {
            return;
        }
    }

    // 3. Try amixer (ALSA)
    if let Ok(status) = Command::new("amixer")
        .args(["-D", "pulse", "set", "Master", "toggle"])
        .status()
    {
        if status.success() {
            return;
        }
    }

    // 4. Fallback to xdotool
    let _ = Command::new("xdotool").arg("key").arg("XF86AudioMute").status();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mpris_players_discovery() {
        let players = get_active_mpris_players();
        println!("Discovered MPRIS players: {:?}", players);
        for p in &players {
            let status = get_player_status(p);
            println!("Player: {} -> Status: {:?}", p, status);
        }
    }
}


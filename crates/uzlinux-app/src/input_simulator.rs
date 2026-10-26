use log::{info, warn};
use enigo::{Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings, Axis};

/// Parses and executes input simulation sequences compatible with OpenDeck Starter Pack.
/// Syntax examples:
/// - `[t("hello world!")]`
/// - `[k(ctrl,p),k(uni('c')),k(ctrl,r)]`
/// - `[m(10,10,r),b(l)]`
/// - `[s(5)]`
pub fn execute_simulation(script: &str) {
    let script = script.trim();
    if script.is_empty() {
        return;
    }

    info!("Executing input simulation: {}", script);

    // Initialize Enigo instance
    let mut enigo = match Enigo::new(&Settings::default()) {
        Ok(e) => e,
        Err(err) => {
            warn!("Failed to initialize Enigo (trying shell fallback): {}", err);
            shell_fallback(script);
            return;
        }
    };

    let inner = if script.starts_with('[') && script.ends_with(']') {
        &script[1..script.len() - 1]
    } else {
        script
    };

    for item in split_commands(inner) {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }

        if let Err(e) = execute_single_command(&mut enigo, item) {
            warn!("Error executing simulation command '{}': {}", item, e);
        }
    }
}

fn split_commands(s: &str) -> Vec<String> {
    let mut commands = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut paren_depth: i32 = 0;

    for c in s.chars() {
        match c {
            '"' => {
                in_string = !in_string;
                current.push(c);
            }
            '(' if !in_string => {
                paren_depth += 1;
                current.push(c);
            }
            ')' if !in_string => {
                paren_depth = paren_depth.saturating_sub(1);
                current.push(c);
            }
            ',' if !in_string && paren_depth == 0 => {
                if !current.trim().is_empty() {
                    commands.push(current.trim().to_string());
                }
                current.clear();
            }
            _ => current.push(c),
        }
    }
    if !current.trim().is_empty() {
        commands.push(current.trim().to_string());
    }
    commands
}

fn execute_single_command(enigo: &mut Enigo, cmd: &str) -> anyhow::Result<()> {
    if let Some(rest) = cmd.strip_prefix("t(") {
        // Text entry: t("hello world")
        if let Some(end) = rest.strip_suffix(')') {
            let text = parse_quoted_string(end);
            let _ = enigo.text(&text);
            return Ok(());
        }
    } else if let Some(rest) = cmd.strip_prefix("k(") {
        // Key action: k(ctrl, p), k(ctrl, r), k(uni('a')), k(enter)
        if let Some(args_str) = rest.strip_suffix(')') {
            let parts: Vec<&str> = args_str.split(',').map(|s| s.trim()).collect();
            let key_str = parts.first().copied().unwrap_or("");
            let dir_str = parts.get(1).copied().unwrap_or("c");

            let direction = match dir_str.to_lowercase().as_str() {
                "p" | "press" | "pressed" => Direction::Press,
                "r" | "release" => Direction::Release,
                _ => Direction::Click,
            };

            if let Some(key) = parse_key(key_str) {
                let _ = enigo.key(key, direction);
            }
            return Ok(());
        }
    } else if let Some(rest) = cmd.strip_prefix("b(") {
        // Mouse button: b(l), b(r), b(m)
        if let Some(args_str) = rest.strip_suffix(')') {
            let parts: Vec<&str> = args_str.split(',').map(|s| s.trim()).collect();
            let btn_str = parts.first().copied().unwrap_or("l");
            let dir_str = parts.get(1).copied().unwrap_or("c");

            let button = match btn_str.to_lowercase().as_str() {
                "r" | "right" => Button::Right,
                "m" | "middle" => Button::Middle,
                _ => Button::Left,
            };

            let direction = match dir_str.to_lowercase().as_str() {
                "p" | "press" | "pressed" => Direction::Press,
                "r" | "release" => Direction::Release,
                _ => Direction::Click,
            };

            let _ = enigo.button(button, direction);
            return Ok(());
        }
    } else if let Some(rest) = cmd.strip_prefix("m(") {
        // Mouse move: m(x, y, r) or m(x, y, a)
        if let Some(args_str) = rest.strip_suffix(')') {
            let parts: Vec<&str> = args_str.split(',').map(|s| s.trim()).collect();
            let x: i32 = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
            let y: i32 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
            let coord_str = parts.get(2).copied().unwrap_or("r");

            let coord = if coord_str.eq_ignore_ascii_case("a") || coord_str.eq_ignore_ascii_case("abs") {
                Coordinate::Abs
            } else {
                Coordinate::Rel
            };

            let _ = enigo.move_mouse(x, y, coord);
            return Ok(());
        }
    } else if let Some(rest) = cmd.strip_prefix("s(") {
        // Scroll: s(5) or s(5, v) or s(5, h)
        if let Some(args_str) = rest.strip_suffix(')') {
            let parts: Vec<&str> = args_str.split(',').map(|s| s.trim()).collect();
            let amount: i32 = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
            let axis_str = parts.get(1).copied().unwrap_or("v");

            let axis = if axis_str.eq_ignore_ascii_case("h") || axis_str.eq_ignore_ascii_case("horizontal") {
                Axis::Horizontal
            } else {
                Axis::Vertical
            };

            let _ = enigo.scroll(amount, axis);
            return Ok(());
        }
    } else {
        // Plain string fallback
        let _ = enigo.text(cmd);
    }

    Ok(())
}

fn parse_quoted_string(s: &str) -> String {
    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

fn parse_key(s: &str) -> Option<Key> {
    let s = s.trim();

    if let Some(rest) = s.strip_prefix("uni(") {
        if let Some(c_str) = rest.strip_suffix(')') {
            let clean = c_str.trim().trim_matches('\'').trim_matches('"');
            if let Some(ch) = clean.chars().next() {
                return Some(Key::Unicode(ch));
            }
        }
    }

    let key = match s.to_lowercase().as_str() {
        "ctrl" | "control" => Key::Control,
        "alt" => Key::Alt,
        "shift" => Key::Shift,
        "super" | "win" | "cmd" | "command" => Key::Meta,
        "return" | "enter" => Key::Return,
        "backspace" => Key::Backspace,
        "escape" | "esc" => Key::Escape,
        "tab" => Key::Tab,
        "space" => Key::Space,
        "up" | "arrowup" => Key::UpArrow,
        "down" | "arrowdown" => Key::DownArrow,
        "left" | "arrowleft" => Key::LeftArrow,
        "right" | "arrowright" => Key::RightArrow,
        "delete" => Key::Delete,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" => Key::PageUp,
        "pagedown" => Key::PageDown,
        "f1" => Key::F1,
        "f2" => Key::F2,
        "f3" => Key::F3,
        "f4" => Key::F4,
        "f5" => Key::F5,
        "f6" => Key::F6,
        "f7" => Key::F7,
        "f8" => Key::F8,
        "f9" => Key::F9,
        "f10" => Key::F10,
        "f11" => Key::F11,
        "f12" => Key::F12,
        other => {
            if other.len() == 1 {
                Key::Unicode(other.chars().next().unwrap())
            } else {
                return None;
            }
        }
    };

    Some(key)
}

fn shell_fallback(script: &str) {
    // If xdotool is available and on X11 / Xwayland
    let _ = std::process::Command::new("xdotool")
        .arg("type")
        .arg(script)
        .spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_commands() {
        let input = r#"t("hello, world!"), k(ctrl, p), k(uni('c')), k(ctrl, r), m(10, 20, r), b(l)"#;
        let cmds = split_commands(input);
        assert_eq!(cmds.len(), 6);
        assert_eq!(cmds[0], r#"t("hello, world!")"#);
        assert_eq!(cmds[1], "k(ctrl, p)");
        assert_eq!(cmds[2], "k(uni('c'))");
        assert_eq!(cmds[3], "k(ctrl, r)");
        assert_eq!(cmds[4], "m(10, 20, r)");
        assert_eq!(cmds[5], "b(l)");
    }

    #[test]
    fn test_parse_key() {
        assert_eq!(parse_key("ctrl"), Some(Key::Control));
        assert_eq!(parse_key("shift"), Some(Key::Shift));
        assert_eq!(parse_key("f5"), Some(Key::F5));
        assert_eq!(parse_key("enter"), Some(Key::Return));
        assert_eq!(parse_key("a"), Some(Key::Unicode('a')));
        assert_eq!(parse_key("uni('x')"), Some(Key::Unicode('x')));
    }
}

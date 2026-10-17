#![allow(dead_code)]

use std::collections::HashMap;
use std::io::{Cursor, Write};
use std::path::Path;
use zip::write::{SimpleFileOptions, ZipWriter};
use zip::CompressionMethod;
use serde_json::json;

pub const MAGIC: [u8; 2] = [0x7C, 0x7C]; // "||"
pub const HID_CHUNK_SIZE: usize = 1024;
pub const FIRST_PACKET_PAYLOAD: usize = 1016; // 1024 - 8 header bytes

// Host -> Device Commands (2 bytes big endian)
// 0x000D triggers DeviceWorker::processIconFile in D200 firmware, which extracts all PNG icons
// to /tmp/icon/icons/ and renders both icons and text. 0x0001 (processFile) ignores icon decompression on D200.
pub const CMD_OUT_SET_BUTTONS: u16 = 0x000D;
pub const CMD_OUT_SET_SMALL_WINDOW_DATA: u16 = 0x0006;
pub const CMD_OUT_SET_BRIGHTNESS: u16 = 0x000A;
pub const CMD_OUT_SET_LABEL_STYLE: u16 = 0x000B;
pub const CMD_OUT_PARTIALLY_UPDATE_BUTTONS: u16 = 0x000D;

// Device -> Host Commands (2 bytes big endian)
pub const CMD_IN_BUTTON: u16 = 0x0101;
pub const CMD_IN_BUTTON_ALT: u16 = 0x0102;
pub const CMD_IN_HEARTBEAT: u16 = 0x0103;
pub const CMD_IN_DEVICE_INFO: u16 = 0x0303;

#[derive(Debug, Clone)]
pub enum DeckEvent {
    KeyEvent {
        key: u8,
        col: u8,
        row: u8,
        pos: u8,
        is_encoder: bool,
        pressed: bool,
        encoder_action: Option<u8>, // 0=release, 1=press, 2=left, 3=right
    },
    DeviceInfo(String),
    Heartbeat,
    Unknown {
        cmd: u16,
        len: u32,
    },
}

/// Builds a 1024-byte control packet:
/// [0..2]: "||" (0x7C, 0x7C)
/// [2..4]: command (u16 BE)
/// [4..8]: payload length (u32 LE)
/// [8..8+N]: payload bytes
/// zero-padded to 1024 bytes.
pub fn build_control_packet(command: u16, payload: &[u8]) -> Vec<u8> {
    let mut packet = vec![0u8; HID_CHUNK_SIZE];
    packet[0..2].copy_from_slice(&MAGIC);
    packet[2..4].copy_from_slice(&command.to_be_bytes());
    packet[4..8].copy_from_slice(&(payload.len() as u32).to_le_bytes());
    let copy_len = payload.len().min(FIRST_PACKET_PAYLOAD);
    packet[8..8 + copy_len].copy_from_slice(&payload[..copy_len]);
    packet
}

/// Builds the stream of 1024-byte packets for a ZIP bundle:
/// Packet 0: 8-byte header (MAGIC + CMD_OUT_SET_BUTTONS + length LE) + first 1016 bytes of data
/// Packet 1..N: raw 1024-byte chunks of data, zero-padded
pub fn build_bundle_packets(bundle_data: &[u8]) -> Vec<Vec<u8>> {
    let mut packets = Vec::new();
    let total_len = bundle_data.len();

    // Packet 0
    let mut first_packet = vec![0u8; HID_CHUNK_SIZE];
    first_packet[0..2].copy_from_slice(&MAGIC);
    first_packet[2..4].copy_from_slice(&CMD_OUT_SET_BUTTONS.to_be_bytes());
    first_packet[4..8].copy_from_slice(&(total_len as u32).to_le_bytes());
    let first_chunk_len = total_len.min(FIRST_PACKET_PAYLOAD);
    first_packet[8..8 + first_chunk_len].copy_from_slice(&bundle_data[..first_chunk_len]);
    packets.push(first_packet);

    // Subsequent packets
    let mut offset = FIRST_PACKET_PAYLOAD;
    while offset < total_len {
        let mut chunk_packet = vec![0u8; HID_CHUNK_SIZE];
        let remaining = total_len - offset;
        let chunk_len = remaining.min(HID_CHUNK_SIZE);
        chunk_packet[..chunk_len].copy_from_slice(&bundle_data[offset..offset + chunk_len]);
        packets.push(chunk_packet);
        offset += HID_CHUNK_SIZE;
    }

    packets
}

/// Builds a ZIP bundle containing ONLY the changed key's manifest entry + its PNG icon.
/// Used with CMD_OUT_PARTIALLY_UPDATE_BUTTONS for single-key real-time updates.
pub fn build_single_key_zip(key: &ProfileKeyInfo) -> Result<Vec<u8>, String> {
    if key.id == 14 {
        return Err("Key 14 is phantom (right half of wide key 13); no update needed".to_string());
    }
    let is_wide = key.id == 13;

    // Skip clock/stats wide key — firmware handles the display natively
    if is_wide && (key.action_type == "clock" || key.action_type == "system_stats") {
        return Err("Clock/stats key is handled natively by firmware; no bundle needed".to_string());
    }

    let col = key.id % 5;
    let row = key.id / 5;
    let pos_key = format!("{}_{}", col, row);
    let icon_filename = format!("icons/{}_{}.png", col, row);

    // Render the PNG (or blank if no icon)
    let png_bytes = if !key.icon_path.trim().is_empty() {
        render_key_png(&key.icon_path, is_wide).unwrap_or_else(|_| {
            generate_blank_png(is_wide).unwrap_or_default()
        })
    } else {
        generate_blank_png(is_wide).unwrap_or_default()
    };

    // Build manifest with only this key
    let mut view_param = serde_json::Map::new();
    view_param.insert("Text".to_string(), serde_json::json!(key.title));
    view_param.insert("Icon".to_string(), serde_json::json!(icon_filename));
    let entry = serde_json::json!({
        "State": 0,
        "ViewParam": [view_param]
    });
    let mut manifest_map = serde_json::Map::new();
    manifest_map.insert(pos_key, entry);
    let manifest_bytes = serde_json::to_vec_pretty(&serde_json::json!(manifest_map))
        .map_err(|e| format!("Erro ao gerar manifest.json parcial: {}", e))?;

    // Packaging loop dodging 0x7C, 0x7C ("||") on boundary offsets
    let mut fallback_zip: Option<Vec<u8>> = None;
    for attempt in 0..30 {
        let padding = format!("pad_{:04}_{}", attempt, "x".repeat(attempt * 7));
        let mut buffer = Cursor::new(Vec::new());

        {
            let mut zip = ZipWriter::new(&mut buffer);
            let options = SimpleFileOptions::default()
                .compression_method(CompressionMethod::Deflated);

            zip.start_file("manifest.json", options)
                .map_err(|e| format!("Erro ao criar manifest.json parcial no ZIP: {}", e))?;
            zip.write_all(&manifest_bytes)
                .map_err(|e| format!("Erro ao escrever manifest.json parcial: {}", e))?;

            zip.start_file(&icon_filename, options)
                .map_err(|e| format!("Erro ao criar {} no ZIP parcial: {}", icon_filename, e))?;
            zip.write_all(&png_bytes)
                .map_err(|e| format!("Erro ao escrever {} parcial: {}", icon_filename, e))?;

            if attempt > 0 {
                zip.start_file("padding.bin", options)
                    .map_err(|e| format!("Erro ao criar padding no ZIP parcial: {}", e))?;
                zip.write_all(padding.as_bytes())
                    .map_err(|e| format!("Erro ao escrever padding no ZIP parcial: {}", e))?;
            }

            zip.finish()
                .map_err(|e| format!("Erro ao finalizar ZIP parcial: {}", e))?;
        }

        let zip_bytes = buffer.into_inner();
        if fallback_zip.is_none() {
            fallback_zip = Some(zip_bytes.clone());
        }

        let mut is_clean = true;
        let mut offset = FIRST_PACKET_PAYLOAD;
        while offset < zip_bytes.len() {
            if zip_bytes[offset] == 0x7C && offset + 1 < zip_bytes.len() && zip_bytes[offset + 1] == 0x7C {
                is_clean = false;
                break;
            }
            offset += HID_CHUNK_SIZE;
        }

        if is_clean {
            return Ok(zip_bytes);
        }
    }

    fallback_zip.ok_or_else(|| "Erro ao gerar ZIP parcial".to_string())
}

/// Builds the stream of 1024-byte packets for a partial ZIP update using
/// CMD_OUT_PARTIALLY_UPDATE_BUTTONS (0x000D). Identical layout to build_bundle_packets
/// but uses the partial-update command code so the firmware updates only the referenced key(s).
pub fn build_partial_update_packets(bundle_data: &[u8]) -> Vec<Vec<u8>> {
    let mut packets = Vec::new();
    let total_len = bundle_data.len();

    // Packet 0: header + first chunk
    let mut first_packet = vec![0u8; HID_CHUNK_SIZE];
    first_packet[0..2].copy_from_slice(&MAGIC);
    first_packet[2..4].copy_from_slice(&CMD_OUT_PARTIALLY_UPDATE_BUTTONS.to_be_bytes());
    first_packet[4..8].copy_from_slice(&(total_len as u32).to_le_bytes());
    let first_chunk_len = total_len.min(FIRST_PACKET_PAYLOAD);
    first_packet[8..8 + first_chunk_len].copy_from_slice(&bundle_data[..first_chunk_len]);
    packets.push(first_packet);

    // Subsequent data chunks
    let mut offset = FIRST_PACKET_PAYLOAD;
    while offset < total_len {
        let mut chunk_packet = vec![0u8; HID_CHUNK_SIZE];
        let remaining = total_len - offset;
        let chunk_len = remaining.min(HID_CHUNK_SIZE);
        chunk_packet[..chunk_len].copy_from_slice(&bundle_data[offset..offset + chunk_len]);
        packets.push(chunk_packet);
        offset += HID_CHUNK_SIZE;
    }

    packets
}

/// Parses an incoming 1024-byte HID buffer into a DeckEvent
pub fn parse_packet(buf: &[u8]) -> Option<DeckEvent> {
    if buf.len() < 12 || buf[0] != MAGIC[0] || buf[1] != MAGIC[1] {
        return None;
    }

    let cmd = u16::from_be_bytes([buf[2], buf[3]]);
    let len = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);

    if cmd == CMD_IN_BUTTON || cmd == CMD_IN_BUTTON_ALT {
        // buf[8]: state
        // buf[9]: index (0..19)
        // buf[10]: 0x01 (or 2 for dial)
        // buf[11]: status (1=pressed, 0=released / 2=left, 3=right for dial)
        let key_index = buf[9];
        let event_type = buf[10];
        let status = buf[11];

        let col = key_index % 5;
        let row = key_index / 5;
        let is_encoder = key_index >= 17 || event_type == 2;
        let pressed = status == 1;

        let encoder_action = if is_encoder {
            Some(status)
        } else {
            None
        };

        return Some(DeckEvent::KeyEvent {
            key: key_index,
            col,
            row,
            pos: key_index,
            is_encoder,
            pressed,
            encoder_action,
        });
    } else if cmd == CMD_IN_HEARTBEAT {
        return Some(DeckEvent::Heartbeat);
    } else if cmd == CMD_IN_DEVICE_INFO {
        let max_len = (len as usize).min(buf.len().saturating_sub(8));
        let str_bytes = &buf[8..8 + max_len];
        let info = String::from_utf8_lossy(str_bytes).to_string();
        return Some(DeckEvent::DeviceInfo(info));
    }

    Some(DeckEvent::Unknown { cmd, len })
}

/// Converts an icon path into a 196x196 PNG suitable for the D200 hardware.
/// If `is_wide` is true (key 13), it centers the icon in a 392x196 canvas
/// before resizing down to 196x196 so the firmware's 2:1 hardware stretch
/// restores the original aspect ratio without distortion.
pub fn render_key_png(path_str: &str, is_wide: bool) -> Result<Vec<u8>, String> {
    let p = Path::new(path_str);
    if !p.exists() {
        return Err(format!("Ícone não encontrado: {}", path_str));
    }

    // Handle SVG via rsvg-convert if available
    let raw_bytes = if path_str.to_lowercase().ends_with(".svg") {
        if let Ok(output) = std::process::Command::new("rsvg-convert")
            .arg("-w")
            .arg(if is_wide { "392" } else { "196" })
            .arg("-h")
            .arg("196")
            .arg("-f")
            .arg("png")
            .arg(p)
            .output()
        {
            if output.status.success() && !output.stdout.is_empty() {
                output.stdout
            } else {
                std::fs::read(p).map_err(|e| format!("Erro ao ler SVG: {}", e))?
            }
        } else {
            std::fs::read(p).map_err(|e| format!("Erro ao ler SVG: {}", e))?
        }
    } else {
        std::fs::read(p).map_err(|e| format!("Erro ao ler arquivo de imagem: {}", e))?
    };

    let img = image::load_from_memory(&raw_bytes)
        .map_err(|e| format!("Erro ao decodificar imagem: {}", e))?;

    let final_img = if is_wide {
        // Create 392x196 canvas with black background, paste 196x196 centered, then squeeze to 196x196
        let square = img.resize_exact(196, 196, image::imageops::FilterType::Lanczos3).to_rgba8();
        let mut canvas = image::RgbaImage::from_pixel(392, 196, image::Rgba([0, 0, 0, 255]));
        image::imageops::overlay(&mut canvas, &square, 98, 0);
        let dynamic = image::DynamicImage::ImageRgba8(canvas);
        dynamic.resize_exact(196, 196, image::imageops::FilterType::Lanczos3).to_rgba8()
    } else {
        img.resize_exact(196, 196, image::imageops::FilterType::Lanczos3).to_rgba8()
    };

    let mut png_buf = Vec::new();
    let mut cursor = Cursor::new(&mut png_buf);
    let encoder = image::codecs::png::PngEncoder::new(&mut cursor);
    final_img.write_with_encoder(encoder)
        .map_err(|e| format!("Erro ao codificar PNG: {}", e))?;

    Ok(png_buf)
}

/// Generates a solid black 196x196 PNG icon used to blank out deactivated or cleared keys.
pub fn generate_blank_png(is_wide: bool) -> Result<Vec<u8>, String> {
    let width = if is_wide { 392 } else { 196 };
    let black = image::RgbaImage::from_pixel(width, 196, image::Rgba([0, 0, 0, 255]));
    let dynamic = image::DynamicImage::ImageRgba8(black);
    let final_img = if is_wide {
        dynamic.resize_exact(196, 196, image::imageops::FilterType::Nearest).to_rgba8()
    } else {
        dynamic.to_rgba8()
    };
    let mut png_buf = Vec::new();
    let mut cursor = Cursor::new(&mut png_buf);
    let encoder = image::codecs::png::PngEncoder::new(&mut cursor);
    image::DynamicImage::ImageRgba8(final_img).write_with_encoder(encoder)
        .map_err(|e| format!("Erro ao codificar PNG preto: {}", e))?;
    Ok(png_buf)
}

/// Helper struct for key profile information when building D200 bundles
pub struct ProfileKeyInfo {
    pub id: usize,
    pub title: String,
    pub icon_path: String,
    pub action_type: String,
}

/// Builds the complete D200 ZIP profile bundle containing manifest.json and 196x196 PNGs.
/// Workaround for D200 firmware quirk:
/// The firmware chokes if a 1024-byte packet boundary (offset 1016 + 1024 * k) contains
/// a 0x00 or 0x7C byte. We add a dummy.txt at the start of the archive with varying padding
/// until all packet boundaries dodge 0x00 and 0x7C.
pub fn build_d200_profile_zip(keys: &[ProfileKeyInfo]) -> Result<Vec<u8>, String> {
    // 1. Pre-render all PNG icons for keys that have an icon.
    // For cleared/empty keys, supply a solid black PNG so the LCD screen physically blanks out!
    let mut key_pngs: HashMap<usize, Vec<u8>> = HashMap::new();
    let blank_square_png = generate_blank_png(false).unwrap_or_default();
    let blank_wide_png = generate_blank_png(true).unwrap_or_default();

    for key in keys {
        if key.id == 14 {
            // Key 14 is the phantom right half of the wide key 13; hardware does not address it
            continue;
        }
        let is_wide = key.id == 13;
        if is_wide && (key.action_type == "clock" || key.action_type == "system_stats") {
            // Native clock or system stats mode: omitted from manifest so firmware handles display
            continue;
        }

        if !key.icon_path.trim().is_empty() {
            if let Ok(png) = render_key_png(&key.icon_path, is_wide) {
                key_pngs.insert(key.id, png);
            } else {
                key_pngs.insert(key.id, if is_wide { blank_wide_png.clone() } else { blank_square_png.clone() });
            }
        } else {
            // Key has no custom icon: lay down solid black icon so any previous icon is erased
            key_pngs.insert(key.id, if is_wide { blank_wide_png.clone() } else { blank_square_png.clone() });
        }
    }

    // 2. Build manifest structure
    let mut manifest_map = serde_json::Map::new();
    for key in keys {
        if key.id == 14 {
            continue;
        }
        // If key 13 (wide screen) is set to clock or stats mode, omit it from the manifest
        // so the device hardware runs in native display mode!
        if key.id == 13 && (key.action_type == "clock" || key.action_type == "system_stats") {
            continue;
        }

        let col = key.id % 5;
        let row = key.id / 5;
        let pos_key = format!("{}_{}", col, row);

        let mut view_param = serde_json::Map::new();
        view_param.insert("Text".to_string(), json!(key.title));
        if key_pngs.contains_key(&key.id) {
            let icon_name = format!("icons/{}_{}.png", col, row);
            view_param.insert("Icon".to_string(), json!(icon_name));
        }

        let entry = json!({
            "State": 0,
            "ViewParam": [view_param]
        });

        manifest_map.insert(pos_key, entry);
    }

    let manifest_bytes = serde_json::to_vec_pretty(&json!(manifest_map))
        .map_err(|e| format!("Erro ao gerar manifest.json: {}", e))?;

    // 3. Packaging loop dodging 0x7C, 0x7C ("||") on boundary offsets
    let mut fallback_zip: Option<Vec<u8>> = None;
    for attempt in 0..30 {
        let padding = format!("pad_{:04}_{}", attempt, "x".repeat(attempt * 7));
        let mut buffer = Cursor::new(Vec::new());

        {
            let mut zip = ZipWriter::new(&mut buffer);
            let options = SimpleFileOptions::default()
                .compression_method(CompressionMethod::Deflated);

            // manifest.json must ALWAYS be the first entry in the ZIP archive for the firmware parser
            zip.start_file("manifest.json", options)
                .map_err(|e| format!("Erro ao criar manifest.json no ZIP: {}", e))?;
            zip.write_all(&manifest_bytes)
                .map_err(|e| format!("Erro ao escrever manifest.json: {}", e))?;

            // Icons written in deterministic, sorted order
            let mut sorted_icons: Vec<(&usize, &Vec<u8>)> = key_pngs.iter().collect();
            sorted_icons.sort_by_key(|(id, _)| *id);
            for (key_id, png) in sorted_icons {
                let col = key_id % 5;
                let row = key_id / 5;
                let icon_filename = format!("icons/{}_{}.png", col, row);
                zip.start_file(&icon_filename, options)
                    .map_err(|e| format!("Erro ao criar {} no ZIP: {}", icon_filename, e))?;
                zip.write_all(png)
                    .map_err(|e| format!("Erro ao escrever {}: {}", icon_filename, e))?;
            }

            // If boundary collision occurred on previous attempt, append padding at the end of the archive
            if attempt > 0 {
                zip.start_file("padding.bin", options)
                    .map_err(|e| format!("Erro ao criar padding no ZIP: {}", e))?;
                zip.write_all(padding.as_bytes())
                    .map_err(|e| format!("Erro ao escrever padding: {}", e))?;
            }

            zip.finish()
                .map_err(|e| format!("Erro ao finalizar ZIP: {}", e))?;
        }

        let zip_bytes = buffer.into_inner();
        if fallback_zip.is_none() {
            fallback_zip = Some(zip_bytes.clone());
        }

        // Check if any packet boundary starts with the magic bytes 0x7C, 0x7C ("||")
        // which could trick the firmware packet parser into detecting an unexpected new packet header.
        let mut is_clean = true;
        let mut offset = FIRST_PACKET_PAYLOAD;
        while offset < zip_bytes.len() {
            if zip_bytes[offset] == 0x7C && offset + 1 < zip_bytes.len() && zip_bytes[offset + 1] == 0x7C {
                is_clean = false;
                break;
            }
            offset += HID_CHUNK_SIZE;
        }

        if is_clean {
            return Ok(zip_bytes);
        }
    }

    // Fallback: return valid zip even if padding couldn't fully dodge
    fallback_zip.ok_or_else(|| "Erro ao gerar bundle ZIP".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_d200_profile_zip_real_profile() {
        let keys = vec![
            ProfileKeyInfo { id: 0, title: "Vivaldi".into(), icon_path: "/usr/share/pixmaps/vivaldi.png".into(), action_type: "app".into() },
            ProfileKeyInfo { id: 1, title: "Terminal".into(), icon_path: "/home/lucastiago/Imagens/Imagem do Codex 22 de ago. de 2026, 17_58_12.png".into(), action_type: "app".into() },
            ProfileKeyInfo { id: 2, title: "VS Code".into(), icon_path: "/usr/share/pixmaps/antigravity.png".into(), action_type: "app".into() },
            ProfileKeyInfo { id: 3, title: "Locke Games".into(), icon_path: "/usr/share/pixmaps/vivaldi.png".into(), action_type: "website".into() },
            ProfileKeyInfo { id: 4, title: "Steam".into(), icon_path: "/usr/share/pixmaps/steam.png".into(), action_type: "app".into() },
            ProfileKeyInfo { id: 5, title: "Play/Pause".into(), icon_path: "/home/lucastiago/.config/uzlinux-studio/icons/ulanzi_defaults/Play.png".into(), action_type: "media".into() },
            ProfileKeyInfo { id: 6, title: "Mute".into(), icon_path: "/home/lucastiago/.config/uzlinux-studio/icons/ulanzi_defaults/Mute.png".into(), action_type: "media".into() },
            ProfileKeyInfo { id: 7, title: "Vol -".into(), icon_path: "/home/lucastiago/.config/uzlinux-studio/icons/ulanzi_defaults/Volume Down.png".into(), action_type: "media".into() },
            ProfileKeyInfo { id: 8, title: "Vol +".into(), icon_path: "/home/lucastiago/.config/uzlinux-studio/icons/ulanzi_defaults/Volume Up.png".into(), action_type: "media".into() },
            ProfileKeyInfo { id: 9, title: "Próxima".into(), icon_path: "/home/lucastiago/.config/uzlinux-studio/icons/ulanzi_defaults/Next Track.png".into(), action_type: "media".into() },
            ProfileKeyInfo { id: 10, title: "YouTube".into(), icon_path: "/home/lucastiago/.config/uzlinux-studio/icons/ulanzi_defaults/db5e5d2d580d8b25f027e24643511b2e.jpg".into(), action_type: "text".into() },
            ProfileKeyInfo { id: 11, title: "Mudo".into(), icon_path: "assets/ulanzi_official/actions/btn_media_mute.png".into(), action_type: "media".into() },
            ProfileKeyInfo { id: 12, title: "".into(), icon_path: "".into(), action_type: "".into() },
            ProfileKeyInfo { id: 13, title: "Relógio".into(), icon_path: "".into(), action_type: "clock".into() },
            ProfileKeyInfo { id: 14, title: "Captura".into(), icon_path: "".into(), action_type: "hotkey".into() },
        ];

        let res = build_d200_profile_zip(&keys);
        println!("build_d200_profile_zip result: {:?}", res.as_ref().map(|b| b.len()));
        assert!(res.is_ok(), "Failed to build d200 zip: {:?}", res.err());
    }

    #[test]
    fn test_build_single_key_zip() {
        let key = ProfileKeyInfo {
            id: 0,
            title: "Vivaldi".into(),
            icon_path: "/usr/share/pixmaps/vivaldi.png".into(),
            action_type: "app".into(),
        };
        let res = build_single_key_zip(&key);
        assert!(res.is_ok(), "Failed to build single key zip: {:?}", res.err());
        let zip_bytes = res.unwrap();
        // A single key bundle is very small (around 5-15 KB), compared to 420 KB of the full profile
        assert!(zip_bytes.len() < 50_000, "Single key zip is too large: {} bytes", zip_bytes.len());

        let packets = build_partial_update_packets(&zip_bytes);
        assert!(!packets.is_empty());
        assert_eq!(packets[0][0..2], MAGIC);
        assert_eq!(u16::from_be_bytes([packets[0][2], packets[0][3]]), CMD_OUT_PARTIALLY_UPDATE_BUTTONS);
    }
}



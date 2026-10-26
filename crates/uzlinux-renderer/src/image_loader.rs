use std::path::Path;
use anyhow::{Context, Result, anyhow};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use image::{DynamicImage, ImageReader};
use std::io::Cursor;

pub struct ImageLoader;

impl ImageLoader {
    /// Loads an image from a path or a base64 string (including data:image/... URIs).
    pub fn load_image(input: &str, base_search_dir: Option<&Path>) -> Result<DynamicImage> {
        let trimmed = input.trim();

        // 1. Check for data URI: "data:image/png;base64,..."
        if let Some(pos) = trimmed.find("base64,") {
            let b64_data = &trimmed[pos + 7..];
            let bytes = BASE64.decode(b64_data.trim())
                .context("Failed to decode base64 data URI")?;
            return Self::decode_bytes(&bytes);
        }

        // 2. Check if input is a file path
        let path = Path::new(trimmed);
        if path.is_file() {
            if let Some(img) = try_read_image(path) {
                return Ok(img);
            }
        }

        // Check relative to base search dir (e.g. plugins dir or config dir)
        if let Some(base_dir) = base_search_dir {
            let mut candidates = Vec::new();
            let norm = trimmed.trim_start_matches('/');

            // Handle "plugins/<uuid>/<rest>" mapping to "<base_dir>/plugins/<uuid>.sdPlugin/<rest>"
            let parts: Vec<&str> = norm.split('/').collect();
            if parts.len() >= 2 && parts[0] == "plugins" {
                let uuid = parts[1];
                let rest = parts[2..].join("/");
                candidates.push(base_dir.join("plugins").join(format!("{}.sdPlugin", uuid)).join(&rest));
                candidates.push(base_dir.join(format!("{}.sdPlugin", uuid)).join(&rest));
                candidates.push(base_dir.join("plugins").join(uuid).join(&rest));
            } else if parts.len() >= 1 {
                let uuid = parts[0];
                let rest = parts[1..].join("/");
                candidates.push(base_dir.join("plugins").join(format!("{}.sdPlugin", uuid)).join(&rest));
                candidates.push(base_dir.join(format!("{}.sdPlugin", uuid)).join(&rest));
            }

            candidates.push(base_dir.join(norm));
            candidates.push(base_dir.join("plugins").join(norm));

            for cand in candidates {
                if cand.is_file() {
                    if let Some(img) = try_read_image(&cand) {
                        return Ok(img);
                    }
                }
                for ext in &[".png", "@2x.png", ".svg", ".jpg", ".jpeg"] {
                    let cand_str = cand.to_string_lossy();
                    let custom_str = format!("{}{}", cand_str, ext);
                    let custom = Path::new(&custom_str);
                    if custom.is_file() {
                        if let Some(img) = try_read_image(custom) {
                            return Ok(img);
                        }
                    }
                }
            }
        }

        // 3. Attempt direct base64 decode if plausible
        if trimmed.len() > 100 && !trimmed.contains('\n') && !trimmed.contains(' ') {
            if let Ok(bytes) = BASE64.decode(trimmed) {
                if let Ok(img) = Self::decode_bytes(&bytes) {
                    return Ok(img);
                }
            }
        }

        Err(anyhow!("Could not load image from input: '{}'", trimmed))
    }

    pub fn decode_bytes(bytes: &[u8]) -> Result<DynamicImage> {
        if let Ok(reader) = ImageReader::new(Cursor::new(bytes)).with_guessed_format() {
            if let Ok(img) = reader.decode() {
                return Ok(img);
            }
        }

        // Try SVG decode using resvg / usvg
        let opt = resvg::usvg::Options::default();
        if let Ok(tree) = resvg::usvg::Tree::from_data(bytes, &opt) {
            let size = tree.size().to_int_size();
            let target_w = size.width().max(1);
            let target_h = size.height().max(1);

            let max_dim = 256.0f32;
            let current_max = (target_w.max(target_h) as f32).max(1.0);
            let scale = if current_max < 72.0 {
                (72.0 / current_max).max(1.0)
            } else if current_max > max_dim {
                max_dim / current_max
            } else {
                1.0
            };

            let out_w = ((target_w as f32) * scale).round().max(1.0) as u32;
            let out_h = ((target_h as f32) * scale).round().max(1.0) as u32;

            if let Some(mut pixmap) = resvg::tiny_skia::Pixmap::new(out_w, out_h) {
                let sx = out_w as f32 / tree.size().width();
                let sy = out_h as f32 / tree.size().height();
                let transform = resvg::tiny_skia::Transform::from_scale(sx, sy);
                resvg::render(&tree, transform, &mut pixmap.as_mut());

                if let Some(rgba) = image::RgbaImage::from_raw(out_w, out_h, pixmap.take()) {
                    return Ok(DynamicImage::ImageRgba8(rgba));
                }
            }
        }

        Err(anyhow!("Failed to decode image from bytes"))
    }
}

fn try_read_image(path: &Path) -> Option<DynamicImage> {
    if let Ok(bytes) = std::fs::read(path) {
        if let Ok(img) = ImageLoader::decode_bytes(&bytes) {
            return Some(img);
        }
    }
    None
}

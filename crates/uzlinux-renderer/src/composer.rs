use std::io::Cursor;
use std::path::Path;
use std::sync::OnceLock;
use anyhow::Result;
use ab_glyph::{point, Font, FontArc, PxScale, ScaleFont};
use image::{imageops, ImageFormat, Rgba, RgbaImage};
use uzlinux_core::models::StateDefinition;

use crate::color::parse_hex_color;
use crate::image_loader::ImageLoader;

static DEFAULT_FONT_BYTES: &[u8] = include_bytes!("../assets/font.ttf");
static DEFAULT_FONT: OnceLock<FontArc> = OnceLock::new();

fn get_default_font() -> &'static FontArc {
    DEFAULT_FONT.get_or_init(|| {
        FontArc::try_from_slice(DEFAULT_FONT_BYTES).expect("Failed to load embedded default font")
    })
}

pub struct ButtonComposer {
    target_width: u32,
    target_height: u32,
}

impl ButtonComposer {
    pub fn new(target_width: u32, target_height: u32) -> Self {
        Self {
            target_width,
            target_height,
        }
    }

    /// Composes a full button image from a StateDefinition.
    pub fn compose_state(
        &self,
        state: &StateDefinition,
        base_search_dir: Option<&Path>,
    ) -> Result<RgbaImage> {
        let bg_color = parse_hex_color(&state.background_colour);
        let mut canvas = RgbaImage::from_pixel(self.target_width, self.target_height, bg_color);

        // Load image (either image_override or image)
        let img_src = state.image_override.as_deref().unwrap_or(&state.image);
        if !img_src.is_empty() {
            match ImageLoader::load_image(img_src, base_search_dir) {
                Ok(dyn_img) => {
                    let scale = (state.image_scale as f32 / 100.0).clamp(0.1, 1.0);
                    let fit_w = ((self.target_width as f32) * scale).round() as u32;
                    let fit_h = ((self.target_height as f32) * scale).round() as u32;

                    let resized = dyn_img.resize(fit_w, fit_h, imageops::FilterType::Lanczos3);
                    let (rw, rh) = (resized.width(), resized.height());

                    // Center coordinates
                    let x = ((self.target_width as i64 - rw as i64) / 2).max(0) as i64;
                    let y = ((self.target_height as i64 - rh as i64) / 2).max(0) as i64;

                    imageops::overlay(&mut canvas, &resized.to_rgba8(), x, y);
                }
                Err(err) => {
                    log::warn!("Failed to load key image: {}", err);
                }
            }
        }

        // Draw title / text if visible
        let text = state.text_override.as_deref().unwrap_or(&state.text).trim();
        if state.show && !text.is_empty() {
            let text_color = parse_hex_color(&state.colour);
            let stroke_color = parse_hex_color(&state.stroke_colour);
            self.draw_text_on_canvas(
                &mut canvas,
                text,
                state.size as f32,
                text_color,
                &state.alignment,
                stroke_color,
                state.stroke_size,
            );
        }

        Ok(canvas)
    }

    fn draw_text_on_canvas(
        &self,
        canvas: &mut RgbaImage,
        text: &str,
        requested_size: f32,
        color: Rgba<u8>,
        alignment: &str,
        stroke_color: Rgba<u8>,
        stroke_size: u32,
    ) {
        if text.is_empty() {
            return;
        }

        let font = get_default_font();
        let target_w = self.target_width as f32;
        let target_h = self.target_height as f32;

        let mut font_size = if requested_size <= 0.0 { 14.0 } else { requested_size.clamp(8.0, 28.0) };

        // Measure text width and downscale if too wide
        let mut scale = PxScale::from(font_size);
        let mut scaled_font = font.as_scaled(scale);

        let mut total_width = calculate_text_width(&scaled_font, text);
        let max_width = target_w - 6.0;

        while total_width > max_width && font_size > 8.0 {
            font_size -= 1.0;
            scale = PxScale::from(font_size);
            scaled_font = font.as_scaled(scale);
            total_width = calculate_text_width(&scaled_font, text);
        }

        // Horizontal positioning: centered
        let start_x = ((target_w - total_width) / 2.0).max(2.0);

        // Vertical positioning based on alignment
        let ascent = scaled_font.ascent();
        let descent = scaled_font.descent(); // negative
        let baseline_y = match alignment.to_lowercase().as_str() {
            "top" => ascent + 2.0,
            "middle" | "center" => ((target_h + ascent + descent) / 2.0).round(),
            _ => target_h + descent - 2.0, // "bottom" (default)
        };

        // Layout glyphs
        let mut glyphs_to_draw = Vec::new();
        let mut cursor_x = start_x;
        for ch in text.chars() {
            let mut glyph = scaled_font.scaled_glyph(ch);
            glyph.position = point(cursor_x, baseline_y);
            let adv = scaled_font.h_advance(glyph.id);
            glyphs_to_draw.push(glyph);
            cursor_x += adv;
        }

        // Draw outline/stroke if enabled
        let stroke_radius = stroke_size.min(2) as i32;
        if stroke_radius > 0 && stroke_color[3] > 0 {
            for dy in -stroke_radius..=stroke_radius {
                for dx in -stroke_radius..=stroke_radius {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    if dx * dx + dy * dy <= stroke_radius * stroke_radius + 1 {
                        for glyph in &glyphs_to_draw {
                            if let Some(outlined) = font.outline_glyph(glyph.clone()) {
                                let bounds = outlined.px_bounds();
                                outlined.draw(|gx, gy, coverage| {
                                    if coverage > 0.05 {
                                        let px = (bounds.min.x as i32) + dx + (gx as i32);
                                        let py = (bounds.min.y as i32) + dy + (gy as i32);
                                        if px >= 0 && px < self.target_width as i32 && py >= 0 && py < self.target_height as i32 {
                                            blend_pixel(canvas, px as u32, py as u32, stroke_color, coverage);
                                        }
                                    }
                                });
                            }
                        }
                    }
                }
            }
        }

        // Draw main text glyphs
        for glyph in glyphs_to_draw {
            if let Some(outlined) = font.outline_glyph(glyph) {
                let bounds = outlined.px_bounds();
                outlined.draw(|gx, gy, coverage| {
                    if coverage > 0.05 {
                        let px = (bounds.min.x as i32) + (gx as i32);
                        let py = (bounds.min.y as i32) + (gy as i32);
                        if px >= 0 && px < self.target_width as i32 && py >= 0 && py < self.target_height as i32 {
                            blend_pixel(canvas, px as u32, py as u32, color, coverage);
                        }
                    }
                });
            }
        }
    }

    /// Renders an alert indicator (warning triangle) over the image.
    pub fn overlay_alert(&self, canvas: &mut RgbaImage) {
        let amber = Rgba([245, 158, 11, 230]);
        let white = Rgba([255, 255, 255, 255]);

        let cx = (self.target_width / 2) as i32;
        let cy = (self.target_height / 2) as i32;
        let r = ((self.target_width.min(self.target_height) as f32) * 0.35) as i32;

        // Draw diamond / triangle indicator
        for y in (cy - r)..=(cy + r) {
            for x in (cx - r)..=(cx + r) {
                let dx = (x - cx).abs();
                let dy = (y - cy).abs();
                if dx + dy <= r {
                    if x >= 0 && x < self.target_width as i32 && y >= 0 && y < self.target_height as i32 {
                        canvas.put_pixel(x as u32, y as u32, amber);
                    }
                }
            }
        }

        // Draw exclamation line
        let ex_top = cy - r / 2;
        let ex_bot = cy + r / 4;
        for y in ex_top..ex_bot {
            for dx in -1..=1 {
                let px = (cx + dx) as u32;
                let py = y as u32;
                if px < self.target_width && py < self.target_height {
                    canvas.put_pixel(px, py, white);
                }
            }
        }
        // Exclamation dot
        let dot_y = cy + r / 2;
        for dx in -1..=1 {
            for dy in -1..=1 {
                let px = (cx + dx) as u32;
                let py = (dot_y + dy) as u32;
                if px < self.target_width && py < self.target_height {
                    canvas.put_pixel(px, py, white);
                }
            }
        }
    }

    /// Renders a success indicator (green circle + checkmark) over the image.
    pub fn overlay_ok(&self, canvas: &mut RgbaImage) {
        let green = Rgba([16, 185, 129, 230]);
        let white = Rgba([255, 255, 255, 255]);

        let cx = (self.target_width / 2) as i32;
        let cy = (self.target_height / 2) as i32;
        let r = ((self.target_width.min(self.target_height) as f32) * 0.35) as i32;
        let r_sq = r * r;

        // Draw circle
        for y in (cy - r)..=(cy + r) {
            for x in (cx - r)..=(cx + r) {
                let dx = x - cx;
                let dy = y - cy;
                if dx * dx + dy * dy <= r_sq {
                    if x >= 0 && x < self.target_width as i32 && y >= 0 && y < self.target_height as i32 {
                        canvas.put_pixel(x as u32, y as u32, green);
                    }
                }
            }
        }

        // Draw checkmark lines
        for step in 0..=r {
            // Short line
            let x1 = cx - r / 3 + step / 3;
            let y1 = cy + step / 3;
            if x1 >= 0 && x1 < self.target_width as i32 && y1 >= 0 && y1 < self.target_height as i32 {
                canvas.put_pixel(x1 as u32, y1 as u32, white);
            }
            // Long line
            let x2 = cx - r / 6 + step / 2;
            let y2 = cy + r / 4 - step / 2;
            if x2 >= 0 && x2 < self.target_width as i32 && y2 >= 0 && y2 < self.target_height as i32 {
                canvas.put_pixel(x2 as u32, y2 as u32, white);
            }
        }
    }

    /// Encodes an RgbaImage to PNG format bytes.
    pub fn to_png_bytes(image: &RgbaImage) -> Result<Vec<u8>> {
        let mut buffer = Cursor::new(Vec::new());
        image.write_to(&mut buffer, ImageFormat::Png)?;
        Ok(buffer.into_inner())
    }
}

fn calculate_text_width<F: Font>(scaled_font: &impl ScaleFont<F>, text: &str) -> f32 {
    text.chars().map(|ch| {
        let g = scaled_font.scaled_glyph(ch);
        scaled_font.h_advance(g.id)
    }).sum()
}

fn blend_pixel(canvas: &mut RgbaImage, x: u32, y: u32, color: Rgba<u8>, coverage: f32) {
    let base = canvas.get_pixel_mut(x, y);
    let alpha = (color[3] as f32 / 255.0) * coverage;
    let inv_alpha = 1.0 - alpha;

    base[0] = (color[0] as f32 * alpha + base[0] as f32 * inv_alpha).round() as u8;
    base[1] = (color[1] as f32 * alpha + base[1] as f32 * inv_alpha).round() as u8;
    base[2] = (color[2] as f32 * alpha + base[2] as f32 * inv_alpha).round() as u8;
    base[3] = (255.0 * (alpha + (base[3] as f32 / 255.0) * inv_alpha)).min(255.0).round() as u8;
}

pub mod color;
pub mod image_loader;
pub mod composer;
pub mod cache;

pub use color::parse_hex_color;
pub use image_loader::ImageLoader;
pub use composer::ButtonComposer;
pub use cache::ImageCache;

#[cfg(test)]
mod tests {
    use super::*;
    use uzlinux_core::models::StateDefinition;

    #[test]
    fn test_parse_hex_colors() {
        let white = parse_hex_color("#FFFFFF");
        assert_eq!(white, image::Rgba([255, 255, 255, 255]));

        let black = parse_hex_color("#000000");
        assert_eq!(black, image::Rgba([0, 0, 0, 255]));

        let trans_red = parse_hex_color("#FF000080");
        assert_eq!(trans_red, image::Rgba([255, 0, 0, 128]));
    }

    #[test]
    fn test_compose_button_and_overlays() {
        let composer = ButtonComposer::new(72, 72);
        let mut state = StateDefinition::default();
        state.background_colour = "#1E1E2E".to_string();

        let mut canvas = composer.compose_state(&state, None).expect("Should compose button");
        assert_eq!(canvas.width(), 72);
        assert_eq!(canvas.height(), 72);

        // Test alert overlay
        composer.overlay_alert(&mut canvas);
        let png = ButtonComposer::to_png_bytes(&canvas).expect("Should encode to PNG");
        assert!(png.starts_with(&[0x89, b'P', b'N', b'G']));

        // Test ok overlay
        composer.overlay_ok(&mut canvas);
        let png_ok = ButtonComposer::to_png_bytes(&canvas).expect("Should encode to PNG");
        assert!(png_ok.starts_with(&[0x89, b'P', b'N', b'G']));
    }

    #[test]
    fn test_render_button_title() {
        let composer = ButtonComposer::new(72, 72);
        let mut state = StateDefinition::default();
        state.background_colour = "#000000".to_string();
        state.text = "Ulanzi D200".to_string();
        state.colour = "#FFFFFF".to_string();
        state.show = true;
        state.alignment = "bottom".to_string();

        let canvas = composer.compose_state(&state, None).expect("Should compose button with text");
        // Count non-black pixels
        let non_bg_count = canvas.pixels().filter(|p| **p != image::Rgba([0, 0, 0, 255])).count();
        assert!(non_bg_count > 20, "Text should render visible pixels on canvas");
    }

    #[test]
    fn test_image_loader_data_uri_png_and_svg() {
        use base64::engine::general_purpose::STANDARD as BASE64;
        use base64::Engine;

        // 1. Test SVG data URI
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><rect width="64" height="64" fill="#FF5500"/></svg>"##;
        let svg_b64 = BASE64.encode(svg.as_bytes());
        let svg_uri = format!("data:image/svg+xml;base64,{}", svg_b64);
        let svg_img = ImageLoader::load_image(&svg_uri, None).expect("Should decode SVG data URI");
        assert_eq!(svg_img.width(), 72);
        assert_eq!(svg_img.height(), 72);

        // 2. Test PNG data URI
        let small_png = image::RgbaImage::from_pixel(32, 32, image::Rgba([0, 255, 128, 255]));
        let mut png_bytes = Vec::new();
        small_png.write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png).unwrap();
        let png_b64 = BASE64.encode(&png_bytes);
        let png_uri = format!("data:image/png;base64,{}", png_b64);
        let png_img = ImageLoader::load_image(&png_uri, None).expect("Should decode PNG data URI");
        assert_eq!(png_img.width(), 32);
        assert_eq!(png_img.height(), 32);
    }
}

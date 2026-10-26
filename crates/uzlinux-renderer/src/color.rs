use image::Rgba;

/// Parses a hex color string (e.g., "#FFFFFF", "#000000", "#FF000080") into an `Rgba<u8>`.
pub fn parse_hex_color(hex: &str) -> Rgba<u8> {
    let clean = hex.trim_start_matches('#');
    let len = clean.len();

    if len == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&clean[0..2], 16),
            u8::from_str_radix(&clean[2..4], 16),
            u8::from_str_radix(&clean[4..6], 16),
        ) {
            return Rgba([r, g, b, 255]);
        }
    } else if len == 8 {
        if let (Ok(r), Ok(g), Ok(b), Ok(a)) = (
            u8::from_str_radix(&clean[0..2], 16),
            u8::from_str_radix(&clean[2..4], 16),
            u8::from_str_radix(&clean[4..6], 16),
            u8::from_str_radix(&clean[6..8], 16),
        ) {
            return Rgba([r, g, b, a]);
        }
    } else if len == 3 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&clean[0..1], 16),
            u8::from_str_radix(&clean[1..2], 16),
            u8::from_str_radix(&clean[2..3], 16),
        ) {
            return Rgba([r * 17, g * 17, b * 17, 255]);
        }
    }

    Rgba([0, 0, 0, 255])
}

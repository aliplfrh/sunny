use qrcode::{EcLevel, QrCode};

/// Encode locally; keep the input exact so the scanned text matches what was typed.
pub fn encode(text: &str) -> Result<Vec<Vec<bool>>, String> {
    if text.trim().is_empty() {
        return Err("Type text or a URL to generate a QR code.".to_string());
    }
    if text.len() > 512 {
        return Err("QR content is too long (maximum 512 UTF-8 bytes).".to_string());
    }
    let code = QrCode::with_error_correction_level(text.as_bytes(), EcLevel::M)
        .map_err(|e| format!("Could not generate QR code: {e}"))?;
    let size = code.width();
    let colors = code.to_colors();
    Ok(colors
        .chunks(size)
        .map(|row| {
            row.iter()
                .map(|color| *color == qrcode::Color::Dark)
                .collect()
        })
        .collect())
}

/// Build a crisp black/white RGBA image, including a four-module quiet zone.
pub fn rgba(modules: &[Vec<bool>]) -> (usize, Vec<u8>) {
    const SCALE: usize = 4;
    const QUIET: usize = 4;
    let side = (modules.len() + QUIET * 2) * SCALE;
    let mut pixels = vec![255; side * side * 4];
    for (y, row) in modules.iter().enumerate() {
        for (x, &dark) in row.iter().enumerate() {
            if !dark {
                continue;
            }
            for py in (y + QUIET) * SCALE..(y + QUIET + 1) * SCALE {
                for px in (x + QUIET) * SCALE..(x + QUIET + 1) * SCALE {
                    let offset = (py * side + px) * 4;
                    pixels[offset..offset + 3].fill(0);
                }
            }
        }
    }
    (side, pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_text_as_square_modules_with_finder_patterns() {
        let modules = encode("https://example.com").expect("valid QR");
        let size = modules.len();
        assert!(size >= 21);
        assert_eq!(size % 4, 1);
        assert!(modules.iter().all(|row| row.len() == size));
        assert!(modules[0][0]);
        assert!(modules[6][6]);
        assert!(!modules[7][7]);
    }

    #[test]
    fn rejects_empty_and_oversized_input() {
        assert!(encode("   ").is_err());
        assert!(encode(&"🙂".repeat(129)).is_err());
        assert!(encode("a").is_ok());
    }

    #[test]
    fn image_is_opaque_black_on_white_with_quiet_zone() {
        let modules = encode("Sunny").expect("valid QR");
        let (side, pixels) = rgba(&modules);
        assert_eq!(side, (modules.len() + 8) * 4);
        assert_eq!(pixels.len(), side * side * 4);
        assert_eq!(&pixels[..4], &[255, 255, 255, 255]);
        let pixel = ((16 * side) + 16) * 4;
        assert_eq!(&pixels[pixel..pixel + 4], &[0, 0, 0, 255]);
    }
}

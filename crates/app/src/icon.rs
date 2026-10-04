//! The app's icon, drawn rather than shipped: a disc in the accent colour with a play
//! triangle. One description serves the window, the taskbar and the tray
//! at whatever size each asks for.

use crate::theme;

/// Samples per pixel along each axis, for smooth edges.
const SAMPLES: u32 = 4;

/// The icon as `size` by `size` RGBA pixels.
pub fn rgba(size: u32) -> Vec<u8> {
    let disc = theme::DARK.accent;
    let glyph = theme::DARK.on_accent;
    let mut pixels = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            // How much of this pixel the disc covers, and the triangle.
            let (mut in_disc, mut in_glyph) = (0u32, 0u32);
            for sample in 0..SAMPLES * SAMPLES {
                let offset = |n: u32| (n as f32 + 0.5) / SAMPLES as f32;
                // In units where the icon spans -1 to 1.
                let px = (x as f32 + offset(sample % SAMPLES)) / size as f32 * 2.0 - 1.0;
                let py = (y as f32 + offset(sample / SAMPLES)) / size as f32 * 2.0 - 1.0;
                if px * px + py * py <= 1.0 {
                    in_disc += 1;
                    if in_play_triangle(px, py) {
                        in_glyph += 1;
                    }
                }
            }
            let total = (SAMPLES * SAMPLES) as f32;
            let glyph_share = in_glyph as f32 / in_disc.max(1) as f32;
            let mix =
                |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * glyph_share) as u8;
            pixels.extend([
                mix(disc.r(), glyph.r()),
                mix(disc.g(), glyph.g()),
                mix(disc.b(), glyph.b()),
                (in_disc as f32 / total * 255.0) as u8,
            ]);
        }
    }
    pixels
}

/// A triangle pointing right, nudged right of centre so it looks centred.
fn in_play_triangle(x: f32, y: f32) -> bool {
    let (left, right, half_height) = (-0.28, 0.46, 0.44);
    if x < left || x > right {
        return false;
    }
    // The edges close in on the tip.
    let reach = half_height * (right - x) / (right - left);
    y.abs() <= reach
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(pixels: &[u8], size: u32, x: u32, y: u32) -> [u8; 4] {
        let at = ((y * size + x) * 4) as usize;
        [pixels[at], pixels[at + 1], pixels[at + 2], pixels[at + 3]]
    }

    #[test]
    fn the_corners_are_clear_and_the_middle_is_the_glyph() {
        let size = 32;
        let pixels = rgba(size);
        assert_eq!(pixels.len(), (size * size * 4) as usize);
        assert_eq!(pixel(&pixels, size, 0, 0)[3], 0);
        let glyph = theme::DARK.on_accent;
        assert_eq!(
            pixel(&pixels, size, 16, 16),
            [glyph.r(), glyph.g(), glyph.b(), 255]
        );
        // Left of the triangle, inside the disc: the accent.
        let disc = theme::DARK.accent;
        assert_eq!(
            pixel(&pixels, size, 5, 16),
            [disc.r(), disc.g(), disc.b(), 255]
        );
    }
}

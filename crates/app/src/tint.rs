//! The colour a cover lends to the page behind it.
//!
//! A cover's own colours vary too much to sit behind white text, so what is
//! taken from it is its hue: the average of its most colourful pixels, set
//! to one fixed darkness that text stays readable on.

use eframe::egui::{Color32, ColorImage};

/// How dark every tint is made, whatever the cover.
const LIGHTNESS: f32 = 0.30;
/// The most colourful a tint gets; beyond this a page shouts.
const MOST_SATURATED: f32 = 0.6;
/// A cover greyer than this has no hue worth borrowing.
const LEAST_SATURATED: f32 = 0.08;
/// Pixels looked at along each axis. A cover's colour does not need more.
const SAMPLES: usize = 24;

/// The tint for `image`, or `None` for a cover with no colour to speak of.
pub fn of(image: &ColorImage) -> Option<Color32> {
    let [width, height] = image.size;
    if width == 0 || height == 0 {
        return None;
    }
    // Averaged as a vector, since hue is an angle: red at both ends of the
    // scale must average to red, not to cyan.
    let (mut x, mut y, mut saturation_sum, mut weight_sum) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    for row in 0..SAMPLES {
        for column in 0..SAMPLES {
            let pixel = image.pixels[(row * height / SAMPLES) * width + column * width / SAMPLES];
            let (hue, saturation, _) = to_hsl(pixel);
            // The more colourful a pixel, the more it counts.
            let weight = saturation * saturation;
            x += hue.to_radians().cos() * weight;
            y += hue.to_radians().sin() * weight;
            saturation_sum += saturation * weight;
            weight_sum += weight;
        }
    }
    if weight_sum <= f32::EPSILON {
        return None;
    }
    let saturation = saturation_sum / weight_sum;
    if saturation < LEAST_SATURATED {
        return None;
    }
    let hue = y.atan2(x).to_degrees().rem_euclid(360.0);
    Some(from_hsl(hue, saturation.min(MOST_SATURATED), LIGHTNESS))
}

/// `a` moved towards `b` by `amount`, from 0 (all `a`) to 1 (all `b`).
pub fn blend(a: Color32, b: Color32, amount: f32) -> Color32 {
    let mix = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * amount).round() as u8;
    Color32::from_rgb(mix(a.r(), b.r()), mix(a.g(), b.g()), mix(a.b(), b.b()))
}

/// Hue in degrees, saturation and lightness from 0 to 1.
fn to_hsl(color: Color32) -> (f32, f32, f32) {
    let [r, g, b] = [color.r(), color.g(), color.b()].map(|channel| f32::from(channel) / 255.0);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let lightness = (max + min) / 2.0;
    let chroma = max - min;
    if chroma <= f32::EPSILON {
        return (0.0, 0.0, lightness);
    }
    let saturation = chroma / (1.0 - (2.0 * lightness - 1.0).abs());
    let hue = if max == r {
        ((g - b) / chroma).rem_euclid(6.0)
    } else if max == g {
        (b - r) / chroma + 2.0
    } else {
        (r - g) / chroma + 4.0
    } * 60.0;
    (hue, saturation, lightness)
}

fn from_hsl(hue: f32, saturation: f32, lightness: f32) -> Color32 {
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let sector = hue / 60.0;
    let second = chroma * (1.0 - (sector.rem_euclid(2.0) - 1.0).abs());
    let (r, g, b) = match sector as u32 {
        0 => (chroma, second, 0.0),
        1 => (second, chroma, 0.0),
        2 => (0.0, chroma, second),
        3 => (0.0, second, chroma),
        4 => (second, 0.0, chroma),
        _ => (chroma, 0.0, second),
    };
    let lift = lightness - chroma / 2.0;
    let channel = |value: f32| ((value + lift) * 255.0).round() as u8;
    Color32::from_rgb(channel(r), channel(g), channel(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(color: Color32) -> ColorImage {
        ColorImage::filled([32, 32], color)
    }

    #[test]
    fn a_red_cover_gives_a_dark_red() {
        let tint = of(&flat(Color32::from_rgb(230, 30, 30))).expect("a tint");
        assert!(
            tint.r() > tint.g() * 2 && tint.r() > tint.b() * 2,
            "{tint:?}"
        );
        let (_, _, lightness) = to_hsl(tint);
        assert!((lightness - LIGHTNESS).abs() < 0.02);
    }

    #[test]
    fn a_grey_cover_gives_none() {
        assert_eq!(of(&flat(Color32::from_gray(120))), None);
        assert_eq!(of(&flat(Color32::BLACK)), None);
    }

    #[test]
    fn the_colourful_part_of_a_cover_decides() {
        // Mostly grey, with a blue band.
        let mut image = flat(Color32::from_gray(90));
        for pixel in image.pixels.iter_mut().take(32 * 8) {
            *pixel = Color32::from_rgb(20, 40, 220);
        }
        let tint = of(&image).expect("a tint");
        assert!(tint.b() > tint.r() && tint.b() > tint.g(), "{tint:?}");
    }

    #[test]
    fn hsl_goes_there_and_back() {
        for color in [
            Color32::from_rgb(200, 60, 20),
            Color32::from_rgb(20, 200, 90),
            Color32::from_rgb(40, 60, 210),
        ] {
            let (hue, saturation, lightness) = to_hsl(color);
            let back = from_hsl(hue, saturation, lightness);
            for (a, b) in [
                (color.r(), back.r()),
                (color.g(), back.g()),
                (color.b(), back.b()),
            ] {
                assert!(a.abs_diff(b) <= 1, "{color:?} became {back:?}");
            }
        }
    }

    #[test]
    fn blending_moves_between_the_two() {
        let (black, white) = (Color32::BLACK, Color32::WHITE);
        assert_eq!(blend(black, white, 0.0), black);
        assert_eq!(blend(black, white, 1.0), white);
        assert_eq!(blend(black, white, 0.5), Color32::from_gray(128));
    }
}

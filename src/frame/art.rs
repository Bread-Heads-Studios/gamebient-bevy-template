//! Frame art: the game's own `assets/marquee.png` and `assets/bezel.png`
//! when present, otherwise the shared fallback embedded in the binary. The
//! bezel's saturation is reduced once, here, by rewriting its pixels; no
//! shader is involved.

use bevy::asset::RenderAssetUsages;
use bevy::asset::io::file::FileAssetReader;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

/// Bezel saturation multiplier, applied at all times (Contract E).
pub const BEZEL_SATURATION: f32 = 0.6;
/// Alpha of the sweep band at its centre.
pub const SWEEP_PEAK_ALPHA: f32 = 0.22;
/// Width of the sweep texture in pixels. It is stretched, so it only needs
/// enough samples for a smooth ramp.
const SWEEP_TEXTURE_WIDTH: u32 = 64;

const FALLBACK_BEZEL: &[u8] = include_bytes!("art/fallback-bezel.png");
const FALLBACK_MARQUEE: &[u8] = include_bytes!("art/fallback-marquee.png");

// The CSS `saturate()` luma weights, so the website's filter and this
// function give the same picture.
const LUMA_R: f32 = 0.213;
const LUMA_G: f32 = 0.715;
const LUMA_B: f32 = 0.072;

/// Multiplies the saturation of RGBA8 pixels in place. Works on the encoded
/// (sRGB) values, as the CSS filter does. Alpha is left alone.
pub fn desaturate_rgba8(data: &mut [u8], saturation: f32) {
    for px in data.chunks_exact_mut(4) {
        let rgb = [f32::from(px[0]), f32::from(px[1]), f32::from(px[2])];
        let luma = LUMA_R * rgb[0] + LUMA_G * rgb[1] + LUMA_B * rgb[2];
        for (channel, value) in rgb.into_iter().enumerate() {
            px[channel] = (luma + saturation * (value - luma))
                .round()
                .clamp(0.0, 255.0) as u8;
        }
    }
}

/// One row of RGBA8 pixels: white, transparent at both ends, rising in a
/// straight line to `SWEEP_PEAK_ALPHA` in the middle.
pub fn sweep_pixels(width: u32) -> Vec<u8> {
    let last = (width.max(2) - 1) as f32;
    (0..width)
        .flat_map(|x| {
            let t = x as f32 / last;
            let envelope = 1.0 - (2.0 * t - 1.0).abs();
            let alpha = (envelope * SWEEP_PEAK_ALPHA * 255.0).round() as u8;
            [255, 255, 255, alpha]
        })
        .collect()
}

/// Decodes a PNG to RGBA8 sRGB with linear filtering. `None` if the bytes
/// are not a PNG or cannot be converted.
pub fn decode_png(bytes: &[u8]) -> Option<Image> {
    let image = Image::from_buffer(
        bytes,
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        true,
        ImageSampler::linear(),
        RenderAssetUsages::default(),
    )
    .ok()?;
    if image.texture_descriptor.format == TextureFormat::Rgba8UnormSrgb {
        return Some(image);
    }
    let mut converted = image.convert(TextureFormat::Rgba8UnormSrgb)?;
    converted.sampler = ImageSampler::linear();
    Some(converted)
}

/// Largest side of a game's own art, in pixels.
const MAX_ART_SIDE: u32 = 4096;

/// Why a game's own art of this size cannot be used, if it cannot.
pub fn art_problem(width: u32, height: u32, must_be_square: bool) -> Option<String> {
    if width > MAX_ART_SIDE || height > MAX_ART_SIDE {
        return Some(format!(
            "is {width}x{height}, a side is larger than {MAX_ART_SIDE}"
        ));
    }
    if must_be_square && width != height {
        return Some(format!("is {width}x{height}, not square"));
    }
    None
}

/// The game's own file from `assets/`, or the embedded fallback. The flag
/// is true when the fallback was used. A file that is present but unusable
/// is reported with a warning and replaced by the fallback.
fn read_art(file: &str, fallback: &'static [u8], must_be_square: bool) -> (Image, bool) {
    let path = FileAssetReader::get_base_path().join("assets").join(file);
    match std::fs::read(&path) {
        Ok(bytes) => match decode_png(&bytes) {
            Some(image) => match art_problem(image.width(), image.height(), must_be_square) {
                None => return (image, false),
                Some(problem) => warn!("frame: {} {problem}; using the fallback", path.display()),
            },
            None => warn!(
                "frame: {} is not a PNG, or cannot be converted to 8-bit RGBA; using the fallback",
                path.display()
            ),
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => warn!(
            "frame: could not read {}: {error}; using the fallback",
            path.display()
        ),
    }
    (
        decode_png(fallback).expect("embedded fallback frame art decodes"),
        true,
    )
}

/// Handles to the three frame textures.
#[derive(Resource)]
pub struct FrameArt {
    pub bezel: Handle<Image>,
    pub marquee: Handle<Image>,
    pub sweep: Handle<Image>,
    /// True when the shared marquee is in use, which has no title on it.
    pub marquee_is_fallback: bool,
}

/// `Startup`: read the art, bake the bezel's saturation, build the sweep.
pub fn load_frame_art(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let (mut bezel, _) = read_art("bezel.png", FALLBACK_BEZEL, true);
    if let Some(data) = bezel.data.as_mut() {
        desaturate_rgba8(data, BEZEL_SATURATION);
    }
    let (marquee, marquee_is_fallback) = read_art("marquee.png", FALLBACK_MARQUEE, false);
    let mut sweep = Image::new(
        Extent3d {
            width: SWEEP_TEXTURE_WIDTH,
            height: 1,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        sweep_pixels(SWEEP_TEXTURE_WIDTH),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    sweep.sampler = ImageSampler::linear();
    commands.insert_resource(FrameArt {
        bezel: images.add(bezel),
        marquee: images.add(marquee),
        sweep: images.add(sweep),
        marquee_is_fallback,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saturation_0_6_matches_the_css_saturate_matrix() {
        // Pure red, opaque. CSS saturate(0.6): r' = (0.213 + 0.787 * 0.6) * 255 = 174.7,
        // g' = b' = (0.213 - 0.213 * 0.6) * 255 = 21.7.
        let mut px = [255, 0, 0, 255];
        desaturate_rgba8(&mut px, 0.6);
        assert_eq!(px, [175, 22, 22, 255]);

        // Pure green: luma 0.715 * 255 = 182.3; g' = 182.3 + 0.6 * 72.7 = 225.9,
        // r' = b' = 182.3 * 0.4 = 72.9.
        let mut px = [0, 255, 0, 255];
        desaturate_rgba8(&mut px, 0.6);
        assert_eq!(px, [73, 226, 73, 255]);
    }

    #[test]
    fn grey_and_alpha_are_untouched() {
        let mut px = [128, 128, 128, 77, 0, 0, 0, 0, 255, 255, 255, 255];
        desaturate_rgba8(&mut px, 0.6);
        assert_eq!(px, [128, 128, 128, 77, 0, 0, 0, 0, 255, 255, 255, 255]);
    }

    #[test]
    fn saturation_1_is_the_identity_and_0_is_grey() {
        let mut px = [200, 100, 50, 255];
        desaturate_rgba8(&mut px, 1.0);
        assert_eq!(px, [200, 100, 50, 255]);
        desaturate_rgba8(&mut px, 0.0);
        // luma = 0.213 * 200 + 0.715 * 100 + 0.072 * 50 = 117.7
        assert_eq!(px, [118, 118, 118, 255]);
    }

    #[test]
    fn desaturation_never_raises_the_brightest_channel() {
        for r in (0..=255).step_by(51) {
            for g in (0..=255).step_by(51) {
                for b in (0..=255).step_by(51) {
                    let mut px = [r as u8, g as u8, b as u8, 255];
                    desaturate_rgba8(&mut px, BEZEL_SATURATION);
                    let before = r.max(g).max(b);
                    let after = i32::from(px[0]).max(i32::from(px[1])).max(i32::from(px[2]));
                    assert!(after <= before, "{r},{g},{b} -> {px:?}");
                }
            }
        }
    }

    #[test]
    fn sweep_is_white_transparent_at_the_edges_and_peaks_in_the_middle() {
        let px = sweep_pixels(65);
        assert_eq!(px.len(), 65 * 4);
        assert_eq!(&px[0..4], &[255, 255, 255, 0]);
        assert_eq!(&px[64 * 4..], &[255, 255, 255, 0]);
        // 0.22 * 255 = 56.1
        assert_eq!(&px[32 * 4..33 * 4], &[255, 255, 255, 56]);
    }

    #[test]
    fn embedded_fallback_art_decodes_at_the_expected_sizes() {
        let bezel = decode_png(FALLBACK_BEZEL).expect("fallback bezel decodes");
        assert_eq!(
            bezel.texture_descriptor.format,
            TextureFormat::Rgba8UnormSrgb
        );
        assert_eq!(
            (bezel.width(), bezel.height()),
            (960, 960),
            "fallback bezel"
        );
        let marquee = decode_png(FALLBACK_MARQUEE).expect("fallback marquee decodes");
        assert_eq!(
            (marquee.width(), marquee.height()),
            (1080, 360),
            "fallback marquee"
        );
    }

    /// Contract E, art geometry: the bezel's centre is one flat colour. At
    /// 960 x 960 the centre is the 540 x 540 from (210, 210).
    #[test]
    fn the_fallback_bezel_has_a_flat_centre() {
        let bezel = decode_png(FALLBACK_BEZEL).expect("fallback bezel decodes");
        let data = bezel.data.as_ref().expect("decoded pixels");
        let pixel = |x: usize, y: usize| &data[(y * 960 + x) * 4..(y * 960 + x) * 4 + 4];
        let first = pixel(210, 210);
        for y in 210..750 {
            for x in 210..750 {
                assert_eq!(pixel(x, y), first, "pixel {x},{y}");
            }
        }
    }

    #[test]
    fn art_size_problems_are_named() {
        assert_eq!(art_problem(1080, 360, false), None);
        assert_eq!(art_problem(1920, 1920, true), None);
        assert_eq!(art_problem(4096, 4096, true), None);
        assert!(art_problem(1920, 1080, true).unwrap().contains("square"));
        assert!(art_problem(4097, 4097, true).unwrap().contains("4096"));
        assert!(art_problem(1080, 5000, false).unwrap().contains("4096"));
    }

    #[test]
    fn a_damaged_file_does_not_decode() {
        assert!(decode_png(b"not a png").is_none());
    }
}

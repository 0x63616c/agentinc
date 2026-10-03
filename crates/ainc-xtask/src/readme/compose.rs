//! Place a real GPUI Pilot app render on a vivid gradient.
//!
//! The layout follows the open-source Tokokino screenshot composer's background
//! and padding approach (https://github.com/ShivaBhattacharjee/Tokokino). The
//! macOS controls and corner mask come from a repository native-window capture;
//! no extra title bar is added above the actual app pixels.
//! Usage: `cargo xtask readme-compose RAW.png OUTPUT.png`
use crate::release::{parse_args, resolve, usage_error};
use anyhow::{Result, bail};
use image::{
    GrayImage, ImageEncoder, Luma, Rgba, RgbaImage,
    codecs::png::{CompressionType, FilterType, PngEncoder},
};
use std::{
    fs,
    path::{Path, PathBuf},
};

const CANVAS: (u32, u32) = (3200, 2070);
const APP_SIZE: (u32, u32) = (2720, 1656);
const MAX_BYTES: u64 = 1_000_000;
const STOPS: [(f64, [u8; 3]); 3] = [
    (0.0, [255, 214, 92]),
    (0.46, [255, 104, 130]),
    (1.0, [181, 71, 233]),
];
const NATIVE_REFERENCE: &str = "crates/ainc-mac/docs/verification/native-capture-library.png";
/// Traffic-light region at the configured 18 pt window offset: x, y, width, height.
const CONTROLS: (u32, u32, u32, u32) = (26, 30, 137, 41);
const EDGE: u32 = 80;
const SHADOW_OFFSET: u32 = 34;
const SHADOW_BLUR: f32 = 52.0;
const SHADOW_OPACITY: f64 = 0.48;

/// Python's `round()`: ties to even.
fn round(value: f64) -> u8 {
    value.round_ties_even() as u8
}

/// A vertical gradient through `STOPS`, one color per row.
fn background(canvas: (u32, u32)) -> RgbaImage {
    let mut image = RgbaImage::new(canvas.0, canvas.1);
    for y in 0..canvas.1 {
        let position = y as f64 / (canvas.1 - 1) as f64;
        for pair in STOPS.windows(2) {
            let ((start, first), (end, last)) = (pair[0], pair[1]);
            if position <= end {
                let fraction = (position - start) / (end - start);
                let mut color = [0, 0, 0, 255];
                for channel in 0..3 {
                    let (a, b) = (first[channel] as f64, last[channel] as f64);
                    color[channel] = round(a + (b - a) * fraction);
                }
                for x in 0..canvas.0 {
                    image.put_pixel(x, y, Rgba(color));
                }
                break;
            }
        }
    }
    image
}

/// Pillow's box length for one of `passes` box blurs approximating a Gaussian of `radius`.
fn box_radius(radius: f32, passes: f32) -> f32 {
    let sigma2 = radius * radius / passes;
    let length = (12.0 * sigma2 + 1.0).sqrt();
    let l = ((length - 1.0) / 2.0).floor();
    let a =
        (2.0 * l + 1.0) * (l * (l + 1.0) - 3.0 * sigma2) / (6.0 * (sigma2 - (l + 1.0) * (l + 1.0)));
    l + a
}

/// One box blur of one row or column, with Pillow's fractional edge weights; reads beyond the
/// ends repeat the end pixel, and the result is rounded back to 8 bits.
fn box_line(line: &[u8], radius: f32) -> Vec<u8> {
    let whole = radius.floor() as i64;
    let fraction = radius - whole as f32;
    let weight = 1.0 / (2.0 * radius + 1.0);
    let last = line.len() as i64 - 1;
    let at = |i: i64| line[i.clamp(0, last) as usize] as f32;
    let mut sum: f32 = (-whole..=whole).map(&at).sum();
    let mut out = Vec::with_capacity(line.len());
    for i in 0..=last {
        let edges = at(i - whole - 1) + at(i + whole + 1);
        out.push(
            ((sum + edges * fraction) * weight + 0.5)
                .floor()
                .clamp(0.0, 255.0) as u8,
        );
        sum += at(i + whole + 1) - at(i - whole);
    }
    out
}

/// `ImageFilter.GaussianBlur(radius)`: three horizontal then three vertical box blurs.
fn gaussian_blur(mask: &GrayImage, radius: f32) -> GrayImage {
    let box_size = box_radius(radius, 3.0);
    let (width, height) = mask.dimensions();
    let mut data = mask.as_raw().clone();
    for _ in 0..3 {
        for row in data.chunks_mut(width as usize) {
            let blurred = box_line(row, box_size);
            row.copy_from_slice(&blurred);
        }
    }
    for _ in 0..3 {
        for x in 0..width as usize {
            let column: Vec<u8> = (0..height as usize)
                .map(|y| data[y * width as usize + x])
                .collect();
            for (y, value) in box_line(&column, box_size).into_iter().enumerate() {
                data[y * width as usize + x] = value;
            }
        }
    }
    GrayImage::from_raw(width, height, data).expect("same dimensions")
}

/// Pillow's `Image.paste(source, position)` for a same-mode image: copy, clipped to the target.
fn paste<P: image::Pixel>(
    target: &mut image::ImageBuffer<P, Vec<P::Subpixel>>,
    source: &image::ImageBuffer<P, Vec<P::Subpixel>>,
    (x, y): (u32, u32),
) {
    for (sx, sy, pixel) in source.enumerate_pixels() {
        if x + sx < target.width() && y + sy < target.height() {
            target.put_pixel(x + sx, y + sy, *pixel);
        }
    }
}

/// Pillow's `Image.crop`: pixels outside the source are transparent black.
fn crop(source: &RgbaImage, (x, y, width, height): (u32, u32, u32, u32)) -> RgbaImage {
    RgbaImage::from_fn(width, height, |cx, cy| {
        if x + cx < source.width() && y + cy < source.height() {
            *source.get_pixel(x + cx, y + cy)
        } else {
            Rgba([0; 4])
        }
    })
}

fn div255(value: u32) -> u32 {
    ((value >> 8) + value) >> 8
}

/// Pillow's `alpha_composite` of one non-premultiplied pixel, fixed point and all.
fn over(dst: Rgba<u8>, src: Rgba<u8>) -> Rgba<u8> {
    const BITS: u32 = 7;
    if src[3] == 0 {
        return dst;
    }
    let (src_a, dst_a) = (src[3] as u32, dst[3] as u32);
    if dst_a == 0 {
        return src;
    }
    let out_a255 = src_a * 255 + dst_a * (255 - src_a);
    let coef1 = src_a * 255 * 255 * (1 << BITS) / out_a255;
    let coef2 = 255 * (1 << BITS) - coef1;
    let mut out = [0u8; 4];
    for channel in 0..3 {
        let mixed = src[channel] as u32 * coef1 + dst[channel] as u32 * coef2;
        out[channel] = (div255(mixed + (0x80 << BITS)) >> BITS) as u8;
    }
    out[3] = div255(out_a255 + 0x80) as u8;
    Rgba(out)
}

fn alpha_composite(dst: &mut RgbaImage, src: &RgbaImage, (x, y): (u32, u32)) {
    for (sx, sy, pixel) in src.enumerate_pixels() {
        if x + sx < dst.width() && y + sy < dst.height() {
            let target = dst.get_pixel_mut(x + sx, y + sy);
            *target = over(*target, *pixel);
        }
    }
}

/// The geometry the real run uses; tests pass a smaller one.
struct Layout {
    canvas: (u32, u32),
    app: (u32, u32),
    controls: (u32, u32, u32, u32),
    probe: (u32, u32),
    padding: u32,
    edge: u32,
    offset: u32,
    blur: f32,
}

const LAYOUT: Layout = Layout {
    canvas: CANVAS,
    app: APP_SIZE,
    controls: CONTROLS,
    probe: (20, 20),
    padding: 200,
    edge: EDGE,
    offset: SHADOW_OFFSET,
    blur: SHADOW_BLUR,
};

fn compose(app: &RgbaImage, native: &RgbaImage, layout: &Layout) -> Result<RgbaImage> {
    let mut app = app.clone();
    if app.dimensions() != layout.app {
        bail!(
            "Expected a {}×{} retina Pilot render, got ({}, {})",
            layout.app.0,
            layout.app.1,
            app.width(),
            app.height()
        );
    }
    if app.width() > layout.canvas.0 - layout.padding
        || app.height() > layout.canvas.1 - layout.padding
    {
        bail!(
            "Capture lacks padding on ({}, {}): ({}, {})",
            layout.canvas.0,
            layout.canvas.1,
            app.width(),
            app.height()
        );
    }
    if native.width() != app.width() {
        bail!("Native reference width differs from Pilot render");
    }
    // The app and reference use the same #0c0c0c top bar. Copy only the actual
    // macOS traffic-light region at the configured 18 pt window offset.
    let controls = layout.controls;
    if app.get_pixel(layout.probe.0, layout.probe.1)
        != native.get_pixel(layout.probe.0, layout.probe.1)
    {
        bail!("Native control background no longer matches app header");
    }
    paste(&mut app, &crop(native, controls), (controls.0, controls.1));
    // Reuse the alpha of a real native window at the same retina width, with
    // its exact top/bottom corner contours. The middle rows remain opaque.
    let mut mask = GrayImage::from_pixel(app.width(), app.height(), Luma([255]));
    let alpha = GrayImage::from_fn(native.width(), native.height(), |x, y| {
        Luma([native.get_pixel(x, y)[3]])
    });
    let band = |top: u32| {
        GrayImage::from_fn(app.width(), layout.edge, |x, y| {
            *alpha.get_pixel(x, top + y)
        })
    };
    paste(&mut mask, &band(0), (0, 0));
    paste(
        &mut mask,
        &band(native.height() - layout.edge),
        (0, app.height() - layout.edge),
    );
    for (x, y, pixel) in mask.enumerate_pixels() {
        app.get_pixel_mut(x, y)[3] = pixel[0];
    }
    let mut image = background(layout.canvas);
    let placement = (
        (layout.canvas.0 - app.width()) / 2,
        (layout.canvas.1 - app.height()) / 2,
    );
    let mut shadow_mask = GrayImage::new(layout.canvas.0, layout.canvas.1);
    paste(
        &mut shadow_mask,
        &mask,
        (placement.0, placement.1 + layout.offset),
    );
    let blurred = gaussian_blur(&shadow_mask, layout.blur);
    let shadow = RgbaImage::from_fn(layout.canvas.0, layout.canvas.1, |x, y| {
        Rgba([
            0,
            0,
            0,
            round(blurred.get_pixel(x, y)[0] as f64 * SHADOW_OPACITY),
        ])
    });
    alpha_composite(&mut image, &shadow, (0, 0));
    alpha_composite(&mut image, &app, placement);
    Ok(image)
}

/// `image.convert("RGB").save(format="PNG", optimize=True, compress_level=9)`.
fn encode(image: &RgbaImage) -> Result<Vec<u8>> {
    let rgb = image::DynamicImage::ImageRgba8(image.clone()).to_rgb8();
    let mut bytes = Vec::new();
    PngEncoder::new_with_quality(&mut bytes, CompressionType::Best, FilterType::Adaptive)
        .write_image(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            image::ExtendedColorType::Rgb8,
        )?;
    Ok(bytes)
}

fn run(raw: &Path, output: &Path, reference: &Path, layout: &Layout) -> Result<()> {
    let app = image::open(raw)?.to_rgba8();
    let native = image::open(reference)?.to_rgba8();
    let image = compose(&app, &native, layout)?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = encode(&image)?;
    fs::write(output, &bytes)?;
    let size = bytes.len() as u64;
    if size >= MAX_BYTES {
        fs::remove_file(output)?;
        bail!(
            "Hero PNG is {} bytes (limit {})",
            thousands(size),
            thousands(MAX_BYTES)
        );
    }
    println!("{}: {} bytes", output.display(), thousands(size));
    Ok(())
}

/// Python's `{n:,}`.
fn thousands(number: u64) -> String {
    let digits = number.to_string();
    let mut out = String::new();
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

pub fn cli(root: &Path, args: &[String]) -> Result<()> {
    let opts = parse_args("readme-compose", args, &[], &[]);
    let [raw, output] = opts.positional.as_slice() else {
        usage_error(
            "readme-compose",
            "expected exactly two arguments: raw output",
        );
    };
    let reference: PathBuf = resolve(root).join(NATIVE_REFERENCE);
    run(Path::new(raw), Path::new(output), &reference, &LAYOUT)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: Rgba<u8> = Rgba([12, 12, 12, 255]);

    fn small() -> Layout {
        Layout {
            canvas: (60, 50),
            app: (40, 20),
            controls: (4, 4, 10, 6),
            probe: (20, 10),
            padding: 4,
            edge: 4,
            offset: 2,
            blur: 3.0,
        }
    }

    /// A native reference with transparent corners and red controls on the header color.
    fn native() -> RgbaImage {
        RgbaImage::from_fn(40, 24, |x, y| {
            if (x < 2 && y < 2) || (x >= 38 && y >= 22) {
                Rgba([0, 0, 0, 0])
            } else if (4..14).contains(&x) && (4..10).contains(&y) {
                Rgba([255, 0, 0, 255])
            } else {
                HEADER
            }
        })
    }

    fn app() -> RgbaImage {
        RgbaImage::from_pixel(40, 20, HEADER)
    }

    #[test]
    fn gradient_hits_its_stops_and_rounds_like_python() {
        let image = background((4, 101));
        assert_eq!(image.get_pixel(0, 0).0, [255, 214, 92, 255]);
        assert_eq!(image.get_pixel(3, 100).0, [181, 71, 233, 255]);
        // y = 46 of 100 is exactly the middle stop.
        assert_eq!(image.get_pixel(1, 46).0, [255, 104, 130, 255]);
        assert_eq!(round(0.5), 0);
        assert_eq!(round(1.5), 2);
    }

    #[test]
    fn blur_keeps_flat_areas_and_spreads_an_impulse_symmetrically() {
        let flat = GrayImage::from_pixel(20, 20, Luma([200]));
        assert!(gaussian_blur(&flat, 3.0).pixels().all(|p| p[0] == 200));
        let mut dot = GrayImage::new(41, 41);
        dot.put_pixel(20, 20, Luma([255]));
        let blurred = gaussian_blur(&dot, 3.0);
        let center = blurred.get_pixel(20, 20)[0];
        assert!(center > 0 && center < 255);
        assert_eq!(blurred.get_pixel(18, 20), blurred.get_pixel(22, 20));
        assert_eq!(blurred.get_pixel(20, 17), blurred.get_pixel(20, 23));
        assert!(blurred.get_pixel(20, 18)[0] < center);
    }

    #[test]
    fn alpha_composite_matches_pillow_fixed_point() {
        let dst = Rgba([10, 20, 30, 255]);
        assert_eq!(over(dst, Rgba([200, 100, 50, 0])), dst);
        assert_eq!(
            over(dst, Rgba([200, 100, 50, 255])),
            Rgba([200, 100, 50, 255])
        );
        assert_eq!(over(dst, Rgba([0, 0, 0, 128])), Rgba([5, 10, 15, 255]));
    }

    #[test]
    fn composes_controls_corner_mask_and_shadow() {
        let layout = small();
        let image = compose(&app(), &native(), &layout).unwrap();
        assert_eq!(image.dimensions(), (60, 50));
        let (left, top) = (10, 15);
        // Controls come from the native reference, the rest of the app is untouched.
        assert_eq!(image.get_pixel(left + 5, top + 5).0, [255, 0, 0, 255]);
        assert_eq!(image.get_pixel(left + 20, top + 10), &HEADER);
        // The transparent native corner shows the gradient (plus a faint shadow) instead of the app.
        assert_ne!(image.get_pixel(left, top), &HEADER);
        // A shadow darkens the canvas just below the window.
        assert!(
            image.get_pixel(30, top + 20 + 1)[0] < background(layout.canvas).get_pixel(30, 36)[0]
        );
        // Far from the window it is the pure gradient.
        assert_eq!(
            image.get_pixel(0, 0),
            background(layout.canvas).get_pixel(0, 0)
        );
    }

    #[test]
    fn rejects_bad_inputs_with_the_python_messages() {
        let layout = small();
        let wrong = RgbaImage::new(30, 20);
        let message = compose(&wrong, &native(), &layout).unwrap_err().to_string();
        assert_eq!(
            message,
            "Expected a 40×20 retina Pilot render, got (30, 20)"
        );
        let narrow = RgbaImage::from_pixel(39, 24, HEADER);
        assert_eq!(
            compose(&app(), &narrow, &layout).unwrap_err().to_string(),
            "Native reference width differs from Pilot render"
        );
        let mut off = native();
        off.put_pixel(20, 10, Rgba([1, 2, 3, 255]));
        assert_eq!(
            compose(&app(), &off, &layout).unwrap_err().to_string(),
            "Native control background no longer matches app header"
        );
        let crowded = Layout {
            canvas: (41, 100),
            ..small()
        };
        assert!(
            compose(&app(), &native(), &crowded)
                .unwrap_err()
                .to_string()
                .starts_with("Capture lacks padding")
        );
    }

    #[test]
    fn writes_a_png_and_reports_its_size() {
        let dir = tempfile::tempdir().unwrap();
        let (raw, reference, output) = (
            dir.path().join("raw.png"),
            dir.path().join("native.png"),
            dir.path().join("nested/out.png"),
        );
        app().save(&raw).unwrap();
        native().save(&reference).unwrap();
        run(&raw, &output, &reference, &small()).unwrap();
        let written = image::open(&output).unwrap();
        assert_eq!((written.width(), written.height()), (60, 50));
        assert_eq!(thousands(1_234_567), "1,234,567");
        assert_eq!(thousands(999), "999");
    }
}

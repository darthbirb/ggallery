//! WebP thumbnails, lossy q78, 320px longest edge.
//!
//! Settled by measurement in M0: AVIF encoded 41x slower for a 12% size win
//! and would not decode at all through the `image` crate on this platform. See
//! docs/ENGINEERING-NOTES.md before reopening that.

use std::path::Path;

use image::imageops::FilterType;
use image::DynamicImage;

use crate::db::items::ItemFile;
use crate::error::{AppError, Result};
use crate::fs::paths::LibraryPaths;
use crate::media::Kind;
use crate::sidecar::Tools;

pub const EDGE: u32 = 320;
pub const QUALITY: f32 = 78.0;

/// Generates the thumbnail and reports the source dimensions when it learned
/// them for certain — decoding the full image is the only place they are known
/// for free, which is what lets an item probed by a version that could not
/// read the file heal itself without a second pass over the disk.
pub fn generate(paths: &LibraryPaths, item: &ItemFile, tools: &Tools) -> Result<Option<(i64, i64)>> {
    let src = paths.item_path(&item.uuid, &item.ext);
    let out = paths.thumb_path(&item.uuid);

    let (image, source_size) = match Kind::parse(&item.kind) {
        // Decoder chosen by content — see `media::open_image_reader`.
        Kind::Image => {
            let decoded = crate::media::open_image_reader(&src)?
                .decode()
                .map_err(|e| AppError::media(e.to_string()))?;
            let size = (decoded.width() as i64, decoded.height() as i64);
            (decoded, Some(size))
        }
        // A poster frame is already scaled, so it says nothing trustworthy
        // about the size of the video it came from.
        Kind::Video => (poster_frame(&src, item.duration_ms, tools)?, None),
        Kind::Other => {
            return Err(AppError::media("no thumbnail for this file type"));
        }
    };

    write_webp(&fit(image, EDGE), &out, QUALITY)?;
    Ok(source_size)
}

/// A frame from 10% in — far enough past the fades and black leaders that open
/// most clips, early enough to still be the shot the file is about.
fn poster_frame(src: &Path, duration_ms: Option<i64>, tools: &Tools) -> Result<DynamicImage> {
    let ffmpeg = tools
        .ffmpeg
        .as_ref()
        .ok_or(AppError::ToolMissing("ffmpeg"))?;

    let at = duration_ms
        .map(|ms| (ms as f64 / 1000.0 * 0.1).clamp(0.0, 600.0))
        .unwrap_or(0.0);

    let png = ffmpeg.frame(src, at, EDGE)?;
    image::load_from_memory(&png).map_err(|e| AppError::media(e.to_string()))
}

/// Downscale to fit `edge` on the longest side, preserving aspect.
///
/// Two steps when the source is much larger: a fast box pass to roughly twice
/// the target, then a filtered pass to the exact size. One Lanczos pass
/// straight from a 6000px original costs more than the encode does.
pub fn fit(image: DynamicImage, edge: u32) -> DynamicImage {
    let (w, h) = (image.width(), image.height());
    if w <= edge && h <= edge {
        return image;
    }

    let scale = edge as f64 / w.max(h) as f64;
    let tw = ((w as f64 * scale).round() as u32).max(1);
    let th = ((h as f64 * scale).round() as u32).max(1);

    let image = if w / tw >= 4 {
        image.thumbnail(tw * 2, th * 2)
    } else {
        image
    };
    image.resize_exact(tw, th, FilterType::Triangle)
}

pub fn write_webp(image: &DynamicImage, out: &Path, quality: f32) -> Result<()> {
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let rgb = image.to_rgb8();
    let encoded = webp::Encoder::from_rgb(rgb.as_raw(), rgb.width(), rgb.height()).encode(quality);
    std::fs::write(out, &*encoded)?;
    Ok(())
}

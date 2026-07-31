//! Video scrub strips: ten frames spread across the clip, tiled into one WebP.
//!
//! The grid scrubs a video tile by shifting the background position of this
//! strip, so hovering a video costs one already-decoded image and no player.

use image::{DynamicImage, GenericImage, RgbImage};

use crate::db::items::ItemFile;
use crate::error::{AppError, Result};
use crate::fs::paths::LibraryPaths;
use crate::media::thumbs::write_webp;
use crate::media::Kind;
use crate::sidecar::Tools;

pub const FRAMES: u32 = 10;
pub const FRAME_WIDTH: u32 = 240;
pub const QUALITY: f32 = 70.0;

pub fn generate(paths: &LibraryPaths, item: &ItemFile, tools: &Tools) -> Result<()> {
    if Kind::parse(&item.kind) != Kind::Video {
        return Err(AppError::media("scrub strips are video only"));
    }
    let ffmpeg = tools
        .ffmpeg
        .as_ref()
        .ok_or(AppError::ToolMissing("ffmpeg"))?;
    let duration = item
        .duration_ms
        .filter(|ms| *ms > 0)
        .ok_or_else(|| AppError::media("video has no duration"))? as f64
        / 1000.0;

    let src = paths.item_path(&item.folder_rel, &item.disk_name)?;
    let out = paths.sprite_path(&item.uuid);

    let frames: Vec<DynamicImage> = ffmpeg
        .frames(&src, FRAMES as usize, duration, FRAME_WIDTH)?
        .iter()
        .filter_map(|png| image::load_from_memory(png).ok())
        .collect();

    let first = frames
        .first()
        .ok_or_else(|| AppError::media("no frame decoded"))?;
    let (cell_w, cell_h) = (first.width(), first.height());

    let mut strip = RgbImage::new(cell_w * FRAMES, cell_h);
    for index in 0..FRAMES {
        // A short clip can yield fewer than ten frames; repeat the last so the
        // strip is always exactly ten cells wide and the frontend never has to
        // ask how many there are.
        let Some(frame) = frames.get(index as usize).or_else(|| frames.last()) else {
            continue;
        };
        let cell = frame.to_rgb8();
        if cell.width() != cell_w || cell.height() != cell_h {
            continue;
        }
        strip
            .copy_from(&cell, index * cell_w, 0)
            .map_err(|e| AppError::media(e.to_string()))?;
    }

    write_webp(&DynamicImage::ImageRgb8(strip), &out, QUALITY)
}

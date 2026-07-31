pub mod hash;
pub mod probe;
pub mod sprites;
pub mod thumbs;

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use crate::error::Result;

/// Open an image with the decoder chosen by **content**, never by extension.
///
/// `image::open` and `image::image_dimensions` both pick a decoder from the
/// file extension. Extensions lie: the first real library this was pointed at
/// held six JPEGs named `.PNG`, straight off a phone. Every one of them failed
/// with "Invalid PNG signature" and indexed with no dimensions, while opening
/// perfectly in every other program on the machine.
///
/// `with_guessed_format` reads the magic bytes and only falls back to the
/// extension when sniffing is inconclusive, so this is strictly better
/// information than the filename.
pub fn open_image_reader(path: &Path) -> Result<image::ImageReader<BufReader<File>>> {
    Ok(image::ImageReader::open(path)?.with_guessed_format()?)
}

/// What the app will try to do with a file. Anything it does not recognise is
/// still indexed — the library is the truth, not a curated subset — it just
/// never gets a thumbnail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Image,
    Video,
    Other,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Image => "image",
            Kind::Video => "video",
            Kind::Other => "other",
        }
    }

    /// Parse the value stored in `item.kind`. Not `FromStr`: an unknown value
    /// is `Other`, not an error.
    pub fn parse(s: &str) -> Kind {
        match s {
            "image" => Kind::Image,
            "video" => Kind::Video,
            _ => Kind::Other,
        }
    }

    /// Classification is by extension only. Sniffing content would mean
    /// opening every file in the library during the walk, which is the one
    /// thing the walk must stay cheap enough not to do.
    pub fn from_ext(ext: &str) -> Kind {
        const IMAGE: &[&str] = &[
            "jpg", "jpeg", "jpe", "png", "gif", "webp", "bmp", "tif", "tiff", "ico", "avif",
            "heic", "heif", "jxl",
        ];
        const VIDEO: &[&str] = &[
            "mp4", "m4v", "mov", "mkv", "webm", "avi", "wmv", "mpg", "mpeg", "flv", "ts", "m2ts",
            "mts", "3gp", "ogv",
        ];

        if IMAGE.contains(&ext) {
            Kind::Image
        } else if VIDEO.contains(&ext) {
            Kind::Video
        } else {
            Kind::Other
        }
    }
}

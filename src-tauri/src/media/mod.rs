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

    /// `from_ext`, upgraded to `Video` for an animated GIF, WebP or APNG —
    /// PLAN.md locked decision 17. Extension alone can't tell a still GIF
    /// from an animated one, so `gif`/`webp`/`png` get one extra, cheap read:
    /// a structural scan for an animation marker, never a full decode. Every
    /// other extension is classified exactly as before, with no extra I/O.
    ///
    /// A sniff that fails to open or parse the file falls back to the
    /// extension-only answer — probing already tolerates unreadable files
    /// (see `image::open_image_reader`'s doc comment), and this must too.
    pub fn classify(path: &Path, ext: &str) -> Kind {
        let base = Kind::from_ext(ext);
        if base != Kind::Image {
            return base;
        }
        let animated = match ext {
            "gif" => animated::gif_is_animated(path),
            "webp" => animated::webp_is_animated(path),
            "png" => animated::png_is_animated(path),
            _ => false,
        };
        if animated {
            Kind::Video
        } else {
            base
        }
    }
}

/// Structural sniffs for animated GIF/WebP/APNG — container-format block
/// scans, never a pixel decode. Each stops reading as soon as the answer is
/// known, so even a large animated file only costs a few KB.
mod animated {
    use std::fs::File;
    use std::io::{BufReader, Read};
    use std::path::Path;

    /// A GIF is animated if more than one Image Descriptor (`0x2C`) block
    /// appears. Blocks between them (Graphic Control / Comment / Application
    /// extensions, `0x21`) carry a standard length-prefixed sub-block chain
    /// terminated by a zero-length block, which is what `skip_sub_blocks`
    /// walks past without touching pixel data.
    pub fn gif_is_animated(path: &Path) -> bool {
        let Ok(file) = File::open(path) else { return false };
        let mut r = BufReader::new(file);

        let mut header = [0u8; 13];
        if r.read_exact(&mut header).is_err() {
            return false;
        }
        if &header[0..3] != b"GIF" {
            return false;
        }
        let packed = header[10];
        if packed & 0x80 != 0 {
            let table_len = 3usize * (1 << ((packed & 0x07) + 1));
            if skip(&mut r, table_len).is_err() {
                return false;
            }
        }

        let mut images = 0u32;
        loop {
            let mut marker = [0u8; 1];
            if r.read_exact(&mut marker).is_err() {
                return false;
            }
            match marker[0] {
                0x3B => return false, // trailer — end of stream
                0x2C => {
                    images += 1;
                    if images > 1 {
                        return true;
                    }
                    // Image Descriptor body: left,top,w,h (8) + packed (1).
                    let mut desc = [0u8; 9];
                    if r.read_exact(&mut desc).is_err() {
                        return false;
                    }
                    if desc[8] & 0x80 != 0 {
                        let table_len = 3usize * (1 << ((desc[8] & 0x07) + 1));
                        if skip(&mut r, table_len).is_err() {
                            return false;
                        }
                    }
                    // LZW minimum code size, then the sub-block chain.
                    if skip(&mut r, 1).is_err() || skip_sub_blocks(&mut r).is_err() {
                        return false;
                    }
                }
                0x21 => {
                    // Extension introducer: one label byte, then a sub-block chain.
                    if skip(&mut r, 1).is_err() || skip_sub_blocks(&mut r).is_err() {
                        return false;
                    }
                }
                _ => return false, // malformed — not this app's problem to fix
            }
        }
    }

    fn skip_sub_blocks(r: &mut impl Read) -> std::io::Result<()> {
        loop {
            let mut len = [0u8; 1];
            r.read_exact(&mut len)?;
            if len[0] == 0 {
                return Ok(());
            }
            skip(r, len[0] as usize)?;
        }
    }

    fn skip(r: &mut impl Read, n: usize) -> std::io::Result<()> {
        let mut buf = [0u8; 256];
        let mut left = n;
        while left > 0 {
            let take = left.min(buf.len());
            r.read_exact(&mut buf[..take])?;
            left -= take;
        }
        Ok(())
    }

    /// A WebP is animated if its RIFF container carries a top-level `ANIM`
    /// chunk (present alongside `VP8X` for any animated file, per the WebP
    /// container spec). A simple `VP8 `/`VP8L` file — no `VP8X` at all — is
    /// always a single static frame.
    pub fn webp_is_animated(path: &Path) -> bool {
        let Ok(file) = File::open(path) else { return false };
        let mut r = BufReader::new(file);

        let mut riff_header = [0u8; 12];
        if r.read_exact(&mut riff_header).is_err() {
            return false;
        }
        if &riff_header[0..4] != b"RIFF" || &riff_header[8..12] != b"WEBP" {
            return false;
        }

        loop {
            let mut chunk_header = [0u8; 8];
            if r.read_exact(&mut chunk_header).is_err() {
                return false; // ran out of chunks without seeing ANIM
            }
            let fourcc = &chunk_header[0..4];
            if fourcc == b"ANIM" {
                return true;
            }
            if fourcc == b"VP8 " || fourcc == b"VP8L" {
                return false; // simple format — never animated
            }
            let size = u32::from_le_bytes(chunk_header[4..8].try_into().unwrap()) as usize;
            let padded = size + (size & 1); // chunks are even-padded
            if skip(&mut r, padded).is_err() {
                return false;
            }
        }
    }

    /// An APNG is a PNG carrying an `acTL` chunk before its first `IDAT` —
    /// the whole of the APNG spec's detection rule.
    pub fn png_is_animated(path: &Path) -> bool {
        let Ok(file) = File::open(path) else { return false };
        let mut r = BufReader::new(file);

        let mut signature = [0u8; 8];
        if r.read_exact(&mut signature).is_err() || signature != *b"\x89PNG\r\n\x1a\n" {
            return false;
        }

        loop {
            let mut chunk_header = [0u8; 8];
            if r.read_exact(&mut chunk_header).is_err() {
                return false;
            }
            let length = u32::from_be_bytes(chunk_header[0..4].try_into().unwrap()) as usize;
            let kind = &chunk_header[4..8];
            if kind == b"acTL" {
                return true;
            }
            if kind == b"IDAT" {
                return false; // acTL must precede the first IDAT — spec rule
            }
            if skip(&mut r, length + 4).is_err() {
                return false; // + 4 for the trailing CRC
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::io::Write;

        fn scratch(name: &str) -> std::path::PathBuf {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("target")
                .join("test-fixtures")
                .join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            path
        }

        fn write(name: &str, bytes: &[u8]) -> std::path::PathBuf {
            let path = scratch(name);
            std::fs::File::create(&path).unwrap().write_all(bytes).unwrap();
            path
        }

        fn gif_image_descriptor() -> Vec<u8> {
            let mut b = vec![0x2C, 0, 0, 0, 0, 1, 0, 1, 0, 0x00]; // descriptor, no local table
            b.push(2); // LZW min code size
            b.push(1); // one-byte sub-block
            b.push(0x00);
            b.push(0x00); // sub-block terminator
            b
        }

        fn gif_bytes(frames: u32) -> Vec<u8> {
            let mut b = b"GIF89a".to_vec();
            b.extend_from_slice(&[1, 0, 1, 0, 0x00, 0, 0]); // 1x1, no global table
            for _ in 0..frames {
                b.extend(gif_image_descriptor());
            }
            b.push(0x3B); // trailer
            b
        }

        #[test]
        fn a_single_frame_gif_is_not_animated() {
            let path = write("static.gif", &gif_bytes(1));
            assert!(!gif_is_animated(&path));
        }

        #[test]
        fn a_multi_frame_gif_is_animated() {
            let path = write("animated.gif", &gif_bytes(2));
            assert!(gif_is_animated(&path));
        }

        fn webp_static_bytes() -> Vec<u8> {
            let rgb = image::RgbImage::from_pixel(2, 2, image::Rgb([10, 20, 30]));
            webp::Encoder::from_rgb(rgb.as_raw(), 2, 2).encode(80.0).to_vec()
        }

        fn webp_animated_bytes() -> Vec<u8> {
            let mut b = b"RIFF".to_vec();
            b.extend_from_slice(&[0, 0, 0, 0]); // riff size, unchecked by the sniff
            b.extend_from_slice(b"WEBP");
            b.extend_from_slice(b"ANIM");
            b.extend_from_slice(&6u32.to_le_bytes());
            b.extend_from_slice(&[0, 0, 0, 0, 1, 0]); // background colour + loop count
            b
        }

        #[test]
        fn a_static_webp_is_not_animated() {
            let path = write("static.webp", &webp_static_bytes());
            assert!(!webp_is_animated(&path));
        }

        #[test]
        fn a_webp_carrying_an_anim_chunk_is_animated() {
            let path = write("animated.webp", &webp_animated_bytes());
            assert!(webp_is_animated(&path));
        }

        fn png_chunk(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
            let mut b = (payload.len() as u32).to_be_bytes().to_vec();
            b.extend_from_slice(kind);
            b.extend_from_slice(payload);
            b.extend_from_slice(&[0, 0, 0, 0]); // crc, unchecked by the sniff
            b
        }

        fn apng_bytes() -> Vec<u8> {
            let mut b = b"\x89PNG\r\n\x1a\n".to_vec();
            let ihdr = {
                let mut p = 1u32.to_be_bytes().to_vec(); // width
                p.extend_from_slice(&1u32.to_be_bytes()); // height
                p.extend_from_slice(&[8, 2, 0, 0, 0]); // depth, colour, compress, filter, interlace
                p
            };
            b.extend(png_chunk(b"IHDR", &ihdr));
            b.extend(png_chunk(b"acTL", &[0, 0, 0, 2, 0, 0, 0, 0]));
            b
        }

        #[test]
        fn a_static_png_is_not_animated() {
            let image = image::RgbImage::from_pixel(2, 2, image::Rgb([1, 2, 3]));
            let path = scratch("static.png");
            image::DynamicImage::ImageRgb8(image)
                .save(&path)
                .unwrap();
            assert!(!png_is_animated(&path));
        }

        #[test]
        fn a_png_carrying_an_actl_chunk_before_idat_is_animated() {
            let path = write("animated.png", &apng_bytes());
            assert!(png_is_animated(&path));
        }

        #[test]
        fn classify_upgrades_only_the_animated_case() {
            let static_gif = write("classify-static.gif", &gif_bytes(1));
            let animated_gif = write("classify-animated.gif", &gif_bytes(2));
            assert_eq!(crate::media::Kind::classify(&static_gif, "gif"), crate::media::Kind::Image);
            assert_eq!(crate::media::Kind::classify(&animated_gif, "gif"), crate::media::Kind::Video);
        }
    }
}

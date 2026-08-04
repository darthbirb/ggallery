//! Dimensions, duration, codec and captured date.
//!
//! Probing never fails a job: a file the app cannot parse is still a real file
//! in the library and must still appear in the grid. Missing metadata is left
//! `None` and `captured_at` falls back to the file's own creation time
//! (`fs::walk::created_secs`, not its modification time — the file was made
//! once and may have been touched many times since), flagged as such so a
//! guess is never mistaken for metadata even though the inspector no longer
//! spells the source out in words.

use std::path::Path;

use crate::media::Kind;
use crate::sidecar::ffmpeg::Ffmpeg;

#[derive(Debug, Clone, Default)]
pub struct Probe {
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub duration_ms: Option<i64>,
    pub codec: Option<String>,
    pub bitrate: Option<i64>,
    pub captured_at: Option<i64>,
    /// exif | container | created
    pub captured_src: Option<String>,
}

pub fn probe(path: &Path, kind: Kind, created: i64, ffmpeg: Option<&Ffmpeg>) -> Probe {
    let mut out = match kind {
        Kind::Image => probe_image(path),
        Kind::Video => ffmpeg.map(|ff| probe_video(path, ff)).unwrap_or_default(),
        Kind::Other => Probe::default(),
    };

    if out.captured_at.is_none() {
        out.captured_at = Some(created);
        out.captured_src = Some("created".into());
    }
    out
}

fn probe_image(path: &Path) -> Probe {
    let mut out = Probe::default();

    if let Some((w, h)) = image_dimensions(path) {
        out.width = Some(w as i64);
        out.height = Some(h as i64);
    }

    if let Some(taken) = exif_captured_at(path) {
        out.captured_at = Some(taken);
        out.captured_src = Some("exif".into());
    }

    out
}

/// Dimensions read through a content sniff, so a file whose extension lies
/// still lands in the grid at the right shape rather than as a 1:1 guess.
fn image_dimensions(path: &Path) -> Option<(u32, u32)> {
    crate::media::open_image_reader(path)
        .ok()?
        .into_dimensions()
        .ok()
}

fn exif_captured_at(path: &Path) -> Option<i64> {
    let file = std::fs::File::open(path).ok()?;
    let mut reader = std::io::BufReader::new(file);
    let exif = exif::Reader::new().read_from_container(&mut reader).ok()?;

    for tag in [exif::Tag::DateTimeOriginal, exif::Tag::DateTime] {
        let Some(field) = exif.get_field(tag, exif::In::PRIMARY) else {
            continue;
        };
        if let exif::Value::Ascii(ref values) = field.value {
            let Some(raw) = values.first() else { continue };
            if let Ok(dt) = exif::DateTime::from_ascii(raw) {
                return Some(epoch_seconds(
                    dt.year as i64,
                    dt.month as u32,
                    dt.day as u32,
                    dt.hour as i64,
                    dt.minute as i64,
                    dt.second as i64,
                ));
            }
        }
    }
    None
}

fn probe_video(path: &Path, ffmpeg: &Ffmpeg) -> Probe {
    let mut out = Probe::default();
    let Ok(json) = ffmpeg.probe(path) else {
        return out;
    };

    if let Some(streams) = json.get("streams").and_then(|s| s.as_array()) {
        if let Some(video) = streams
            .iter()
            .find(|s| s.get("codec_type").and_then(|v| v.as_str()) == Some("video"))
        {
            out.width = video.get("width").and_then(|v| v.as_i64());
            out.height = video.get("height").and_then(|v| v.as_i64());
            out.codec = video
                .get("codec_name")
                .and_then(|v| v.as_str())
                .map(str::to_string);
        }
    }

    if let Some(format) = json.get("format") {
        out.duration_ms = format
            .get("duration")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .map(|secs| (secs * 1000.0).round() as i64);
        out.bitrate = format
            .get("bit_rate")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<i64>().ok());

        if let Some(created) = format
            .get("tags")
            .and_then(|t| t.get("creation_time"))
            .and_then(|v| v.as_str())
            .and_then(parse_iso8601)
        {
            out.captured_at = Some(created);
            out.captured_src = Some("container".into());
        }
    }

    out
}

/// `2024-06-12T10:33:21.000000Z` — the only shape containers actually emit.
fn parse_iso8601(text: &str) -> Option<i64> {
    let (date, rest) = text.split_once('T')?;
    let time = rest.split(['.', 'Z', '+']).next()?;

    let mut date_parts = date.split('-');
    let year: i64 = date_parts.next()?.parse().ok()?;
    let month: u32 = date_parts.next()?.parse().ok()?;
    let day: u32 = date_parts.next()?.parse().ok()?;

    let mut time_parts = time.split(':');
    let hour: i64 = time_parts.next()?.parse().ok()?;
    let minute: i64 = time_parts.next()?.parse().ok()?;
    let second: i64 = time_parts.next().unwrap_or("0").parse().ok()?;

    Some(epoch_seconds(year, month, day, hour, minute, second))
}

/// Civil date to Unix seconds, treating the value as UTC. No timezone
/// database, and none wanted: EXIF has no offset in the common case, so
/// pretending to know one would be a lie either way.
fn epoch_seconds(year: i64, month: u32, day: u32, hour: i64, minute: i64, second: i64) -> i64 {
    days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second
}

/// Howard Hinnant's `days_from_civil`, days since 1970-01-01.
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let month = month as i64;
    let day = day as i64;
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_matches_known_dates() {
        assert_eq!(epoch_seconds(1970, 1, 1, 0, 0, 0), 0);
        assert_eq!(epoch_seconds(2000, 3, 1, 0, 0, 0), 951_868_800);
        assert_eq!(epoch_seconds(2024, 6, 12, 10, 33, 21), 1_718_188_401);
    }

    #[test]
    fn parses_container_timestamps() {
        assert_eq!(
            parse_iso8601("2024-06-12T10:33:21.000000Z"),
            Some(1_718_188_401)
        );
        assert_eq!(parse_iso8601("nonsense"), None);
    }
}

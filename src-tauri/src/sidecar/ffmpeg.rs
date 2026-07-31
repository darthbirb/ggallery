use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::error::{AppError, Result};

/// Windows: keep every sidecar invocation from flashing a console window.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone)]
pub struct Ffmpeg {
    ffmpeg: PathBuf,
    ffprobe: PathBuf,
}

impl Ffmpeg {
    pub fn new(ffmpeg: PathBuf, ffprobe: PathBuf) -> Self {
        Ffmpeg { ffmpeg, ffprobe }
    }

    pub fn binary(&self) -> &Path {
        &self.ffmpeg
    }

    /// `ffprobe -show_format -show_streams`, parsed.
    pub fn probe(&self, file: &Path) -> Result<serde_json::Value> {
        let out = run(
            &self.ffprobe,
            &[
                "-v",
                "error",
                "-print_format",
                "json",
                "-show_format",
                "-show_streams",
                &file.to_string_lossy(),
            ],
        )?;
        Ok(serde_json::from_slice(&out)?)
    }

    /// A single frame, seeked to `at_secs`, as PNG bytes.
    pub fn frame(&self, file: &Path, at_secs: f64, width: u32) -> Result<Vec<u8>> {
        let out = run(
            &self.ffmpeg,
            &[
                "-v",
                "error",
                "-nostdin",
                "-ss",
                &format!("{at_secs:.3}"),
                "-i",
                &file.to_string_lossy(),
                "-frames:v",
                "1",
                "-vf",
                &format!("scale={width}:-2:flags=bilinear"),
                "-f",
                "image2pipe",
                "-vcodec",
                "png",
                "-",
            ],
        )?;
        if out.is_empty() {
            return Err(AppError::media("ffmpeg produced no frame"));
        }
        Ok(out)
    }

    /// Up to `count` frames spread across the whole file, in one pass. One
    /// process per video rather than one per frame — a scrub strip is ten
    /// frames, and ten spawns per video is minutes of pure process overhead
    /// across a real library.
    pub fn frames(
        &self,
        file: &Path,
        count: usize,
        duration_secs: f64,
        width: u32,
    ) -> Result<Vec<Vec<u8>>> {
        if duration_secs <= 0.0 || count == 0 {
            return Err(AppError::media("cannot sample frames without a duration"));
        }
        // fps=count/duration lands one frame per slice, starting at t=0.
        let rate = format!("{}/{:.3}", count, duration_secs);
        let out = run(
            &self.ffmpeg,
            &[
                "-v",
                "error",
                "-nostdin",
                "-i",
                &file.to_string_lossy(),
                "-vf",
                &format!("fps={rate},scale={width}:-2:flags=bilinear"),
                "-frames:v",
                &count.to_string(),
                "-f",
                "image2pipe",
                "-vcodec",
                "png",
                "-",
            ],
        )?;
        let frames = split_pngs(&out);
        if frames.is_empty() {
            return Err(AppError::media("ffmpeg produced no frames"));
        }
        Ok(frames)
    }
}

fn run(binary: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let mut cmd = Command::new(binary);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let out = cmd.output()?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let tail = err.lines().last().unwrap_or("").trim().to_string();
        return Err(AppError::media(format!(
            "{} failed: {}",
            binary.file_name().unwrap_or_default().to_string_lossy(),
            if tail.is_empty() { "no output" } else { &tail }
        )));
    }
    Ok(out.stdout)
}

/// image2pipe concatenates complete PNGs. Split them by walking the chunk
/// structure to each IEND rather than searching for the signature — a byte
/// sequence can occur inside compressed image data, a chunk length cannot lie.
fn split_pngs(buf: &[u8]) -> Vec<Vec<u8>> {
    const SIG: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    let mut frames = Vec::new();
    let mut pos = 0usize;

    while pos + 8 <= buf.len() && buf[pos..pos + 8] == SIG {
        let start = pos;
        let mut cursor = pos + 8;
        loop {
            if cursor + 8 > buf.len() {
                return frames;
            }
            let len = u32::from_be_bytes([
                buf[cursor],
                buf[cursor + 1],
                buf[cursor + 2],
                buf[cursor + 3],
            ]) as usize;
            let kind = &buf[cursor + 4..cursor + 8];
            let next = match cursor.checked_add(12).and_then(|c| c.checked_add(len)) {
                Some(next) if next <= buf.len() => next,
                _ => return frames,
            };
            cursor = next;
            if kind == b"IEND" {
                break;
            }
        }
        frames.push(buf[start..cursor].to_vec());
        pos = cursor;
    }

    frames
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(payload: &[u8]) -> Vec<u8> {
        let mut out = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
        out.extend((payload.len() as u32).to_be_bytes());
        out.extend(b"IDAT");
        out.extend(payload);
        out.extend([0, 0, 0, 0]); // crc, unchecked here
        out.extend(0u32.to_be_bytes());
        out.extend(b"IEND");
        out.extend([0, 0, 0, 0]);
        out
    }

    #[test]
    fn splits_concatenated_frames() {
        // Payload deliberately contains a PNG signature: signature scanning
        // would split here, chunk walking must not.
        let a = png(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
        let b = png(&[1, 2, 3]);
        let mut joined = a.clone();
        joined.extend(&b);

        let frames = split_pngs(&joined);
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0], a);
        assert_eq!(frames[1], b);
    }
}

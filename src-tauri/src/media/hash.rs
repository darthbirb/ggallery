use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::error::Result;

/// Content hash is identity. It is what lets M1.5 rename every file to a UUID
/// and re-link the database afterwards, and what makes reconciliation after an
/// external tool touched the folder a lookup rather than a guess.
pub fn blake3_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; 256 * 1024];

    loop {
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }

    Ok(hasher.finalize().to_hex().to_string())
}

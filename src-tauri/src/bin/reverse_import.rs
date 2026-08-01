//! Standalone reversal tool for the M1.5 UUID rename.
//!
//! Reads `<root>/.gallery/library.jsonl` and renames every item still
//! carrying a `<uuid>.<ext>` name back to its pre-import filename.
//!
//! Deliberately independent of the database and the rest of the app: it must
//! still work if the database is exactly the thing that broke, or on a
//! machine where the app itself will not start. It only needs the jsonl file
//! and the files it describes.
//!
//! Usage:
//!   reverse_import <library-root> [--dry-run]

use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::ExitCode;

use gallery_lib::fs::import::ReversalRecord;
use gallery_lib::fs::paths::LibraryPaths;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(root) = args.next() else {
        eprintln!("usage: reverse_import <library-root> [--dry-run]");
        return ExitCode::FAILURE;
    };
    let dry_run = args.any(|a| a == "--dry-run");

    let paths = LibraryPaths::new(PathBuf::from(root));
    let jsonl_path = paths.jsonl_path();
    let file = match fs::File::open(&jsonl_path) {
        Ok(file) => file,
        Err(err) => {
            eprintln!("could not open {}: {err}", jsonl_path.display());
            return ExitCode::FAILURE;
        }
    };

    // Keyed by uuid so a rerun of the wizard that re-logged the same file
    // (the crash-resume path in `fs::import::execute`) only reverses it once.
    // The content for a given uuid never actually changes between entries.
    let mut records: HashMap<String, ReversalRecord> = HashMap::new();
    let mut unreadable_lines = 0u32;
    for line in BufReader::new(file).lines() {
        let Ok(line) = line else {
            unreadable_lines += 1;
            continue;
        };
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<ReversalRecord>(&line) {
            Ok(record) => {
                records.insert(record.uuid.clone(), record);
            }
            Err(err) => {
                eprintln!("skipping unreadable line: {err}");
                unreadable_lines += 1;
            }
        }
    }

    if records.is_empty() {
        println!("nothing to reverse — {} has no entries", jsonl_path.display());
        return ExitCode::SUCCESS;
    }

    // Sorted for stable, readable output — order has no other significance,
    // every record is independent.
    let mut ordered: Vec<ReversalRecord> = records.into_values().collect();
    ordered.sort_by(|a, b| {
        (a.folder_rel.as_str(), a.new_name.as_str()).cmp(&(b.folder_rel.as_str(), b.new_name.as_str()))
    });

    let mut restored = 0u32;
    let mut already_original = 0u32;
    let mut conflicts = 0u32;

    for record in &ordered {
        let folder_abs = match paths.to_abs(&record.folder_rel) {
            Ok(abs) => abs,
            Err(err) => {
                eprintln!("skipping {}: {err}", record.folder_rel);
                conflicts += 1;
                continue;
            }
        };
        let current = folder_abs.join(&record.new_name);
        let original = folder_abs.join(&record.orig_name);

        if !current.is_file() {
            // Never renamed in the first place, or already reversed.
            already_original += 1;
            continue;
        }
        if original.exists() {
            eprintln!(
                "conflict: {} already exists — leaving {} as it is",
                original.display(),
                current.display()
            );
            conflicts += 1;
            continue;
        }

        println!("{} -> {}", current.display(), original.display());
        if !dry_run {
            if let Err(err) = fs::rename(&current, &original) {
                eprintln!("failed to restore {}: {err}", current.display());
                conflicts += 1;
                continue;
            }
        }
        restored += 1;
    }

    let prefix = if dry_run { "[dry run] " } else { "" };
    println!(
        "{prefix}{restored} restored, {already_original} already original, {conflicts} conflicts, \
         {unreadable_lines} unreadable log lines"
    );

    if conflicts > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

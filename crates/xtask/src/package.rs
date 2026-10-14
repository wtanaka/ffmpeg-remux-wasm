//! `cargo xtask package [output.zip]`: a zip of the files git would
//! commit, for uploading the project somewhere that takes a zip.
//!
//! The list is what git reports as tracked, plus new files that `.gitignore`
//! doesn't exclude, so build output, local configuration and the like are left
//! out. It needs `git`, which finds the list; the zip is written here, in the
//! standard library only (the files are stored, not compressed: the project is
//! small, and a stored zip needs no compression code).

use crate::{io, paths, process, Result};
use std::path::{Path, PathBuf};

const TOO_BIG: &str =
    "too large for the plain zip format this task writes (over 4 GB)";

/// A file to put in the zip.
struct Entry {
    /// The path inside the zip.
    name: String,
    data: Vec<u8>,
    /// The permission bits to record: 0o644, or 0o755 for an executable.
    mode: u32,
}

/// The permissions a zip records for a file with these Unix permission bits.
fn zip_mode(unix_mode: u32) -> u32 {
    if unix_mode & 0o111 == 0 {
        0o644
    } else {
        0o755
    }
}

/// The CRC-32 that zip uses (the same polynomial as gzip and PNG).
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn le16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn le32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

/// What the central directory needs to know about a file already written.
struct Written {
    name: String,
    crc: u32,
    size: u32,
    offset: u32,
    mode: u32,
}

/// The bytes of a stored zip holding the entries.
fn zip_bytes(entries: &[Entry]) -> std::result::Result<Vec<u8>, String> {
    let mut zip: Vec<u8> = Vec::new();
    let mut written: Vec<Written> = Vec::new();
    for entry in entries {
        let name = entry.name.as_bytes();
        let name_len = u16::try_from(name.len())
            .map_err(|_| format!("the file name {} is too long", entry.name))?;
        let size = u32::try_from(entry.data.len())
            .map_err(|_| format!("{} is {TOO_BIG}", entry.name))?;
        let offset = u32::try_from(zip.len())
            .map_err(|_| format!("the zip is {TOO_BIG}"))?;
        let crc = crc32(&entry.data);
        le32(&mut zip, 0x0403_4B50); // local file header
        le16(&mut zip, 20); // version needed
        le16(&mut zip, 0x0800); // names are UTF-8
        le16(&mut zip, 0); // stored
        le16(&mut zip, 0); // time
        le16(&mut zip, 0x21); // date 1980-01-01: the zip never depends on when
        le32(&mut zip, crc);
        le32(&mut zip, size);
        le32(&mut zip, size);
        le16(&mut zip, name_len);
        le16(&mut zip, 0); // no extra field
        zip.extend_from_slice(name);
        zip.extend_from_slice(&entry.data);
        written.push(Written {
            name: entry.name.clone(),
            crc,
            size,
            offset,
            mode: entry.mode,
        });
    }
    let directory_start = u32::try_from(zip.len())
        .map_err(|_| format!("the zip is {TOO_BIG}"))?;
    for file in &written {
        le32(&mut zip, 0x0201_4B50); // central directory header
        le16(&mut zip, 0x031E); // made by Unix, zip 3.0
        le16(&mut zip, 20);
        le16(&mut zip, 0x0800);
        le16(&mut zip, 0);
        le16(&mut zip, 0);
        le16(&mut zip, 0x21);
        le32(&mut zip, file.crc);
        le32(&mut zip, file.size);
        le32(&mut zip, file.size);
        le16(&mut zip, file.name.len() as u16); // checked above
        le16(&mut zip, 0); // extra
        le16(&mut zip, 0); // comment
        le16(&mut zip, 0); // disk
        le16(&mut zip, 0); // internal attributes
        le32(&mut zip, (0o100_000 | file.mode) << 16); // a regular file
        le32(&mut zip, file.offset);
        zip.extend_from_slice(file.name.as_bytes());
    }
    let directory_size = u32::try_from(zip.len())
        .map_err(|_| format!("the zip is {TOO_BIG}"))?
        - directory_start;
    let count = u16::try_from(written.len()).map_err(|_| {
        "more than 65535 files, too many for the plain zip format this task \
         writes"
            .to_string()
    })?;
    le32(&mut zip, 0x0605_4B50); // end of central directory
    le16(&mut zip, 0);
    le16(&mut zip, 0);
    le16(&mut zip, count);
    le16(&mut zip, count);
    le32(&mut zip, directory_size);
    le32(&mut zip, directory_start);
    le16(&mut zip, 0);
    Ok(zip)
}

/// The file names in `git ls-files -z` output, sorted and without repeats.
fn parse_git_listing(listing: &[u8]) -> Vec<String> {
    let mut names: Vec<String> = listing
        .split(|byte| *byte == 0)
        .filter(|name| !name.is_empty())
        .map(|name| String::from_utf8_lossy(name).into_owned())
        .collect();
    names.sort();
    names.dedup();
    names
}

/// The `target_directory` in `cargo metadata`'s JSON.
fn parse_target_directory(json: &str) -> std::result::Result<PathBuf, String> {
    let key = "\"target_directory\":\"";
    let start = json
        .find(key)
        .ok_or("cargo metadata did not say where the target directory is")?
        + key.len();
    // The value is a JSON string: undo the escapes of quotes and backslashes.
    let mut path = String::new();
    let mut chars = json[start..].chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Ok(PathBuf::from(path)),
            '\\' => path.extend(chars.next()),
            _ => path.push(c),
        }
    }
    Err("cargo metadata's output ended inside the target directory".into())
}

/// Cargo's target directory, asked of cargo so that CARGO_TARGET_DIR and
/// `build.target-dir` in a cargo config are both respected.
fn target_directory() -> Result<PathBuf> {
    let output = process::output(
        "cargo",
        &["metadata", "--no-deps", "--format-version", "1"],
    )?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }
    Ok(parse_target_directory(&String::from_utf8_lossy(
        &output.stdout,
    ))?)
}

/// The files to package, relative to the repository root.
fn files_in_git() -> Result<Vec<String>> {
    let output = process::output(
        "git",
        &[
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ],
    )?;
    if !output.status.success() {
        return Err(format!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }
    Ok(parse_git_listing(&output.stdout))
}

/// The permission bits of a file, as zip records them.
#[cfg(unix)]
fn file_mode(metadata: &std::fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt as _;
    zip_mode(metadata.permissions().mode())
}

#[cfg(not(unix))]
fn file_mode(_metadata: &std::fs::Metadata) -> u32 {
    0o644
}

/// Reads the files. A file that git lists but that has been deleted, a symbolic
/// link or a folder (a submodule) isn't packaged.
fn read_entries(root: &Path, names: &[String]) -> Result<Vec<Entry>> {
    let mut entries = Vec::new();
    for name in names {
        let path = root.join(name);
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        entries.push(Entry {
            name: name.clone(),
            data: io::read(&path)?,
            mode: file_mode(&metadata),
        });
    }
    Ok(entries)
}

pub fn run(output: Option<&str>) -> Result {
    let root = paths::root();
    // A path given on the command line is relative to where it was typed; the
    // default is under cargo's target directory.
    let output: PathBuf = match output {
        Some(path) => PathBuf::from(path),
        None => target_directory()?.join(paths::PACKAGE_ZIP),
    };
    if let Some(folder) = output.parent().filter(|p| !p.as_os_str().is_empty())
    {
        io::create_dir_all(folder)?;
    }
    // Never package the zip itself, if an earlier one is lying in the
    // repository.
    let output_name = output.canonicalize().ok();
    let mut names = files_in_git()?;
    names.retain(|name| {
        output_name.as_deref() != root.join(name).canonicalize().ok().as_deref()
    });
    let entries = read_entries(&root, &names)?;
    let zip = zip_bytes(&entries)?;
    io::write(&output, &zip)?;
    println!(
        "wrote {} ({} files, {} KB)",
        output.display(),
        entries.len(),
        zip.len().div_ceil(1024)
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, data: &[u8], mode: u32) -> Entry {
        Entry {
            name: name.to_string(),
            data: data.to_vec(),
            mode,
        }
    }

    fn u16_at(bytes: &[u8], at: usize) -> usize {
        u16::from_le_bytes([bytes[at], bytes[at + 1]]) as usize
    }

    fn u32_at(bytes: &[u8], at: usize) -> usize {
        u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize
    }

    /// Reads a stored zip back: each file's name, permissions and contents.
    fn read_zip(zip: &[u8]) -> Vec<(String, u32, Vec<u8>)> {
        let end = zip.len() - 22; // no comment, so the end record is last
        assert_eq!(u32_at(zip, end), 0x0605_4B50);
        let count = u16_at(zip, end + 10);
        let mut at = u32_at(zip, end + 16);
        let mut files = Vec::new();
        for _ in 0..count {
            assert_eq!(u32_at(zip, at), 0x0201_4B50);
            let crc = u32_at(zip, at + 16);
            let size = u32_at(zip, at + 24);
            let name_len = u16_at(zip, at + 28);
            let mode = (u32_at(zip, at + 38) >> 16) as u32 & 0o7777;
            let local = u32_at(zip, at + 42);
            let name =
                String::from_utf8(zip[at + 46..at + 46 + name_len].to_vec())
                    .unwrap();
            // The local header names the same file, then holds the data.
            assert_eq!(u32_at(zip, local), 0x0403_4B50);
            let local_name_len = u16_at(zip, local + 26);
            let data_at = local + 30 + local_name_len;
            let data = zip[data_at..data_at + size].to_vec();
            assert_eq!(crc32(&data) as usize, crc);
            files.push((name, mode, data));
            at += 46 + name_len;
        }
        files
    }

    #[test]
    fn crc32_matches_the_standard_check_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn a_zip_reads_back_as_what_was_put_in() {
        let zip = zip_bytes(&[
            entry("a.txt", b"one", 0o644),
            entry("dir/run.sh", b"#!/bin/sh\n", 0o755),
            entry("empty", b"", 0o644),
        ])
        .unwrap();
        assert_eq!(
            read_zip(&zip),
            [
                ("a.txt".to_string(), 0o644, b"one".to_vec()),
                ("dir/run.sh".to_string(), 0o755, b"#!/bin/sh\n".to_vec()),
                ("empty".to_string(), 0o644, Vec::new()),
            ]
        );
    }

    #[test]
    fn the_zip_does_not_depend_on_when_it_is_made() {
        let entries = [entry("a", b"x", 0o644)];
        assert_eq!(zip_bytes(&entries), zip_bytes(&entries));
    }

    #[test]
    fn an_executable_bit_anywhere_makes_a_file_executable() {
        assert_eq!(zip_mode(0o644), 0o644);
        assert_eq!(zip_mode(0o700), 0o755);
        assert_eq!(zip_mode(0o010), 0o755);
    }

    #[test]
    fn the_git_listing_is_sorted_without_repeats_or_blanks() {
        assert_eq!(parse_git_listing(b"b\0a\0b\0\0c\0"), ["a", "b", "c"]);
        assert!(parse_git_listing(b"").is_empty());
    }

    #[test]
    fn the_target_directory_is_read_from_cargo_metadata() {
        let json = r#"{"packages":[],"target_directory":"/tmp/t","other":1}"#;
        assert_eq!(parse_target_directory(json), Ok(PathBuf::from("/tmp/t")));
        let escaped = r#"{"target_directory":"C:\\work\\\"x\""}"#;
        assert_eq!(
            parse_target_directory(escaped),
            Ok(PathBuf::from("C:\\work\\\"x\""))
        );
        assert!(parse_target_directory("{}").is_err());
        assert!(parse_target_directory(r#"{"target_directory":"/tm"#).is_err());
    }
}

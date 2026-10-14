//! File operations whose errors name the file and what was being done to it (a
//! bare "No such file or directory" doesn't say which file).

use crate::Result;
use std::path::Path;

fn fail<T>(doing: &str, path: &Path, err: std::io::Error) -> Result<T> {
    Err(format!("could not {doing} {}: {err}", path.display()).into())
}

pub fn read(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).or_else(|e| fail("read", path, e))
}

pub fn read_to_string(path: &Path) -> Result<String> {
    std::fs::read_to_string(path).or_else(|e| fail("read", path, e))
}

pub fn write(path: &Path, data: impl AsRef<[u8]>) -> Result {
    std::fs::write(path, data).or_else(|e| fail("write", path, e))
}

pub fn copy(from: &Path, to: &Path) -> Result {
    match std::fs::copy(from, to) {
        Ok(_) => Ok(()),
        Err(e) => Err(format!(
            "could not copy {} to {}: {e}",
            from.display(),
            to.display()
        )
        .into()),
    }
}

pub fn rename(from: &Path, to: &Path) -> Result {
    match std::fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(e) => Err(format!(
            "could not move {} to {}: {e}",
            from.display(),
            to.display()
        )
        .into()),
    }
}

pub fn create_dir_all(path: &Path) -> Result {
    std::fs::create_dir_all(path)
        .or_else(|e| fail("create the folder", path, e))
}

/// The names of what is in a folder, sorted.
pub fn list_dir(path: &Path) -> Result<Vec<String>> {
    let entries = std::fs::read_dir(path)
        .or_else(|e| fail("read the folder", path, e))?;
    let mut names: Vec<String> = entries
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    Ok(names)
}

/// Removes a file or a whole folder, if it is there. Whether it was.
pub fn remove_if_exists(path: &Path) -> Result<bool> {
    let removed = if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else if path.exists() {
        std::fs::remove_file(path)
    } else {
        return Ok(false);
    };
    match removed {
        Ok(()) => Ok(true),
        Err(e) => fail("remove", path, e),
    }
}

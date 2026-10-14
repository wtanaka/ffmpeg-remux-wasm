//! SHA-256 hashes, and the `SHA256SUMS` file a release holds.

use crate::{io, paths, Result};
use sha2::{Digest, Sha256};
use std::path::Path;

/// The SHA-256 of `data`, as lowercase hex.
pub fn sha256_hex(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Writes `dir/SHA256SUMS` for the named files in `dir`, in the format of
/// `sha256sum`: `<hash>  <name>` per line.
pub fn write(dir: &Path, names: &[&str]) -> Result<String> {
    let mut sums = String::new();
    for name in names {
        let hash = sha256_hex(&io::read(&dir.join(name))?);
        sums.push_str(&format!("{hash}  {name}\n"));
    }
    io::write(&dir.join(paths::SUMS_NAME), &sums)?;
    Ok(sums)
}

/// Checks every file listed in `dir/SHA256SUMS` against its hash.
pub fn verify(dir: &Path) -> Result {
    let sums_path = dir.join(paths::SUMS_NAME);
    let sums = io::read_to_string(&sums_path)?;
    let mut checked = 0;
    for line in sums.lines().filter(|line| !line.trim().is_empty()) {
        let (want, name) = line.split_once("  ").ok_or_else(|| {
            format!(
                "{} has a line that is not `<hash>  <file>`: {line:?}",
                sums_path.display()
            )
        })?;
        let got = sha256_hex(&io::read(&dir.join(name))?);
        if got != want {
            return Err(format!(
                "{name} has SHA-256 {got}, but {} lists {want}",
                sums_path.display()
            )
            .into());
        }
        checked += 1;
    }
    if checked == 0 {
        return Err(format!("{} lists no files", sums_path.display()).into());
    }
    println!("{checked} files match {}", paths::SUMS_NAME);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{sha256_hex, verify, write};

    /// A fresh empty folder under the system temp directory.
    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("xtask-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn written_sums_verify_and_a_changed_file_does_not() {
        let dir = temp_dir("sums");
        std::fs::write(dir.join("a.txt"), "one").unwrap();
        std::fs::write(dir.join("b.txt"), "two").unwrap();
        let sums = write(&dir, &["a.txt", "b.txt"]).unwrap();
        assert!(sums.contains("  a.txt\n") && sums.contains("  b.txt\n"));
        assert!(verify(&dir).is_ok());
        std::fs::write(dir.join("b.txt"), "changed").unwrap();
        let error = verify(&dir).unwrap_err().to_string();
        assert!(error.contains("b.txt"), "{error}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_empty_sums_file_is_an_error() {
        let dir = temp_dir("empty");
        write(&dir, &[]).unwrap();
        assert!(verify(&dir).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sha256_matches_known_values() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}

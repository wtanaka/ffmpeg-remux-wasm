//! The release version, and the check that a release tag is it: a release is
//! the tag `v<version>`, where the version is `version` in the root Cargo.toml.

use crate::{io, paths, Result};

/// The `version = "..."` line of a Cargo.toml: the first one, which in this
/// repository's root manifest is `[workspace.package]`'s.
pub fn version_in(manifest: &str) -> Option<String> {
    manifest.lines().find_map(|line| {
        let rest = line.strip_prefix("version = \"")?;
        Some(rest.split('"').next()?.to_string())
    })
}

/// The tag for a release version.
pub fn tag_for(version: &str) -> String {
    format!("v{version}")
}

/// Whether `tag` is the tag for `version`.
fn tag_matches(tag: &str, version: &str) -> bool {
    tag == tag_for(version)
}

/// The release version.
pub fn version() -> Result<String> {
    let manifest = io::read_to_string(&paths::abs(paths::ROOT_MANIFEST))?;
    version_in(&manifest).ok_or_else(|| {
        format!(
            "no `version = \"...\"` line in {}: the release version is set \
             there",
            paths::ROOT_MANIFEST
        )
        .into()
    })
}

/// `cargo xtask check-tag <tag>`: fails unless the tag is the release version.
pub fn check_tag(tag: &str) -> Result {
    let version = version()?;
    if tag_matches(tag, &version) {
        println!("tag {tag} is the release version");
        Ok(())
    } else {
        Err(format!(
            "tag {tag} does not match the release version v{version} \
             (`version` in {}): bump it, or retag",
            paths::ROOT_MANIFEST
        )
        .into())
    }
}

#[cfg(test)]
mod tests {
    use super::{tag_matches, version_in};

    #[test]
    fn the_version_is_the_first_version_line() {
        let manifest =
            "[workspace]\n\n[workspace.package]\nversion = \"1.2.3\"\n\
                        edition = \"2021\"\n";
        assert_eq!(version_in(manifest).as_deref(), Some("1.2.3"));
        assert_eq!(version_in("[workspace]\n"), None);
    }

    #[test]
    fn a_tag_is_v_and_the_version() {
        assert!(tag_matches("v1.2.3", "1.2.3"));
        assert!(!tag_matches("1.2.3", "1.2.3"));
        assert!(!tag_matches("v1.2.4", "1.2.3"));
        assert!(!tag_matches("v1.2.3-rc1", "1.2.3"));
    }
}

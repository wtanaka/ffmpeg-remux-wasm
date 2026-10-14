//! `cargo xtask clean [--all]`: removes what the other tasks generate.
//!
//! By default the cheap thing: ffmpeg/dist/ (the release files). `--all` also
//! removes ffmpeg/build/ (the downloaded FFmpeg source and its compiled
//! objects, which take a download and several minutes to get back) and runs
//! `cargo clean`.

use crate::{io, paths, process, Result};

/// What to remove, relative to the repository root.
fn targets(all: bool) -> Vec<&'static str> {
    if all {
        vec![paths::DIST_DIR, paths::BUILD_DIR]
    } else {
        vec![paths::DIST_DIR]
    }
}

pub fn run(all: bool) -> Result {
    for path in targets(all) {
        if io::remove_if_exists(&paths::abs(path))? {
            println!("removed {path}");
        }
    }
    if all {
        process::run("cargo", &["clean"])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::targets;
    use crate::paths;

    #[test]
    fn the_build_tree_is_only_removed_with_all() {
        assert_eq!(targets(false), vec![paths::DIST_DIR]);
        assert_eq!(targets(true), vec![paths::DIST_DIR, paths::BUILD_DIR]);
    }
}

//! The release notes: what changed, written from the commit messages by
//! git-cliff (Conventional Commits; see .github/cliff.toml).

use crate::process::args;
use crate::{paths, process, tools, Result};

/// git-cliff arguments: the notes of the latest tag.
fn latest_args() -> Vec<String> {
    args(&[
        "--config",
        paths::CLIFF_CONFIG,
        "--latest",
        "--strip",
        "header",
    ])
}

/// git-cliff arguments: render everything since the last tag, only to check
/// that the configuration works.
fn check_args() -> Vec<String> {
    args(&[
        "--config",
        paths::CLIFF_CONFIG,
        "--unreleased",
        "--strip",
        "all",
    ])
}

/// What git-cliff printed (its progress messages go to stderr), once the
/// pinned version is known to be installed.
fn run(arguments: &[String]) -> Result<String> {
    tools::ensure(&tools::git_cliff())?;
    process::text("git-cliff", arguments)
}

/// The notes of the latest tag: this release, if it is the checkout's latest
/// tag (which the release workflow has, with the whole history).
pub fn latest() -> Result<String> {
    run(&latest_args())
}

/// Checks that git-cliff reads its configuration.
pub fn check_config() -> Result {
    run(&check_args()).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_runs_use_the_repositorys_configuration() {
        for arguments in [latest_args(), check_args()] {
            assert!(arguments
                .windows(2)
                .any(|w| w == ["--config", paths::CLIFF_CONFIG]));
        }
        assert!(latest_args().contains(&"--latest".to_string()));
        assert!(check_args().contains(&"--unreleased".to_string()));
    }
}

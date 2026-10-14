//! The programs the tasks use besides cargo (dprint, shfmt, git-cliff): each
//! has a pinned version in a file under .ci/, which .ci/setup-ubuntu.sh
//! installs. A missing or different one is an error that says how to install
//! it; the tasks never install anything themselves.

use crate::process::{self, Env};
use crate::{paths, Result};

pub struct Tool {
    pub program: &'static str,
    /// The pinned version.
    version: String,
    /// What `--version` prints for the pinned version.
    expected: String,
    /// The file that pins the version.
    pin_file: &'static str,
    /// Ways to install it, as lines of the message.
    installs: Vec<String>,
}

fn setup_line() -> String {
    format!(
        "sh {}   (a bare Ubuntu machine: installs everything)",
        paths::SETUP_SCRIPT
    )
}

/// The ways to install a Rust program that has prebuilt downloads: binstall
/// fetches the download, `cargo install` compiles it.
fn cargo_installs(program: &str, version: &str) -> Vec<String> {
    vec![
        format!(
            "cargo binstall {program}@{version}   (a prebuilt download; needs \
             cargo-binstall)"
        ),
        format!(
            "cargo install --locked {program} --version {version}   \
             (compiles it)"
        ),
        setup_line(),
    ]
}

/// A Rust program whose `--version` prints its name and the version, and that
/// cargo installs (from a prebuilt download, or by compiling it).
fn cargo_tool(
    program: &'static str,
    pinned: &str,
    pin_file: &'static str,
) -> Tool {
    let version = pinned.trim().to_string();
    Tool {
        program,
        expected: format!("{program} {version}"),
        installs: cargo_installs(program, &version),
        version,
        pin_file,
    }
}

pub fn dprint() -> Tool {
    cargo_tool("dprint", paths::DPRINT_VERSION, paths::DPRINT_VERSION_FILE)
}

pub fn shfmt() -> Tool {
    let version = paths::SHFMT_VERSION.trim().to_string();
    Tool {
        program: "shfmt",
        expected: format!("v{version}"),
        installs: vec![
            "brew install shfmt   (macOS; may be a different version)"
                .to_string(),
            format!(
                "sh {}   (a bare Ubuntu machine: downloads v{version} and \
                 checks it against {})",
                paths::SETUP_SCRIPT,
                paths::SHFMT_SHA256_FILE
            ),
            "a release binary from https://github.com/mvdan/sh/releases"
                .to_string(),
        ],
        version,
        pin_file: paths::SHFMT_VERSION_FILE,
    }
}

pub fn git_cliff() -> Tool {
    cargo_tool(
        "git-cliff",
        paths::GIT_CLIFF_VERSION,
        paths::GIT_CLIFF_VERSION_FILE,
    )
}

pub fn committed() -> Tool {
    cargo_tool(
        "committed",
        paths::COMMITTED_VERSION,
        paths::COMMITTED_VERSION_FILE,
    )
}

/// Whether what the program printed for `--version` is the pinned version.
fn is_pinned(printed: Option<&str>, expected: &str) -> bool {
    printed == Some(expected)
}

/// What to say when the tool is missing or is another version.
fn message(tool: &Tool, printed: Option<&str>) -> String {
    format!(
        "{} {} is required ({}). Install it with one of:\n  {}\nThe version is \
         pinned in {}.",
        tool.program,
        tool.version,
        printed.unwrap_or(&format!("{} is not installed", tool.program)),
        tool.installs.join("\n  "),
        tool.pin_file
    )
}

/// Checks that the tool is installed at its pinned version.
pub fn ensure(tool: &Tool) -> Result {
    let printed = process::version_of(tool.program, &Env::new());
    if is_pinned(printed.as_deref(), &tool.expected) {
        Ok(())
    } else {
        Err(message(tool, printed.as_deref()).into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_pinned_version_passes() {
        assert!(is_pinned(Some("dprint 0.63.0"), "dprint 0.63.0"));
        assert!(!is_pinned(Some("dprint 0.62.0"), "dprint 0.63.0"));
        assert!(!is_pinned(None, "dprint 0.63.0"));
    }

    #[test]
    fn the_message_says_what_is_wrong_and_how_to_fix_it() {
        let tool = dprint();
        let missing = message(&tool, None);
        assert!(missing.contains("dprint is not installed"), "{missing}");
        assert!(missing
            .contains(&format!("cargo binstall dprint@{}", tool.version)));
        assert!(missing.contains(paths::DPRINT_VERSION_FILE));
        let other = message(&shfmt(), Some("v3.0.0"));
        assert!(other.contains("(v3.0.0)"), "{other}");
        assert!(other.contains(paths::SHFMT_SHA256_FILE));
    }

    #[test]
    fn every_tool_names_its_version_pin_and_the_setup_script() {
        for tool in [dprint(), shfmt(), git_cliff(), committed()] {
            let text = message(&tool, None);
            assert!(text.contains(paths::SETUP_SCRIPT), "{}", tool.program);
            assert!(text.contains(&tool.version), "{text}");
            assert!(text.contains(tool.pin_file), "{text}");
            assert!(tool.expected.contains(&tool.version));
        }
    }
}

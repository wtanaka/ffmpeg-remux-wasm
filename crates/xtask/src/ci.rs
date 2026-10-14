//! `cargo xtask ci [lint|build]`: everything CI checks, so that the workflows
//! only call this and a contributor can run what CI runs.

use crate::task::CiPart;
use crate::{commits, fmt, notes, package, process, wasm, Result};

/// clippy, with every warning an error.
const CLIPPY: [&str; 6] = [
    "clippy",
    "--workspace",
    "--all-targets",
    "--",
    "-D",
    "warnings",
];

/// shellcheck, following the scripts that are sourced from the others.
fn shellcheck_args(scripts: &[String]) -> Vec<String> {
    let mut args: Vec<String> = ["-x", "-P", "SCRIPTDIR"]
        .iter()
        .map(|arg| arg.to_string())
        .collect();
    args.extend(scripts.iter().cloned());
    args
}

/// Formatting, shell scripts (shellcheck), clippy, the release-notes
/// configuration and the commit messages since the last release.
fn lint() -> Result {
    fmt::run(true)?;
    process::run("shellcheck", &shellcheck_args(&fmt::shell_scripts()?))?;
    process::run("cargo", &CLIPPY)?;
    notes::check_config()?;
    commits::check()
}

/// The tests, the module, and the project zip.
fn build() -> Result {
    process::run("cargo", &["test", "--workspace"])?;
    wasm::run()?;
    package::run(None)
}

pub fn run(part: &CiPart) -> Result {
    match part {
        CiPart::All => {
            lint()?;
            build()
        }
        CiPart::Lint => lint(),
        CiPart::Build => build(),
    }
}

#[cfg(test)]
mod tests {
    use super::shellcheck_args;

    #[test]
    fn shellcheck_gets_its_flags_then_the_scripts() {
        let args = shellcheck_args(&[".ci/a.sh".into(), ".ci/b.sh".into()]);
        assert_eq!(args, ["-x", "-P", "SCRIPTDIR", ".ci/a.sh", ".ci/b.sh"]);
    }
}

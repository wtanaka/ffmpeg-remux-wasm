//! The task list, printed by `cargo xtask` (or `cargo xtask help`).

use crate::paths;

/// How each task is written (its name, then its arguments), in the order they
/// are listed. The argument parser uses these in its usage errors.
const USAGES: [&str; 10] = [
    "wasm",
    "ci [lint|build]",
    "fmt [--check]",
    "clean [--all]",
    "check-exports [exports.txt [module.wasm]]",
    "check-tag <tag>",
    "release <commit sha>",
    "download-ci-build <commit sha>",
    "publish-release <tag>",
    "package [output.zip]",
];

/// The task's name: the first word of how it is written.
fn name_of(usage: &str) -> &str {
    usage.split(' ').next().unwrap_or(usage)
}

/// How the named task is written, or `None` if there is no such task.
pub fn usage_of(name: &str) -> Option<&'static str> {
    USAGES.iter().copied().find(|usage| name_of(usage) == name)
}

/// What the named task does.
fn about(name: &str) -> String {
    let (dist, build, exports, module) = (
        paths::DIST_DIR,
        paths::BUILD_DIR,
        paths::EXPORTS_LIST,
        paths::MODULE,
    );
    match name {
        "wasm" => format!(
            "Builds {} into {dist}/.\n\
             Needs the Emscripten SDK; the first build takes a few minutes.",
            paths::MODULE_NAME
        ),
        "ci" => {
            "Everything CI checks, so you can run it yourself. `lint`: fmt \
                 --check,\nshellcheck, clippy, the release notes' \
                 configuration and the commit\nmessages since the last \
                 release (Conventional Commits, 80 columns). `build`: the tests, `wasm` \
                 and `package`.\nDefault: both. The workflows call this."
                .to_string()
        }
        "fmt" => "Formats Rust (rustfmt), Markdown/JSON/TOML/YAML (dprint) \
                  and the\nshell scripts (shfmt). If the pinned dprint or \
                  shfmt is missing, says\nhow to install it. With --check, \
                  only reports what would change."
            .to_string(),
        "clean" => format!(
            "Removes {dist}/ (the release files).\n\
             With --all, also {build}/ (the FFmpeg download and objects)\n\
             and cargo's target directory (`cargo clean`; honours\n\
             CARGO_TARGET_DIR)."
        ),
        "check-exports" => format!(
            "Checks that a built module exports every function in the list.\n\
             Defaults: {exports} and {module}.\n\
             `wasm` runs this for you."
        ),
        "check-tag" => {
            "Fails unless the tag is v<version>, where <version> is \
                        `version`\nin the root Cargo.toml. The release \
                        workflow runs this."
                .to_string()
        }
        "release" => "Tags the commit v<version> (`version` in the root \
                      Cargo.toml), waits for the\nrelease workflow, then \
                      downloads the module and source and checks their \
                      attestations.\nThe full commit hash is required, never \
                      taken from the checkout. Before tagging it\nchecks, on \
                      GitHub: the hash is on the remote and on the default \
                      branch, its\nCargo.toml has this `version`, and CI \
                      passed on it. Safe to run again. Needs the gh CLI."
            .to_string(),
        "download-ci-build" => format!(
            "Downloads the build that CI made for the commit into {dist}/\n\
             and checks it against {}. Fails if CI has not passed for\n\
             that commit. Needs the gh CLI. The release workflow runs this.",
            paths::SUMS_NAME
        ),
        "publish-release" => format!(
            "Publishes {dist}/ as the GitHub release for the tag. The notes are\n\
             written from the commits since the previous tag with git-cliff\n\
             (Conventional Commits), then a line about the build. Safe to run\n\
             again: the release is created once, and its notes and files are\n\
             replaced. Needs the gh CLI and git-cliff; the tag and the history\n\
             must be in the checkout. The release workflow runs this."
        ),
        "package" => format!(
            "Zips the project's files for uploading somewhere that takes a \
             zip.\nDefault: <cargo target dir>/{}.",
            paths::PACKAGE_ZIP
        ),
        other => unreachable!("no description for the task {other}"),
    }
}

/// The help text.
pub fn text() -> String {
    let mut out =
        String::from("usage: cargo xtask <task> [arguments]\n\ntasks:\n");
    for usage in USAGES {
        out.push_str(&format!("\n  {usage}\n"));
        for line in about(name_of(usage)).lines() {
            out.push_str(&format!("      {line}\n"));
        }
    }
    out
}

/// A pointer to the list, for an error message.
pub const HINT: &str = "run `cargo xtask` to list the tasks";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_task_has_a_description() {
        for usage in USAGES {
            assert!(!about(name_of(usage)).is_empty(), "{usage}");
        }
    }

    #[test]
    fn usages_are_found_by_name() {
        assert_eq!(usage_of("clean"), Some("clean [--all]"));
        assert_eq!(usage_of("nope"), None);
    }

    #[test]
    fn the_text_lists_every_task() {
        let text = text();
        for usage in USAGES {
            assert!(text.contains(usage), "{usage}");
        }
    }
}

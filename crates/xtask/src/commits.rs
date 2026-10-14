//! Checks the commit messages: Conventional Commits (`feat: ...`,
//! `fix(exports): ...`), because the release notes are written from them (see
//! notes.rs), and lines no longer than 80 columns. committed does the checking
//! (see .github/committed.toml) on each commit since the latest release tag: the
//! ones already released can't change, and a range from the latest tag doesn't
//! depend on what a push or a pull request is based on. Merge commits, such as
//! the one a pull request's checkout is, are not checked: nobody writes them.

use crate::process::args;
use crate::{paths, process, tools, Result};

/// `git rev-list` arguments: the commits after the tag, or all of them without
/// one, except merges.
fn list_args(tag: Option<&str>) -> Vec<String> {
    let range = tag.map_or("HEAD".to_string(), |tag| format!("{tag}..HEAD"));
    args(&["rev-list", "--no-merges", &range])
}

/// committed arguments: check one commit.
fn check_args(sha: &str) -> Vec<String> {
    args(&["--config", paths::COMMITTED_CONFIG, sha])
}

/// The commits in `git rev-list` output.
fn commits_in(listing: &str) -> Vec<&str> {
    listing
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect()
}

/// What to show about one commit: nothing if committed passed it, otherwise
/// what it printed.
fn complaint(passed: bool, stdout: &str, stderr: &str) -> Option<String> {
    (!passed).then(|| format!("{stdout}{stderr}"))
}

/// The result, for the end of the check.
fn summary(total: usize, failed: usize) -> std::result::Result<String, String> {
    if failed == 0 {
        Ok(format!(
            "{total} commit messages are in the expected format"
        ))
    } else {
        Err(format!(
            "{failed} of {total} commit messages are not in the expected \
             format (see docs/CONTRIBUTING.md): rewrite them, then run this \
             again"
        ))
    }
}

/// The latest release tag reachable from the checkout, if it has any.
fn latest_tag() -> Option<String> {
    process::text("git", &["describe", "--tags", "--abbrev=0"])
        .ok()
        .filter(|tag| !tag.is_empty())
}

pub fn check() -> Result {
    tools::ensure(&tools::committed())?;
    let Some(tag) = latest_tag() else {
        // Without a tag there is no telling which commits are new, and checking
        // them all would fail on commits from before the convention.
        println!(
            "commit messages not checked: no release tag in this checkout \
             (fetch the tags to check them)"
        );
        return Ok(());
    };
    let listing = process::text("git", &list_args(Some(&tag)))?;
    let shas = commits_in(&listing);
    let mut failed = 0;
    for sha in &shas {
        let output = process::output("committed", &check_args(sha))?;
        let said = |bytes: &[u8]| String::from_utf8_lossy(bytes).into_owned();
        if let Some(text) = complaint(
            output.status.success(),
            &said(&output.stdout),
            &said(&output.stderr),
        ) {
            failed += 1;
            print!("{text}");
        }
    }
    println!("{}", summary(shas.len(), failed)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_are_left_out_and_the_range_starts_at_the_tag() {
        assert_eq!(
            list_args(Some("v0.0.1")),
            ["rev-list", "--no-merges", "v0.0.1..HEAD"]
        );
        assert_eq!(list_args(None), ["rev-list", "--no-merges", "HEAD"]);
    }

    #[test]
    fn a_commit_is_checked_with_the_repositorys_configuration() {
        assert_eq!(
            check_args("abc"),
            ["--config", paths::COMMITTED_CONFIG, "abc"]
        );
        assert_eq!(commits_in("a1\n\n b2 \n"), ["a1", "b2"]);
        assert!(commits_in("").is_empty());
    }

    #[test]
    fn only_a_failed_check_has_something_to_show() {
        assert_eq!(complaint(true, "x", "y"), None);
        assert_eq!(
            complaint(false, "a: error\n", "b"),
            Some("a: error\nb".into())
        );
    }

    #[test]
    fn the_summary_counts_the_failures() {
        assert!(summary(3, 0).unwrap().contains("3 commit messages"));
        let error = summary(3, 2).unwrap_err();
        assert!(error.contains("2 of 3"), "{error}");
    }
}

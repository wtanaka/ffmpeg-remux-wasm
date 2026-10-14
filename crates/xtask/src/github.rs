//! The GitHub side of a release, through the `gh` CLI (which needs GH_TOKEN, or
//! a login): the release workflow publishes the build that CI made for the
//! tagged commit.

use crate::process::args;
use crate::{checksums, gh, io, notes, paths, Result};

/// The CI workflow, and the artifact its build job uploads.
const CI_WORKFLOW: &str = "ci.yml";
const ARTIFACT: &str = "ffmpeg-remux-wasm";

/// `gh` arguments: the id of a successful CI run for the commit.
fn list_run_args(sha: &str) -> Vec<String> {
    args(&[
        "run",
        "list",
        "--workflow",
        CI_WORKFLOW,
        "--commit",
        sha,
        "--status",
        "success",
        "--json",
        "databaseId",
        "--jq",
        ".[0].databaseId",
    ])
}

/// The run id in `gh run list`'s output, if there was a run.
pub fn parse_run_id(printed: &str) -> Option<String> {
    let id = printed.trim();
    (!id.is_empty() && id != "null").then(|| id.to_string())
}

/// `gh` arguments: download the build artifact of a run into the dist folder.
fn download_args(run_id: &str) -> Vec<String> {
    args(&[
        "run",
        "download",
        run_id,
        "--name",
        ARTIFACT,
        "--dir",
        paths::DIST_DIR,
    ])
}

/// The line about the build that ends every release's notes.
fn build_line(emscripten: &str) -> String {
    format!(
        "{} built from the FFmpeg source in this release, with Emscripten \
         {}. See {}.",
        paths::MODULE_NAME,
        emscripten.trim(),
        paths::DOC_NAME
    )
}

/// The release notes: what changed (written from the commits), then the line
/// about the build.
fn compose_notes(changes: &str, emscripten: &str) -> String {
    let changes = changes.trim();
    if changes.is_empty() {
        build_line(emscripten)
    } else {
        format!("{changes}\n\n{}", build_line(emscripten))
    }
}

/// `gh` arguments: replace the notes of the tag's release.
fn edit_args(tag: &str, notes: &str) -> Vec<String> {
    args(&["release", "edit", tag, "--notes", notes])
}

/// `gh` arguments: create the release for the tag.
fn create_args(tag: &str, notes: &str) -> Vec<String> {
    args(&["release", "create", tag, "--title", tag, "--notes", notes])
}

/// The files to upload, as paths from the root: everything in the dist folder
/// except hidden files.
fn release_files(names: &[String]) -> Vec<String> {
    let mut files: Vec<String> = names
        .iter()
        .filter(|name| !name.starts_with('.'))
        .map(|name| format!("{}/{name}", paths::DIST_DIR))
        .collect();
    files.sort();
    files
}

/// `gh` arguments: add the files to the tag's release, replacing any that are
/// there.
fn upload_args(tag: &str, files: &[String]) -> Vec<String> {
    let mut all = args(&["release", "upload", tag]);
    all.extend(files.iter().cloned());
    all.push("--clobber".to_string());
    all
}

/// The id of a successful CI run for the commit, if there is one.
pub fn ci_run_for(sha: &str) -> Result<Option<String>> {
    Ok(parse_run_id(&gh::text(&list_run_args(sha))?))
}

/// The message for a commit that CI has not passed on.
pub fn no_ci_run_message(sha: &str) -> String {
    format!(
        "no successful {CI_WORKFLOW} run for {sha}: wait for CI to pass on \
         this commit (or re-run it), then run this again"
    )
}

/// `cargo xtask download-ci-build <commit sha>`: downloads the build that CI
/// made for the commit into ffmpeg/dist/ and checks it against its SHA256SUMS.
/// Fails if CI has not passed for that commit.
pub fn download_ci_build(sha: &str) -> Result {
    let Some(run_id) = ci_run_for(sha)? else {
        return Err(no_ci_run_message(sha).into());
    };
    let dist = paths::abs(paths::DIST_DIR);
    io::remove_if_exists(&dist)?;
    gh::run(&download_args(&run_id))?;
    checksums::verify(&dist)
}

/// `cargo xtask publish-release <tag>`: publishes ffmpeg/dist/ as the GitHub
/// release for the tag. Safe to run again: the release is created once, and
/// its files are added or replaced.
pub fn publish_release(tag: &str) -> Result {
    let dist = paths::abs(paths::DIST_DIR);
    let names = io::list_dir(&dist).map_err(|e| {
        format!(
            "{e} (run `cargo xtask wasm` or `cargo xtask download-ci-build \
             <sha>` first)"
        )
    })?;
    let files = release_files(&names);
    if files.is_empty() {
        return Err(format!("{} is empty", dist.display()).into());
    }
    let emscripten =
        io::read_to_string(&paths::abs(paths::EMSCRIPTEN_VERSION_FILE))?;
    let notes = compose_notes(&notes::latest()?, &emscripten);
    // A re-run replaces the notes, so they always match the commits.
    if gh::output(&args(&["release", "view", tag]))?
        .status
        .success()
    {
        gh::run(&edit_args(tag, &notes))?;
    } else {
        gh::run(&create_args(tag, &notes))?;
    }
    gh::run(&upload_args(tag, &files))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_run_list_asks_for_a_successful_run_of_the_commit() {
        let args = list_run_args("abc123");
        assert!(args.windows(2).any(|w| w == ["--commit", "abc123"]));
        assert!(args.windows(2).any(|w| w == ["--status", "success"]));
        assert!(args.windows(2).any(|w| w == ["--workflow", CI_WORKFLOW]));
    }

    #[test]
    fn no_run_is_empty_or_null_output() {
        assert_eq!(parse_run_id("12345\n").as_deref(), Some("12345"));
        assert_eq!(parse_run_id(""), None);
        assert_eq!(parse_run_id("null\n"), None);
    }

    #[test]
    fn the_download_goes_into_the_dist_folder() {
        let args = download_args("99");
        assert!(args.windows(2).any(|w| w == ["--dir", paths::DIST_DIR]));
        assert!(args.windows(2).any(|w| w == ["--name", ARTIFACT]));
    }

    #[test]
    fn hidden_files_are_not_released() {
        let names = vec![
            "ffmpeg-remux.wasm".to_string(),
            ".inputs".to_string(),
            "SHA256SUMS".to_string(),
        ];
        assert_eq!(
            release_files(&names),
            [
                format!("{}/SHA256SUMS", paths::DIST_DIR),
                format!("{}/ffmpeg-remux.wasm", paths::DIST_DIR),
            ]
        );
    }

    #[test]
    fn the_upload_replaces_existing_files() {
        let args = upload_args("v1", &["a".to_string(), "b".to_string()]);
        assert_eq!(args, ["release", "upload", "v1", "a", "b", "--clobber"]);
    }

    #[test]
    fn the_notes_are_the_changes_then_the_build_line() {
        let notes = compose_notes("### Added\n- A thing\n", "4.0.10\n");
        assert!(notes.starts_with("### Added\n- A thing\n\n"), "{notes}");
        assert!(notes.ends_with("Emscripten 4.0.10. See FFMPEG-BUILD.md."));
        // Without changes, just the build line.
        assert_eq!(compose_notes("  \n", "4.0.10"), build_line("4.0.10"));
        assert_eq!(create_args("v1", "n")[2], "v1");
        assert_eq!(
            edit_args("v1", "n"),
            ["release", "edit", "v1", "--notes", "n"]
        );
    }
}

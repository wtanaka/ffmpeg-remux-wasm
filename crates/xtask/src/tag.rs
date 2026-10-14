//! `cargo xtask release <commit sha>`: tags the release, waits for the release
//! workflow, and checks the attestations of what it published, through the `gh`
//! CLI (which must be logged in with permission to create tags).
//!
//! The commit is always given explicitly, as a full hash, and everything about
//! it is checked on GitHub, never in the local checkout: the facts are gathered
//! first (`Facts`), then one pure function (`assess`) decides whether it is
//! safe to tag and says what to do.

use crate::process::args;
use crate::{gh, github, io, paths, release, Result};
use std::time::Duration;

const RELEASE_WORKFLOW: &str = "release.yml";
/// How long to wait for the release workflow to start after the tag is made.
const START_ATTEMPTS: u32 = 24;
const START_WAIT: Duration = Duration::from_secs(5);

/// A hash, a version or the like, or what went wrong asking GitHub for it.
type Fact<T> = std::result::Result<T, String>;

/// Where the commit is, compared with the default branch.
#[derive(Debug, PartialEq)]
enum Relation {
    /// It is the tip of the default branch.
    Tip,
    /// It is on the default branch, this many commits before the tip.
    Behind(u64),
}

/// What a tag lookup found.
#[derive(Debug, PartialEq)]
enum TagState {
    Absent,
    /// The tag exists, on this commit.
    At(String),
}

/// Everything asked of GitHub (and of the local manifest) about the release.
struct Facts {
    /// The commit that was asked for.
    sha: String,
    /// The version in the local Cargo.toml.
    version: String,
    /// The hash GitHub gives for that commit.
    on_remote: Fact<String>,
    /// `gh api compare <default>...<sha>`: its status and how far behind.
    relation: Fact<String>,
    /// The root Cargo.toml as it is at that commit.
    remote_manifest: Fact<String>,
    /// The id of a successful CI run for that commit, if there is one.
    ci_run: Fact<Option<String>>,
    /// The state of the release tag.
    tag: Fact<TagState>,
}

/// What `assess` decided.
#[derive(Debug, PartialEq)]
struct Plan {
    /// Whether the tag has to be created (it may already be there).
    create_tag: bool,
    /// Things worth saying, though nothing is wrong.
    notes: Vec<String>,
}

/// A full commit hash: 40 hexadecimal digits, as lowercase. A branch name or a
/// short hash is refused, so that what is tagged is exactly what was asked for.
fn normalize_sha(sha: &str) -> Fact<String> {
    let sha = sha.trim().to_ascii_lowercase();
    if sha.len() == 40 && sha.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(sha)
    } else {
        Err(format!(
            "{sha:?} is not a full commit hash (40 hexadecimal digits): a \
             branch name or a short hash is not accepted, so that what is \
             tagged is what you asked for"
        ))
    }
}

/// A plain version such as `1.2.3`, optionally with a `-pre-release` part: not
/// anything that would make an odd tag name.
fn check_version(version: &str) -> Fact<()> {
    let (core, rest) = match version.split_once(['-', '+']) {
        Some((core, rest)) => (core, Some(rest)),
        None => (version, None),
    };
    let numbers: Vec<&str> = core.split('.').collect();
    let plain = numbers.len() == 3
        && numbers
            .iter()
            .all(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
    let extra = rest.is_none_or(|rest| {
        !rest.is_empty()
            && rest
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
    });
    if plain && extra {
        Ok(())
    } else {
        Err(format!(
            "{version:?} is not a version like 1.2.3 (or 1.2.3-rc.1): fix \
             `version` in {}",
            paths::ROOT_MANIFEST
        ))
    }
}

/// Reads `gh api compare`'s `status behind_by` line.
fn parse_relation(printed: &str) -> Fact<Relation> {
    let mut words = printed.split_whitespace();
    match (words.next(), words.next().map(str::parse::<u64>)) {
        (Some("identical"), _) => Ok(Relation::Tip),
        (Some("behind"), Some(Ok(n))) => Ok(Relation::Behind(n)),
        (Some(status @ ("ahead" | "diverged")), _) => Err(format!(
            "the commit is {status} of the default branch, not on it: only \
             commits that are on the default branch can be released"
        )),
        _ => Err(format!("could not read how the commit relates to the default branch: {printed:?}")),
    }
}

/// Reads a tag lookup: the commit it printed, a not-found error, or another
/// failure.
fn parse_tag_state(
    succeeded: bool,
    stdout: &str,
    stderr: &str,
) -> Fact<TagState> {
    if succeeded {
        Ok(TagState::At(stdout.trim().to_string()))
    } else if stderr.contains("404") || stderr.contains("Not Found") {
        Ok(TagState::Absent)
    } else {
        Err(format!("could not look up the tag: {}", stderr.trim()))
    }
}

/// Decides, from the facts, whether the commit may be tagged: every check that
/// protects a release, in the order they are worth reporting.
fn assess(facts: &Facts) -> Fact<Plan> {
    check_version(&facts.version)?;

    let on_remote = facts.on_remote.as_ref().map_err(|e| {
        format!(
            "GitHub does not have the commit {}: push it first ({e})",
            facts.sha
        )
    })?;
    if *on_remote != facts.sha {
        return Err(format!(
            "GitHub resolved {} to {on_remote}, not the same commit",
            facts.sha
        ));
    }

    let mut notes = Vec::new();
    match parse_relation(facts.relation.as_ref().map_err(String::clone)?)? {
        Relation::Tip => {}
        Relation::Behind(n) => notes.push(format!(
            "the default branch has {n} newer commit{}",
            if n == 1 { "" } else { "s" }
        )),
    }

    let remote_manifest =
        facts.remote_manifest.as_ref().map_err(String::clone)?;
    let remote_version =
        release::version_in(remote_manifest).ok_or_else(|| {
            format!("{} at {} has no version", paths::ROOT_MANIFEST, facts.sha)
        })?;
    if remote_version != facts.version {
        return Err(format!(
            "{} has version {remote_version} at {} on GitHub, but {} here: \
             push the version first, or check which commit you meant",
            paths::ROOT_MANIFEST,
            facts.sha,
            facts.version
        ));
    }

    if facts.ci_run.as_ref().map_err(String::clone)?.is_none() {
        return Err(github::no_ci_run_message(&facts.sha));
    }

    let tag = release::tag_for(&facts.version);
    match facts.tag.as_ref().map_err(String::clone)? {
        TagState::Absent => Ok(Plan {
            create_tag: true,
            notes,
        }),
        TagState::At(at) if *at == facts.sha => {
            notes.push(format!("{tag} already points to this commit"));
            Ok(Plan {
                create_tag: false,
                notes,
            })
        }
        TagState::At(at) => Err(format!(
            "{tag} already exists, on {at}, not {}; delete it on GitHub first \
             if it was a mistake",
            facts.sha
        )),
    }
}

/// `gh` arguments: the repository's name.
fn repo_args() -> Vec<String> {
    args(&[
        "repo",
        "view",
        "--json",
        "nameWithOwner,defaultBranchRef",
        "--jq",
        ".nameWithOwner + \" \" + .defaultBranchRef.name",
    ])
}

/// The repository and its default branch, from `repo_args`' output.
fn parse_repo(printed: &str) -> Fact<(String, String)> {
    match printed.split_whitespace().collect::<Vec<_>>()[..] {
        [repo, branch] => Ok((repo.to_string(), branch.to_string())),
        _ => Err(format!("could not read the repository from {printed:?}")),
    }
}

/// `gh` arguments: the hash GitHub has for a commit.
fn commit_args(repo: &str, sha: &str) -> Vec<String> {
    args(&[
        "api",
        &format!("repos/{repo}/commits/{sha}"),
        "--jq",
        ".sha",
    ])
}

/// `gh` arguments: how the commit relates to the default branch.
fn compare_args(repo: &str, branch: &str, sha: &str) -> Vec<String> {
    args(&[
        "api",
        &format!("repos/{repo}/compare/{branch}...{sha}"),
        "--jq",
        ".status + \" \" + (.behind_by | tostring)",
    ])
}

/// `gh` arguments: the root Cargo.toml as it is at a commit.
fn manifest_args(repo: &str, sha: &str) -> Vec<String> {
    args(&[
        "api",
        "-H",
        "Accept: application/vnd.github.raw",
        &format!("repos/{repo}/contents/{}?ref={sha}", paths::ROOT_MANIFEST),
    ])
}

/// `gh` arguments: the commit a tag points to.
fn lookup_args(repo: &str, tag: &str) -> Vec<String> {
    args(&[
        "api",
        &format!("repos/{repo}/git/ref/tags/{tag}"),
        "--jq",
        ".object.sha",
    ])
}

/// `gh` arguments: create the tag on a commit.
fn create_args(repo: &str, tag: &str, sha: &str) -> Vec<String> {
    args(&[
        "api",
        &format!("repos/{repo}/git/refs"),
        "-f",
        &format!("ref=refs/tags/{tag}"),
        "-f",
        &format!("sha={sha}"),
    ])
}

/// `gh` arguments: the newest run of the release workflow for a tag.
fn find_run_args(tag: &str) -> Vec<String> {
    args(&[
        "run",
        "list",
        "--workflow",
        RELEASE_WORKFLOW,
        "--branch",
        tag,
        "--limit",
        "1",
        "--json",
        "databaseId",
        "--jq",
        ".[0].databaseId",
    ])
}

/// `gh` arguments: download the attested files of a release.
fn download_args(repo: &str, tag: &str, dir: &str) -> Vec<String> {
    args(&[
        "release",
        "download",
        tag,
        "--repo",
        repo,
        "--pattern",
        paths::MODULE_NAME,
        "--pattern",
        "ffmpeg-*.tar.xz",
        "--dir",
        dir,
        "--clobber",
    ])
}

/// `gh` arguments: check a file's attestation.
fn verify_args(file: &str, repo: &str) -> Vec<String> {
    args(&["attestation", "verify", file, "--repo", repo])
}

/// The downloaded files whose attestations are checked: the module and the
/// FFmpeg source (the two the release workflow attests).
fn attested_files(names: &[String]) -> Vec<String> {
    names
        .iter()
        .filter(|name| name.ends_with(".wasm") || name.ends_with(".tar.xz"))
        .cloned()
        .collect()
}

/// Asks GitHub everything `assess` needs. A question that fails is a fact too
/// (its message), so that `assess` can report the most important problem.
fn gather(repo: &str, branch: &str, sha: &str, version: &str) -> Facts {
    let ask = |a: Vec<String>| gh::text(&a).map_err(|e| e.to_string());
    let tag = release::tag_for(version);
    let tag_lookup =
        gh::output(&lookup_args(repo, &tag)).map_err(|e| e.to_string());
    Facts {
        sha: sha.to_string(),
        version: version.to_string(),
        on_remote: ask(commit_args(repo, sha)),
        relation: ask(compare_args(repo, branch, sha)),
        remote_manifest: ask(manifest_args(repo, sha)),
        ci_run: github::ci_run_for(sha).map_err(|e| e.to_string()),
        tag: tag_lookup.and_then(|out| {
            parse_tag_state(
                out.status.success(),
                &String::from_utf8_lossy(&out.stdout),
                &String::from_utf8_lossy(&out.stderr),
            )
        }),
    }
}

/// Waits for the release workflow's run for the tag to appear.
fn wait_for_run(tag: &str) -> Result<String> {
    println!("waiting for the {RELEASE_WORKFLOW} workflow to start for {tag}");
    for _ in 0..START_ATTEMPTS {
        if let Some(id) = github::parse_run_id(&gh::text(&find_run_args(tag))?)
        {
            return Ok(id);
        }
        std::thread::sleep(START_WAIT);
    }
    Err(format!(
        "the {RELEASE_WORKFLOW} workflow did not start for {tag} within {} \
         seconds; check the Actions tab",
        u64::from(START_ATTEMPTS) * START_WAIT.as_secs()
    )
    .into())
}

/// Downloads the release's attested files and checks their attestations.
fn verify_attestations(repo: &str, tag: &str) -> Result {
    let dir = std::env::temp_dir()
        .join(format!("xtask-release-{tag}-{}", std::process::id()));
    io::remove_if_exists(&dir)?;
    io::create_dir_all(&dir)?;
    let result = (|| -> Result {
        gh::run(&download_args(repo, tag, &dir.to_string_lossy()))?;
        let files = attested_files(&io::list_dir(&dir)?);
        if files.is_empty() {
            return Err(
                format!("{tag} has no module or source to verify").into()
            );
        }
        for name in files {
            let path = dir.join(&name).to_string_lossy().into_owned();
            gh::run(&verify_args(&path, repo))?;
            println!("attestation verified: {name}");
        }
        Ok(())
    })();
    io::remove_if_exists(&dir)?;
    result
}

pub fn run(sha: &str) -> Result {
    let sha = normalize_sha(sha)?;
    let version = release::version()?;
    let (repo, branch) = parse_repo(&gh::text(&repo_args())?)?;

    let plan = assess(&gather(&repo, &branch, &sha, &version))?;
    let tag = release::tag_for(&version);
    println!("releasing {tag} of {repo} at {sha}");
    for note in &plan.notes {
        println!("note: {note}");
    }

    if plan.create_tag {
        gh::text(&create_args(&repo, &tag, &sha))?;
        println!("created {tag} on {sha}");
    }
    let run_id = wait_for_run(&tag)?;
    println!("waiting for the {RELEASE_WORKFLOW} run {run_id}");
    gh::run(&args(&["run", "watch", &run_id, "--exit-status"]))?;
    verify_attestations(&repo, &tag)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

    /// Facts for a release that is fine to make.
    fn good() -> Facts {
        Facts {
            sha: SHA.to_string(),
            version: "0.0.1".to_string(),
            on_remote: Ok(SHA.to_string()),
            relation: Ok("identical 0".to_string()),
            remote_manifest: Ok(
                "[workspace.package]\nversion = \"0.0.1\"\n".to_string()
            ),
            ci_run: Ok(Some("42".to_string())),
            tag: Ok(TagState::Absent),
        }
    }

    fn refused(facts: Facts) -> String {
        assess(&facts).unwrap_err()
    }

    #[test]
    fn a_good_release_creates_the_tag() {
        assert_eq!(
            assess(&good()),
            Ok(Plan {
                create_tag: true,
                notes: vec![]
            })
        );
    }

    #[test]
    fn only_a_full_hash_is_accepted() {
        assert_eq!(normalize_sha(&SHA.to_uppercase()).as_deref(), Ok(SHA));
        for bad in [
            "main",
            "abc123",
            "",
            &SHA[..39],
            &format!("{SHA}0"),
            "zzzz456789abcdef0123456789abcdef01234567",
        ] {
            assert!(normalize_sha(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn only_a_plain_version_makes_a_tag() {
        for good in ["0.0.1", "1.2.3", "1.2.3-rc.1", "10.20.30+build.5"] {
            assert!(check_version(good).is_ok(), "{good}");
        }
        for bad in [
            "",
            "1.2",
            "1.2.3.4",
            "v1.2.3",
            "1.2.x",
            "1.2.3-",
            "1.2.3 ; x",
        ] {
            assert!(check_version(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_commit_the_remote_lacks_is_refused() {
        let mut facts = good();
        facts.on_remote = Err("HTTP 422".to_string());
        assert!(refused(facts).contains("does not have the commit"));
        let mut facts = good();
        facts.on_remote = Ok("f".repeat(40));
        assert!(refused(facts).contains("not the same commit"));
    }

    #[test]
    fn only_commits_on_the_default_branch_are_released() {
        assert_eq!(parse_relation("identical 0"), Ok(Relation::Tip));
        assert_eq!(parse_relation("behind 3"), Ok(Relation::Behind(3)));
        assert!(parse_relation("ahead 0").is_err());
        assert!(parse_relation("diverged 2").is_err());
        assert!(parse_relation("").is_err());
        let mut facts = good();
        facts.relation = Ok("diverged 1".to_string());
        assert!(refused(facts).contains("not on it"));
        let mut facts = good();
        facts.relation = Ok("behind 2".to_string());
        assert_eq!(
            assess(&facts).unwrap().notes,
            ["the default branch has 2 newer commits"]
        );
    }

    #[test]
    fn the_version_on_the_remote_must_be_the_local_one() {
        let mut facts = good();
        facts.remote_manifest = Ok("version = \"0.0.2\"\n".to_string());
        let error = refused(facts);
        assert!(
            error.contains("0.0.2") && error.contains("0.0.1"),
            "{error}"
        );
        let mut facts = good();
        facts.remote_manifest = Ok("no version here".to_string());
        assert!(refused(facts).contains("has no version"));
    }

    #[test]
    fn a_commit_without_a_passing_ci_run_is_refused() {
        let mut facts = good();
        facts.ci_run = Ok(None);
        assert!(refused(facts).contains("no successful"));
    }

    #[test]
    fn an_existing_tag_is_accepted_only_on_the_same_commit() {
        let mut facts = good();
        facts.tag = Ok(TagState::At(SHA.to_string()));
        let plan = assess(&facts).unwrap();
        assert!(!plan.create_tag);
        assert!(plan.notes[0].contains("already points"));
        let mut facts = good();
        facts.tag = Ok(TagState::At("f".repeat(40)));
        assert!(refused(facts).contains("already exists"));
    }

    #[test]
    fn a_failed_question_is_reported_not_ignored() {
        let mut facts = good();
        facts.ci_run = Err("HTTP 500".to_string());
        assert!(refused(facts).contains("500"));
        let mut facts = good();
        facts.tag = Err("could not look up the tag: HTTP 401".to_string());
        assert!(refused(facts).contains("401"));
        let mut facts = good();
        facts.version = "banana".to_string();
        assert!(refused(facts).contains("not a version"));
    }

    #[test]
    fn answers_are_read() {
        assert_eq!(parse_repo("o/r main\n"), Ok(("o/r".into(), "main".into())));
        assert!(parse_repo("nonsense").is_err());
        assert_eq!(
            parse_tag_state(true, "abc\n", ""),
            Ok(TagState::At("abc".into()))
        );
        assert_eq!(
            parse_tag_state(false, "", "gh: Not Found (HTTP 404)"),
            Ok(TagState::Absent)
        );
        assert!(parse_tag_state(false, "", "HTTP 401")
            .unwrap_err()
            .contains("401"));
    }

    #[test]
    fn the_commands_name_the_repository_commit_and_tag() {
        assert_eq!(commit_args("o/r", "abc")[1], "repos/o/r/commits/abc");
        assert_eq!(
            compare_args("o/r", "main", "abc")[1],
            "repos/o/r/compare/main...abc"
        );
        assert!(manifest_args("o/r", "abc")
            .contains(&"repos/o/r/contents/Cargo.toml?ref=abc".to_string()));
        assert_eq!(lookup_args("o/r", "v1")[1], "repos/o/r/git/ref/tags/v1");
        let create = create_args("o/r", "v1", "abc");
        assert_eq!(create[1], "repos/o/r/git/refs");
        assert!(create.contains(&"ref=refs/tags/v1".to_string()));
        assert!(create.contains(&"sha=abc".to_string()));
    }

    #[test]
    fn the_release_run_and_attestations_are_found_by_tag() {
        let find = find_run_args("v1");
        assert!(find.windows(2).any(|w| w == ["--branch", "v1"]));
        assert!(find
            .windows(2)
            .any(|w| w == ["--workflow", RELEASE_WORKFLOW]));
        let download = download_args("o/r", "v1", "/tmp/x");
        assert!(download
            .windows(2)
            .any(|w| w == ["--pattern", paths::MODULE_NAME]));
        assert_eq!(
            verify_args("f", "o/r"),
            ["attestation", "verify", "f", "--repo", "o/r"]
        );
        let names = vec![
            "ffmpeg-remux.wasm".to_string(),
            "ffmpeg-7.1.1.tar.xz".to_string(),
            "SHA256SUMS".to_string(),
        ];
        assert_eq!(
            attested_files(&names),
            ["ffmpeg-remux.wasm", "ffmpeg-7.1.1.tar.xz"]
        );
    }
}

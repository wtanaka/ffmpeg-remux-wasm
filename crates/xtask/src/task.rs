//! What was asked for: the command line, parsed into a `Task`. Parsing is pure
//! (arguments in, a task or a usage message out), so every task's arguments are
//! checked here, in one place.

use crate::help;

/// Which part of `ci` to run.
#[derive(Debug, PartialEq)]
pub enum CiPart {
    All,
    Lint,
    Build,
}

#[derive(Debug, PartialEq)]
pub enum Task {
    Help,
    Wasm,
    Ci(CiPart),
    Fmt {
        check: bool,
    },
    Clean {
        all: bool,
    },
    CheckExports {
        list: Option<String>,
        module: Option<String>,
    },
    CheckTag(String),
    Release {
        sha: String,
    },
    DownloadCiBuild(String),
    PublishRelease(String),
    Package {
        output: Option<String>,
    },
}

/// The usage message for a task that was given arguments it doesn't take.
fn usage_error(name: &str, args: &[&str]) -> String {
    format!(
        "usage: cargo xtask {} (got: {})",
        help::usage_of(name).unwrap_or(name),
        args.join(" ")
    )
}

/// The task the command line asks for, or a message saying what is wrong.
pub fn parse(args: &[String]) -> Result<Task, String> {
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    let Some((&name, rest)) = words.split_first() else {
        return Ok(Task::Help);
    };
    let own = |text: &str| text.to_string();
    match (name, rest) {
        ("help" | "--help" | "-h", _) => Ok(Task::Help),
        ("wasm", []) => Ok(Task::Wasm),
        ("ci", []) => Ok(Task::Ci(CiPart::All)),
        ("ci", ["lint"]) => Ok(Task::Ci(CiPart::Lint)),
        ("ci", ["build"]) => Ok(Task::Ci(CiPart::Build)),
        ("fmt", []) => Ok(Task::Fmt { check: false }),
        ("fmt", ["--check"]) => Ok(Task::Fmt { check: true }),
        ("clean", []) => Ok(Task::Clean { all: false }),
        ("clean", ["--all"]) => Ok(Task::Clean { all: true }),
        ("check-exports", []) => Ok(Task::CheckExports {
            list: None,
            module: None,
        }),
        ("check-exports", [list]) => Ok(Task::CheckExports {
            list: Some(own(list)),
            module: None,
        }),
        ("check-exports", [list, module]) => Ok(Task::CheckExports {
            list: Some(own(list)),
            module: Some(own(module)),
        }),
        ("check-tag", [tag]) => Ok(Task::CheckTag(own(tag))),
        ("release", [sha]) => Ok(Task::Release { sha: own(sha) }),
        ("ci-download-build", [sha]) => Ok(Task::DownloadCiBuild(own(sha))),
        ("ci-publish-release", [tag]) => Ok(Task::PublishRelease(own(tag))),
        ("package", []) => Ok(Task::Package { output: None }),
        ("package", [output]) => Ok(Task::Package {
            output: Some(own(output)),
        }),
        (name, rest) if help::usage_of(name).is_some() => {
            Err(usage_error(name, rest))
        }
        (name, _) => Err(format!("unknown task `{name}`; {}", help::HINT)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_words(words: &[&str]) -> Result<Task, String> {
        parse(&words.iter().map(|w| w.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn no_arguments_asks_for_help() {
        assert_eq!(parse_words(&[]), Ok(Task::Help));
        assert_eq!(parse_words(&["--help"]), Ok(Task::Help));
    }

    #[test]
    fn tasks_with_flags() {
        assert_eq!(parse_words(&["fmt"]), Ok(Task::Fmt { check: false }));
        assert_eq!(
            parse_words(&["fmt", "--check"]),
            Ok(Task::Fmt { check: true })
        );
        assert_eq!(
            parse_words(&["clean", "--all"]),
            Ok(Task::Clean { all: true })
        );
        assert_eq!(parse_words(&["ci"]), Ok(Task::Ci(CiPart::All)));
        assert_eq!(parse_words(&["ci", "lint"]), Ok(Task::Ci(CiPart::Lint)));
    }

    #[test]
    fn tasks_with_arguments() {
        assert_eq!(
            parse_words(&["check-exports", "a.txt"]),
            Ok(Task::CheckExports {
                list: Some("a.txt".into()),
                module: None
            })
        );
        assert_eq!(
            parse_words(&["check-tag", "v1"]),
            Ok(Task::CheckTag("v1".into()))
        );
        assert_eq!(
            parse_words(&["package", "out.zip"]),
            Ok(Task::Package {
                output: Some("out.zip".into())
            })
        );
    }

    #[test]
    fn wrong_arguments_get_the_tasks_usage() {
        let error = parse_words(&["fmt", "--chek"]).unwrap_err();
        assert_eq!(error, "usage: cargo xtask fmt [--check] (got: --chek)");
        let error = parse_words(&["check-tag"]).unwrap_err();
        assert!(error.contains("check-tag <tag>"), "{error}");
        assert!(parse_words(&["wasm", "x"])
            .unwrap_err()
            .contains("(got: x)"));
    }

    #[test]
    fn release_requires_a_commit() {
        assert_eq!(
            parse_words(&["release", "abc"]),
            Ok(Task::Release { sha: "abc".into() })
        );
        let error = parse_words(&["release"]).unwrap_err();
        assert!(error.contains("release <commit sha>"), "{error}");
        assert!(parse_words(&["release", "a", "b"]).is_err());
    }

    #[test]
    fn an_unknown_task_points_to_the_list() {
        let error = parse_words(&["bogus"]).unwrap_err();
        assert!(error.contains("unknown task `bogus`"), "{error}");
        assert!(error.contains("cargo xtask"), "{error}");
    }
}

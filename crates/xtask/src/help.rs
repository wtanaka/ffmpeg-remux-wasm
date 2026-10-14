//! The task list, printed by `cargo xtask` (or `cargo xtask help`).

use crate::{graph, paths};

/// The width the help is wrapped to, after the indent.
const WIDTH: usize = 72;

/// A task: how it is written, how it relates to the others, where it can run.
struct Task {
    /// Its name, then its arguments. The argument parser uses this in its
    /// usage errors.
    usage: &'static str,
    /// The tasks whose results it needs, that it does not run.
    needs: &'static [&'static str],
    /// Where it can run, and why that matters.
    place: &'static str,
}

/// Every task. The help lists them in dependency order: each comes after the
/// ones it runs or needs, and where there is no such order, in this order.
const TASKS: [Task; 10] = [
    Task {
        usage: "check-exports [exports.txt [module.wasm]]",
        needs: &[],
        place: "anywhere, on a module that is already built; `wasm` runs it \
                as its last step",
    },
    Task {
        usage: "fmt [--check]",
        needs: &[],
        place: "anywhere",
    },
    Task {
        usage: "package [output.zip]",
        needs: &[],
        place: "anywhere",
    },
    Task {
        usage: "clean [--all]",
        needs: &[],
        place: "anywhere",
    },
    Task {
        usage: "check-tag <tag>",
        needs: &[],
        place: "anywhere (CI runs it on a release)",
    },
    Task {
        usage: "wasm",
        needs: &[],
        place: "anywhere; needs the Emscripten SDK",
    },
    Task {
        usage: "ci [lint|build]",
        needs: &[],
        place: "anywhere (CI runs it on every push and pull request)",
    },
    Task {
        usage: "release <commit sha>",
        needs: &["ci", "ci-publish-release"],
        place: "locally only: needs the gh CLI. Makes the tag that starts \
                `ci-publish-release` in CI, and waits for it",
    },
    Task {
        usage: "ci-download-build <commit sha>",
        needs: &["ci"],
        place: "in CI, or locally with the gh CLI",
    },
    Task {
        usage: "ci-publish-release <tag>",
        needs: &["check-tag", "ci-download-build"],
        place: "in CI, which attests the files first; locally (gh) it \
                publishes them unattested. `release` starts it",
    },
];

/// The task's name: the first word of how it is written.
fn name_of(usage: &str) -> &str {
    usage.split(' ').next().unwrap_or(usage)
}

impl Task {
    fn name(&self) -> &'static str {
        name_of(self.usage)
    }

    /// The tasks it runs itself (see graph.rs).
    fn runs(&self) -> Vec<&'static str> {
        graph::runs_of(self.name())
    }

    /// The tasks that must come before this one.
    fn before(&self) -> Vec<&'static str> {
        let mut names = self.runs();
        names.extend(self.needs);
        names
    }
}

/// How the named task is written, or `None` if there is no such task.
pub fn usage_of(name: &str) -> Option<&'static str> {
    TASKS
        .iter()
        .map(|task| task.usage)
        .find(|usage| name_of(usage) == name)
}

/// The tasks in dependency order: each after the ones it runs or needs, and
/// otherwise in the order of the table. Tasks caught in a cycle (the tests
/// check there are none) come last, in the order of the table.
fn in_order(tasks: &[Task]) -> Vec<&Task> {
    let mut placed: Vec<&Task> = Vec::new();
    while placed.len() < tasks.len() {
        let ready = tasks.iter().find(|task| {
            !placed.iter().any(|done| done.name() == task.name())
                && task
                    .before()
                    .iter()
                    .all(|dep| placed.iter().any(|done| done.name() == *dep))
        });
        match ready {
            Some(task) => placed.push(task),
            None => {
                let rest: Vec<&Task> = tasks
                    .iter()
                    .filter(|task| {
                        !placed.iter().any(|done| done.name() == task.name())
                    })
                    .collect();
                placed.extend(rest);
            }
        }
    }
    placed
}

/// What the named task does, as prose (the help wraps it). What it runs, needs
/// and where it can run are in the table, so they are not repeated here.
fn about(name: &str) -> String {
    let (dist, build) = (paths::DIST_DIR, paths::BUILD_DIR);
    match name {
        "wasm" => format!(
            "Builds {} into {dist}/. The first build takes a few minutes.",
            paths::MODULE_NAME
        ),
        "ci" => "What CI checks. `lint`: fmt --check, shellcheck, clippy, the \
                 release notes' configuration and the commit messages since \
                 the last release. `build`: the tests, `wasm` and `package`. \
                 Default: both."
            .to_string(),
        "fmt" => "Formats Rust (rustfmt), Markdown/JSON/TOML/YAML (dprint) \
                  and shell scripts (shfmt). `--check` only reports. A \
                  missing pinned tool says how to install it."
            .to_string(),
        "clean" => format!(
            "Removes {dist}/. `--all` also removes {build}/ and cargo's \
             target directory."
        ),
        "check-exports" => format!(
            "Checks that the module exports every function in the list. \
             Defaults: {} and {}.",
            paths::EXPORTS_LIST,
            paths::MODULE
        ),
        "check-tag" => "Fails unless the tag is v<version>, where <version> \
                        is `version` in the root Cargo.toml."
            .to_string(),
        "release" => "Tags the commit v<version>, waits for `ci-publish-release` \
                      to finish in CI, then checks the attestations of the module \
                      and source it published. The full commit hash is \
                      required, never taken from the checkout; the commit \
                      must be on the remote's default branch, have this \
                      `version` and have passed CI. Safe to run again."
            .to_string(),
        "ci-download-build" => format!(
            "Downloads the build CI made for the commit into {dist}/ and \
             checks it against {}. Fails if CI has not passed on the commit.",
            paths::SUMS_NAME
        ),
        "ci-publish-release" => format!(
            "Publishes {dist}/ as the release for the tag, with notes \
             written from the commits by git-cliff. Safe to run again: the \
             notes and files are replaced. Needs the tag and its history in \
             the checkout."
        ),
        "package" => format!(
            "Zips the project's files. Default: <cargo target dir>/{}.",
            paths::PACKAGE_ZIP
        ),
        other => unreachable!("no description for the task {other}"),
    }
}

/// Text broken into lines of at most `width` columns, at spaces.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        match lines.last_mut() {
            Some(line) if line.len() + 1 + word.len() <= width => {
                line.push(' ');
                line.push_str(word);
            }
            _ => lines.push(word.to_string()),
        }
    }
    lines
}

/// `label: text`, wrapped, the lines after the first lined up under the text.
fn labeled(label: &str, text: &str) -> Vec<String> {
    let indent = " ".repeat(label.len() + 2);
    wrap(text, WIDTH - label.len() - 2)
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let start = if i == 0 {
                format!("{label}: ")
            } else {
                indent.clone()
            };
            format!("{start}{line}")
        })
        .collect()
}

/// One task as the help shows it: its usage and description, what it runs and
/// needs, and where it can run.
fn render(task: &Task, about: &str, runs: &[&str]) -> String {
    let mut body = wrap(about, WIDTH);
    for (label, names) in [("runs", runs), ("needs", task.needs)] {
        if !names.is_empty() {
            body.extend(labeled(label, &names.join(", ")));
        }
    }
    body.extend(labeled("where", task.place));
    let mut out = format!("\n  {}\n", task.usage);
    for line in body {
        out.push_str(&format!("      {line}\n"));
    }
    out
}

/// The help text.
pub fn text() -> String {
    let mut out = String::from(
        "usage: cargo xtask <task> [arguments]\n\ntasks, in dependency order \
         (each comes after the tasks it runs or needs):\n",
    );
    for task in in_order(&TASKS) {
        out.push_str(&render(task, &about(task.name()), &task.runs()));
    }
    out
}

/// A pointer to the list, for an error message.
pub const HINT: &str = "run `cargo xtask` to list the tasks";

#[cfg(test)]
mod tests {
    use super::*;

    fn names(tasks: &[&Task]) -> Vec<&'static str> {
        tasks.iter().map(|task| task.name()).collect()
    }

    #[test]
    fn every_task_has_a_description() {
        for task in &TASKS {
            assert!(!about(task.name()).is_empty(), "{}", task.name());
        }
    }

    #[test]
    fn usages_are_found_by_name() {
        assert_eq!(usage_of("clean"), Some("clean [--all]"));
        assert_eq!(usage_of("nope"), None);
    }

    #[test]
    fn task_names_are_unique_and_every_dependency_is_a_task() {
        let all = names(&TASKS.iter().collect::<Vec<_>>());
        for (i, name) in all.iter().enumerate() {
            assert!(!all[..i].contains(name), "{name} is listed twice");
        }
        for task in &TASKS {
            for dep in task.before() {
                assert!(all.contains(&dep), "{} names {dep}", task.name());
                assert_ne!(dep, task.name(), "depends on itself");
            }
        }
    }

    #[test]
    fn every_task_comes_after_the_ones_it_depends_on() {
        let order = names(&in_order(&TASKS));
        assert_eq!(order.len(), TASKS.len(), "a task is missing or repeated");
        for task in &TASKS {
            let at = order.iter().position(|n| *n == task.name()).unwrap();
            for dep in task.before() {
                let dep_at = order.iter().position(|n| *n == dep).unwrap();
                assert!(
                    dep_at < at,
                    "{dep} should come before {}",
                    task.name()
                );
            }
        }
    }

    #[test]
    fn the_release_comes_last_and_the_plain_tasks_first() {
        let order = names(&in_order(&TASKS));
        assert_eq!(order.last(), Some(&"release"));
        assert_eq!(order[0], "check-exports");
        assert!(
            order.iter().position(|n| *n == "wasm")
                < order.iter().position(|n| *n == "ci")
        );
    }

    #[test]
    fn the_order_follows_the_table_where_nothing_decides() {
        let tasks = [
            Task {
                usage: "b",
                needs: &[],
                place: "",
            },
            Task {
                usage: "a",
                needs: &["b"],
                place: "",
            },
            Task {
                usage: "c",
                needs: &[],
                place: "",
            },
        ];
        assert_eq!(names(&in_order(&tasks)), ["b", "a", "c"]);
        // A cycle does not loop forever: its tasks come last, in table order.
        let cycle = [
            Task {
                usage: "x",
                needs: &[],
                place: "",
            },
            Task {
                usage: "y",
                needs: &[],
                place: "",
            },
        ];
        assert_eq!(names(&in_order(&cycle)), ["x", "y"]);
    }

    #[test]
    fn a_task_shows_what_it_runs_needs_and_where_it_runs() {
        let task = Task {
            usage: "t <x>",
            needs: &[],
            place: "anywhere",
        };
        let shown = render(&task, "Does it. Twice.", &["a", "b"]);
        assert_eq!(
            shown,
            "\n  t <x>\n      Does it. Twice.\n      runs: a, b\n      \
             where: anywhere\n"
        );
    }

    #[test]
    fn text_is_wrapped_at_spaces() {
        assert_eq!(wrap("aa bb cc", 5), ["aa bb", "cc"]);
        assert_eq!(wrap("aaaaaaa b", 3), ["aaaaaaa", "b"]);
        assert!(wrap("  ", 5).is_empty());
        let lines = labeled("where", &"word ".repeat(30));
        assert!(lines[0].starts_with("where: "));
        assert!(lines.len() > 1);
        // The text lines up under the first line's text.
        assert!(lines.iter().skip(1).all(|l| l.starts_with("       ")));
        assert!(lines.iter().skip(1).all(|l| !l.starts_with("        ")));
    }

    #[test]
    fn no_line_of_the_help_is_wider_than_80_columns() {
        for line in text().lines() {
            assert!(line.len() <= 80, "{line:?}");
        }
    }

    #[test]
    fn the_text_lists_every_task_with_its_place() {
        let text = text();
        for task in &TASKS {
            assert!(text.contains(task.usage), "{}", task.usage);
            // The place is wrapped, so look for its first words.
            let start: Vec<&str> = task.place.split(' ').take(3).collect();
            assert!(text.contains(&start.join(" ")), "{}", task.usage);
        }
    }
}

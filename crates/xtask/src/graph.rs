//! Which tasks a task runs. `main.rs` runs exactly these, and the help lists
//! exactly these, so the list cannot say something the code does not do.

use crate::task::{CiPart, Task};

/// The name a task is typed as.
pub fn name(task: &Task) -> &'static str {
    match task {
        Task::Help => "help",
        Task::Wasm => "wasm",
        Task::Ci(_) => "ci",
        Task::Fmt { .. } => "fmt",
        Task::Clean { .. } => "clean",
        Task::CheckExports { .. } => "check-exports",
        Task::CheckTag(_) => "check-tag",
        Task::Release { .. } => "release",
        Task::DownloadCiBuild(_) => "ci-download-build",
        Task::PublishRelease(_) => "ci-publish-release",
        Task::Package { .. } => "package",
    }
}

fn check_exports() -> Task {
    Task::CheckExports {
        list: None,
        module: None,
    }
}

/// The tasks a task runs before its own work, in order.
pub fn before(task: &Task) -> Vec<Task> {
    let lint = || Task::Fmt { check: true };
    match task {
        // The cheapest check goes first, so a mistake in the formatting is
        // reported before anything slow runs.
        Task::Ci(CiPart::Lint | CiPart::All) => vec![lint()],
        _ => vec![],
    }
}

/// The tasks a task runs after its own work, in order.
pub fn after(task: &Task) -> Vec<Task> {
    let package = || Task::Package { output: None };
    match task {
        // The module must export every function the list names, so a bad
        // link fails here and not in a program that loads it.
        Task::Wasm => vec![check_exports()],
        Task::Ci(CiPart::Build | CiPart::All) => vec![Task::Wasm, package()],
        _ => vec![],
    }
}

/// All the tasks a task runs, in order.
pub fn steps(task: &Task) -> Vec<Task> {
    let mut all = before(task);
    all.extend(after(task));
    all
}

/// Every task, in every form that runs different steps.
fn variants() -> Vec<Task> {
    vec![
        Task::Ci(CiPart::All),
        Task::Ci(CiPart::Lint),
        Task::Ci(CiPart::Build),
        Task::Wasm,
        Task::Fmt { check: false },
        Task::Clean { all: false },
        check_exports(),
        Task::CheckTag(String::new()),
        Task::Release { sha: String::new() },
        Task::DownloadCiBuild(String::new()),
        Task::PublishRelease(String::new()),
        Task::Package { output: None },
    ]
}

/// The names of the tasks the named task runs, each once, in order.
pub fn runs_of(task_name: &str) -> Vec<&'static str> {
    let mut names: Vec<&'static str> = Vec::new();
    for task in variants().iter().filter(|t| name(t) == task_name) {
        for step in steps(task) {
            if !names.contains(&name(&step)) {
                names.push(name(&step));
            }
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ci_runs_the_lint_the_module_and_the_zip() {
        assert_eq!(runs_of("ci"), ["fmt", "wasm", "package"]);
        assert_eq!(runs_of("wasm"), ["check-exports"]);
        assert!(runs_of("fmt").is_empty());
    }

    #[test]
    fn the_lint_part_only_checks_and_the_build_part_builds() {
        let names =
            |part| steps(&Task::Ci(part)).iter().map(name).collect::<Vec<_>>();
        assert_eq!(names(CiPart::Lint), ["fmt"]);
        assert_eq!(names(CiPart::Build), ["wasm", "package"]);
    }

    #[test]
    fn every_variant_is_a_listed_task() {
        for task in variants() {
            let listed = crate::help::usage_of(name(&task));
            assert!(listed.is_some(), "{} is not in the help", name(&task));
        }
    }

    #[test]
    fn ci_checks_the_formatting_without_changing_files() {
        assert_eq!(steps(&Task::Ci(CiPart::Lint)), [Task::Fmt { check: true }]);
    }
}

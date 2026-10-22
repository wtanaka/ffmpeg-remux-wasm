//! Repository tasks, run as `cargo xtask <task>`. `cargo xtask` lists them and
//! what each does (see help.rs).
//!
//! The code is kept as pure functions (decisions, parsing, building command
//! lines and file contents) with thin I/O around them (reading files, running
//! programs), so the decisions can be unit-tested.

mod check_exports;
mod check_module;
mod checksums;
mod ci;
mod clean;
mod commits;
mod dist;
mod fmt;
mod gh;
mod github;
mod graph;
mod help;
mod io;
mod notes;
mod package;
mod paths;
mod process;
mod release;
mod tag;
mod task;
mod tools;
mod wasm;

use std::process::ExitCode;
use task::Task;

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

/// A task's own work, without the tasks it runs (see graph.rs).
fn own(task: &Task) -> Result {
    match task {
        Task::Help => {
            print!("{}", help::text());
            Ok(())
        }
        Task::Wasm => wasm::run(),
        Task::Ci(part) => ci::run(part),
        Task::Fmt { check } => fmt::run(*check),
        Task::Clean { all } => clean::run(*all),
        Task::CheckExports { list, module } => {
            check_exports::run(list.as_deref(), module.as_deref())
        }
        Task::CheckModule { dir } => check_module::run(dir.as_deref()),
        Task::Dist => dist::run(),
        Task::CheckTag(tag) => release::check_tag(tag),
        Task::Release { sha } => tag::run(sha),
        Task::DownloadCiBuild(sha) => github::download_ci_build(sha),
        Task::PublishRelease(tag) => github::publish_release(tag),
        Task::Package { output } => package::run(output.as_deref()),
    }
}

/// The tasks a task runs first, the task itself, then the tasks it runs after.
fn execute(task: &Task) -> Result {
    graph::before(task).iter().try_for_each(execute)?;
    own(task)?;
    graph::after(task).iter().try_for_each(execute)
}

fn run() -> Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    execute(&task::parse(&args)?)
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

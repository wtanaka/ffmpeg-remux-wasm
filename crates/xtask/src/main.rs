//! Repository tasks, run as `cargo xtask <task>`. `cargo xtask` lists them and
//! what each does (see help.rs).
//!
//! The code is kept as pure functions (decisions, parsing, building command
//! lines and file contents) with thin I/O around them (reading files, running
//! programs), so the decisions can be unit-tested.

mod check_exports;
mod checksums;
mod ci;
mod clean;
mod commits;
mod fmt;
mod gh;
mod github;
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

fn run() -> Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match task::parse(&args)? {
        Task::Help => {
            print!("{}", help::text());
            Ok(())
        }
        Task::Wasm => wasm::run(),
        Task::Ci(part) => ci::run(&part),
        Task::Fmt { check } => fmt::run(check),
        Task::Clean { all } => clean::run(all),
        Task::CheckExports { list, module } => {
            check_exports::run(list.as_deref(), module.as_deref())
        }
        Task::CheckTag(tag) => release::check_tag(&tag),
        Task::Release { sha } => tag::run(&sha),
        Task::DownloadCiBuild(sha) => github::download_ci_build(&sha),
        Task::PublishRelease(tag) => github::publish_release(&tag),
        Task::Package { output } => package::run(output.as_deref()),
    }
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

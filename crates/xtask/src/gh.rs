//! Running the `gh` CLI (GitHub's), which needs GH_TOKEN or a login.

use crate::{process, Result};
use std::process::Output;

/// Runs `gh` and captures its output; the caller looks at the exit status.
pub fn output(args: &[String]) -> Result<Output> {
    process::output("gh", args)
}

/// Runs `gh`, passing its output through, failing if it does.
pub fn run(args: &[String]) -> Result {
    process::run("gh", args)
}

/// What `gh` printed, without the final newline; an error carrying `gh`'s own
/// message if it failed.
pub fn text(args: &[String]) -> Result<String> {
    process::text("gh", args)
}

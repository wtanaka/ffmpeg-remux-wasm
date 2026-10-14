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
    let output = output(args)?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(format!(
            "`gh {}` failed: {}",
            args.iter().take(2).cloned().collect::<Vec<_>>().join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into())
    }
}

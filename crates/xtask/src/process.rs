//! Running other programs, with errors that say what was run and why it failed.

use crate::{paths, Result};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Environment variables to set for a program.
pub type Env = Vec<(String, String)>;

/// An argument list from string literals.
pub fn args(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| part.to_string()).collect()
}

/// The command as it is shown in a message: the program and its first few
/// arguments (the flag lists can be long).
fn describe(program: &str, args: &[impl AsRef<str>]) -> String {
    let mut parts = vec![program.to_string()];
    parts.extend(args.iter().take(3).map(|arg| arg.as_ref().to_string()));
    if args.len() > 3 {
        parts.push("...".to_string());
    }
    format!("`{}`", parts.join(" "))
}

/// The message for a program that could not be started.
fn start_error(program: &str, err: &std::io::Error) -> String {
    let hint = if err.kind() == std::io::ErrorKind::NotFound {
        format!(" (is {program} installed and on your PATH?)")
    } else {
        String::new()
    };
    format!("could not run {program}: {err}{hint}")
}

/// The first of `path`'s folders that holds `program` (so that the Emscripten
/// tools are found in the PATH their environment sets, not ours).
fn search(
    program: &str,
    path: &OsString,
    is_file: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    std::env::split_paths(path)
        .map(|dir| dir.join(program))
        .find(|candidate| is_file(candidate))
}

/// `program` as a path, looked up in the PATH that `env` sets (or else ours).
pub fn which(program: &str, env: &Env) -> PathBuf {
    let path = env
        .iter()
        .find(|(key, _)| key == "PATH")
        .map(|(_, value)| OsString::from(value))
        .or_else(|| std::env::var_os("PATH"));
    path.and_then(|path| search(program, &path, Path::is_file))
        .unwrap_or_else(|| PathBuf::from(program))
}

fn build<S: AsRef<str>>(
    dir: &Path,
    env: &Env,
    program: &str,
    args: &[S],
) -> Command {
    let mut command = Command::new(which(program, env));
    command
        .current_dir(dir)
        .envs(env.iter().cloned())
        .args(args.iter().map(AsRef::as_ref));
    command
}

/// Runs a program in `dir` with `env` applied, passing its output through,
/// failing if it does.
pub fn run_in<S: AsRef<str>>(
    dir: &Path,
    env: &Env,
    program: &str,
    args: &[S],
) -> Result {
    let status = build(dir, env, program, args)
        .status()
        .map_err(|e| start_error(program, &e))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "{} failed ({status}); its own error is printed above",
            describe(program, args)
        )
        .into())
    }
}

/// Runs a program in the repository root, passing its output through, failing
/// if it does.
pub fn run<S: AsRef<str>>(program: &str, args: &[S]) -> Result {
    run_in(&paths::root(), &Env::new(), program, args)
}

/// Runs a program in `dir` and captures its output. Fails only if the program
/// could not be started; the caller looks at the exit status.
pub fn output_in<S: AsRef<str>>(
    dir: &Path,
    env: &Env,
    program: &str,
    args: &[S],
) -> Result<Output> {
    build(dir, env, program, args)
        .output()
        .map_err(|e| start_error(program, &e).into())
}

/// Runs a program in the repository root and captures its output.
pub fn output<S: AsRef<str>>(program: &str, args: &[S]) -> Result<Output> {
    output_in(&paths::root(), &Env::new(), program, args)
}

/// What the program prints for `--version`, if it is installed and works.
pub fn version_of(program: &str, env: &Env) -> Option<String> {
    let output =
        output_in(&paths::root(), env, program, &["--version"]).ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_is_shown_with_its_first_arguments() {
        assert_eq!(describe("make", &["-j4"]), "`make -j4`");
        assert_eq!(describe("git", &[] as &[&str]), "`git`");
        assert_eq!(
            describe("emcc", &["a", "b", "c", "d", "e"]),
            "`emcc a b c ...`"
        );
    }

    #[test]
    fn a_missing_program_gets_a_hint() {
        let missing = std::io::Error::from(std::io::ErrorKind::NotFound);
        assert!(start_error("gh", &missing).contains("is gh installed"));
        let other = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        assert!(!start_error("gh", &other).contains("installed"));
    }

    #[test]
    fn programs_are_found_in_the_first_folder_that_has_them() {
        let path = std::env::join_paths(["/a", "/b", "/c"]).unwrap();
        let has =
            |p: &Path| p == Path::new("/b/tool") || p == Path::new("/c/tool");
        assert_eq!(search("tool", &path, has), Some(PathBuf::from("/b/tool")));
        assert_eq!(search("other", &path, has), None);
    }
}

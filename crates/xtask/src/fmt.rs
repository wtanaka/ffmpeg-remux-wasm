//! `cargo xtask fmt [--check]`: rustfmt for Rust, dprint for Markdown, JSON,
//! TOML and YAML (see .dprint.json), and shfmt for the shell scripts.
//!
//! dprint is a prebuilt program, not a dependency, and its version is pinned in
//! .ci/DPRINT_VERSION (which .ci/setup-ubuntu.sh reads too). A missing or
//! different dprint (or shfmt) is an error that says how to install it.

use crate::{io, paths, process, tools, Result};

/// Space after redirects, indented case arms and operators that start a
/// wrapped line: the style the shell scripts are already in.
const SHFMT_STYLE: [&str; 5] = ["-i", "2", "-ci", "-bn", "-sr"];

/// A program to run, and its arguments.
type Command = (&'static str, Vec<String>);

/// The programs that format everything, in order: rustfmt, dprint, shfmt.
fn commands(check: bool, scripts: &[String]) -> Vec<Command> {
    let mut shfmt = process::args(&SHFMT_STYLE);
    shfmt.push(if check { "-d" } else { "-w" }.to_string());
    shfmt.extend(scripts.iter().cloned());
    if check {
        vec![
            ("cargo", process::args(&["fmt", "--all", "--check"])),
            ("dprint", process::args(&["check"])),
            ("shfmt", shfmt),
        ]
    } else {
        vec![
            ("cargo", process::args(&["fmt", "--all"])),
            ("dprint", process::args(&["fmt"])),
            ("shfmt", shfmt),
        ]
    }
}

/// The `*.sh` files among the names in each folder, as paths from the root,
/// sorted. `listings` pairs each folder with the names in it.
fn scripts_in(listings: &[(&str, Vec<String>)]) -> Vec<String> {
    let mut scripts: Vec<String> = listings
        .iter()
        .flat_map(|(folder, names)| {
            names
                .iter()
                .filter(|name| name.ends_with(".sh"))
                .map(move |name| format!("{folder}/{name}"))
        })
        .collect();
    scripts.sort();
    scripts
}

/// The shell scripts to format: `*.sh` in the folders of
/// `paths::SHELL_SCRIPT_DIRS` (not recursive).
pub fn shell_scripts() -> Result<Vec<String>> {
    let mut listings = Vec::new();
    for folder in paths::SHELL_SCRIPT_DIRS {
        listings.push((folder, io::list_dir(&paths::abs(folder))?));
    }
    Ok(scripts_in(&listings))
}

pub fn run(check: bool) -> Result {
    tools::ensure(&tools::dprint())?;
    tools::ensure(&tools::shfmt())?;
    for (program, args) in commands(check, &shell_scripts()?) {
        process::run(program, &args)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_and_format_use_different_flags() {
        let scripts = vec![".ci/a.sh".to_string()];
        let check = commands(true, &scripts);
        let format = commands(false, &scripts);
        assert_eq!(check[0].1, ["fmt", "--all", "--check"]);
        assert_eq!(format[0].1, ["fmt", "--all"]);
        assert_eq!(check[1].1, ["check"]);
        assert_eq!(format[1].1, ["fmt"]);
        assert!(check[2].1.contains(&"-d".to_string()));
        assert!(format[2].1.contains(&"-w".to_string()));
        assert_eq!(check[2].1.last().unwrap(), ".ci/a.sh");
    }

    #[test]
    fn only_shell_scripts_are_picked_and_sorted() {
        let listings = [
            ("b", vec!["x.sh".to_string(), "notes.md".to_string()]),
            ("a", vec!["y.sh".to_string()]),
        ];
        assert_eq!(scripts_in(&listings), ["a/y.sh", "b/x.sh"]);
    }
}

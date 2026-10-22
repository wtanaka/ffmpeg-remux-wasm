//! `cargo xtask wasm`: builds ffmpeg-remux.wasm, FFmpeg's libavformat,
//! libavcodec and libavutil as one Emscripten side module, and assembles what a
//! release publishes: the module, the FFmpeg source it was
//! built from, FFmpeg's licence, the licences of the Emscripten runtime and musl
//! libc (copied from the SDK), the build instructions, the Rust bindings for the
//! structs consumers read (made by crates/gen-ffmpeg-bindings) and SHA256SUMS.
//! They are made in ffmpeg/build/staged/, checked there (see graph.rs), and only
//! then replace ffmpeg/dist/. The flags are in flags.rs.
//!
//! Needs an Emscripten SDK (on the PATH, or EMSDK set) at the version in
//! ffmpeg/EMSCRIPTEN_VERSION, plus curl, make, tar, xz and libclang (for the
//! bindings).
//!
//! The decisions (command lines, checks, the build instructions) are pure
//! functions with tests; `run` does the reading, writing and running.

pub(crate) mod flags;

use crate::process::{self, Env};
use crate::{checksums, io, paths, Result};
use std::path::Path;

const LICENSE_LINE: &str =
    "#define FFMPEG_LICENSE \"LGPL version 2.1 or later\"";
const LIBRARIES: [&str; 3] = [
    "libavformat/libavformat.a",
    "libavcodec/libavcodec.a",
    "libavutil/libavutil.a",
];

fn tarball_name() -> String {
    format!("ffmpeg-{}.tar.xz", flags::FFMPEG_VERSION)
}

/// The variables `emsdk_env.sh` changes: the lines of `env`'s output as
/// printed after loading it, that differ from what `current` says now.
fn emsdk_env_changes(
    printed: &str,
    current: impl Fn(&str) -> Option<String>,
) -> Env {
    printed
        .lines()
        .filter_map(|line| line.split_once('='))
        .filter(|(key, value)| {
            !key.is_empty()
                && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                && current(key).as_deref() != Some(value)
        })
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

/// What to say when `emcc` can't be found.
fn emcc_missing_message(wanted: &str) -> String {
    format!(
        "emcc (the Emscripten compiler) was not found. Either it is not \
         installed, or this shell has not loaded it. To install Emscripten \
         {wanted}:\n\n  \
         git clone https://github.com/emscripten-core/emsdk.git ~/emsdk\n  \
         ~/emsdk/emsdk install {wanted}\n  \
         ~/emsdk/emsdk activate {wanted}\n\n\
         then, in each shell that builds (or set EMSDK=~/emsdk and this task \
         loads it):\n\n  . ~/emsdk/emsdk_env.sh\n\n\
         On a bare Ubuntu machine, sh {} does all of this.",
        paths::SETUP_SCRIPT
    )
}

/// A warning if `emcc --version` doesn't say it is the tested version.
fn version_warning(found: &str, wanted: &str) -> Option<String> {
    (!found.contains(wanted)).then(|| {
        format!(
            "warning: Emscripten {wanted} ({}) is the tested version; this \
             is: {found}",
            paths::EMSCRIPTEN_VERSION_FILE
        )
    })
}

/// Checks the tarball's checksum against the one in flags.rs.
fn check_tarball(
    actual: &str,
    tarball: &str,
) -> std::result::Result<(), String> {
    if actual == flags::FFMPEG_SHA256 {
        Ok(())
    } else {
        Err(format!(
            "{tarball} has SHA-256 {actual}, not the {} in flags.rs. Removed \
             the download; run again to fetch it fresh.",
            flags::FFMPEG_SHA256
        ))
    }
}

/// The flags a build tree is configured with, as one string (which is stored in
/// the tree, to tell later whether it was configured with the current ones).
fn flags_text() -> String {
    flags::CONFIGURE_FLAGS.join(" ")
}

/// Whether a build tree configured with `stored` flags can be kept: configure
/// only runs on a fresh tree, so one configured with other flags is thrown
/// away.
fn tree_is_current(stored: Option<&str>) -> bool {
    stored == Some(flags_text().as_str())
}

/// Checks that FFmpeg is configured under the LGPL, the licence the module is
/// published under.
fn check_license(config_h: &str) -> std::result::Result<(), String> {
    if config_h.lines().any(|line| line == LICENSE_LINE) {
        return Ok(());
    }
    let found: Vec<&str> = config_h
        .lines()
        .filter(|line| line.contains("FFMPEG_LICENSE"))
        .collect();
    Err(format!(
        "FFmpeg is configured as {}, not LGPL 2.1 or later. Check the flags in \
         flags.rs.",
        found.join(" ")
    ))
}

/// `curl` arguments: download the release tarball to `part`.
fn download_args(tarball: &str, part: &str) -> Vec<String> {
    process::args(&[
        "-fsSL",
        &format!("https://ffmpeg.org/releases/{tarball}"),
        "-o",
        part,
    ])
}

/// `emconfigure` arguments.
fn configure_args() -> Vec<String> {
    let mut args = process::args(&["./configure"]);
    args.extend(flags::CONFIGURE_FLAGS.iter().map(|flag| flag.to_string()));
    args
}

/// `emmake` arguments: build the three libraries with `jobs` jobs.
fn make_args(jobs: usize) -> Vec<String> {
    let mut args = process::args(&["make", &format!("-j{jobs}")]);
    args.extend(LIBRARIES.iter().map(|lib| lib.to_string()));
    args
}

/// `cargo` arguments: run the bindings generator on the configured source tree
/// and the Emscripten sysroot, writing `output`.
fn bindings_args(source: &str, sysroot: &str, output: &str) -> Vec<String> {
    let mut args = process::args(&[
        "run",
        "--quiet",
        "--package",
        "gen-ffmpeg-bindings",
        "--",
    ]);
    args.extend(
        [source, sysroot, output, flags::FFMPEG_VERSION].map(String::from),
    );
    args
}

/// The Emscripten SDK's folder, in the SDK that `env` loads.
fn sdk_dir(env: &Env) -> Result<std::path::PathBuf> {
    let emsdk = env
        .iter()
        .find(|(key, _)| key == "EMSDK")
        .map(|(_, value)| value.clone())
        .or_else(|| std::env::var("EMSDK").ok())
        .ok_or("EMSDK is not set: the bindings need the sysroot of the Emscripten SDK, and the licences are copied from it")?;
    Ok(std::path::PathBuf::from(emsdk))
}

/// The Emscripten sysroot's headers, in the SDK that `env` loads.
fn sysroot_include(env: &Env) -> Result<std::path::PathBuf> {
    Ok(sdk_dir(env)?.join("upstream/emscripten/cache/sysroot/include"))
}

/// Where the licence texts of the Emscripten runtime and of musl libc are, in
/// the SDK's `upstream/emscripten` folder, with the name each is published
/// under.
fn licence_sources(root: &Path) -> [(std::path::PathBuf, &'static str); 2] {
    [
        (root.join("LICENSE"), paths::EMSCRIPTEN_LICENSE_NAME),
        (
            root.join("system/lib/libc/musl/COPYRIGHT"),
            paths::MUSL_COPYRIGHT_NAME,
        ),
    ]
}

/// Checks that a licence text copied from the SDK is not empty.
fn check_licence_text(
    path: &Path,
    text: &str,
) -> std::result::Result<(), String> {
    if text.trim().is_empty() {
        Err(format!(
            "{} is empty. A release holds the licence texts of the Emscripten \
             runtime and musl libc, so they must be in the SDK.",
            path.display()
        ))
    } else {
        Ok(())
    }
}

/// The licence texts of the Emscripten runtime and musl libc, read from the SDK
/// that `env` loads, each with the name it is published under. Read before the
/// build, so that a missing one fails early.
fn runtime_licences(env: &Env) -> Result<Vec<(&'static str, String)>> {
    let root = sdk_dir(env)?.join("upstream/emscripten");
    let mut licences = Vec::new();
    for (path, name) in licence_sources(&root) {
        let text = io::read_to_string(&path)?;
        check_licence_text(&path, &text)?;
        licences.push((name, text));
    }
    Ok(licences)
}

/// `emcc` arguments: link the libraries into the module.
fn link_args(exports: &str, module: &str) -> Vec<String> {
    let mut args: Vec<String> =
        LIBRARIES.iter().map(|lib| lib.to_string()).collect();
    args.push(format!("-sEXPORTED_FUNCTIONS=@{exports}"));
    args.extend(process::args(&["-o", module]));
    args.extend(flags::SIDE_LINK_FLAGS.iter().map(|flag| flag.to_string()));
    args
}

/// The build instructions: the template with the flags and versions filled in
/// (written from the flags the build used) and the list of exports inserted.
fn build_doc(template: &str, exports: &str, emscripten: &str) -> String {
    let filled = template
        .replace("@FFMPEG_VERSION@", flags::FFMPEG_VERSION)
        .replace("@FFMPEG_SHA256@", flags::FFMPEG_SHA256)
        .replace("@EMSCRIPTEN_VERSION@", emscripten)
        .replace("@FFMPEG_CONFIGURE_FLAGS@", &flags_text())
        .replace("@SIDE_LINK_FLAGS@", &flags::SIDE_LINK_FLAGS.join(" "));
    let mut doc = String::new();
    for line in filled.lines() {
        if line == "@EXPORTS@" {
            for name in exports.lines() {
                doc.push_str(name);
                doc.push('\n');
            }
        } else {
            doc.push_str(line);
            doc.push('\n');
        }
    }
    doc
}

/// `sh` arguments that load the SDK's environment and print it. They run in the
/// SDK's own folder, because the script only works in shells that can't tell
/// where a sourced file is (such as dash, which is `sh` on Debian and Ubuntu)
/// when it is sourced from there.
fn emsdk_env_args() -> Vec<String> {
    process::args(&["-c", ". ./emsdk_env.sh > /dev/null 2>&1; env"])
}

/// The environment the Emscripten tools need: nothing extra if `emcc` is on
/// the PATH, otherwise what `$EMSDK/emsdk_env.sh` sets.
/// The environment that has the pinned Emscripten SDK's tools.
pub(crate) fn sdk_env() -> Result<Env> {
    let wanted =
        io::read_to_string(&paths::abs(paths::EMSCRIPTEN_VERSION_FILE))?;
    emscripten_env(wanted.trim())
}

/// The configured FFmpeg source tree under `build`.
pub(crate) fn tree_dir(build: &Path) -> std::path::PathBuf {
    build.join(format!("ffmpeg-{}", flags::FFMPEG_VERSION))
}

fn emscripten_env(wanted: &str) -> Result<Env> {
    let works = |env: &Env| process::version_of("emcc", env).is_some();
    if works(&Env::new()) {
        return Ok(Env::new());
    }
    if let Ok(emsdk) = std::env::var("EMSDK") {
        let dir = Path::new(&emsdk);
        if dir.join("emsdk_env.sh").is_file() {
            let printed =
                process::output_in(dir, &Env::new(), "sh", &emsdk_env_args())?;
            let env = emsdk_env_changes(
                &String::from_utf8_lossy(&printed.stdout),
                |key| std::env::var(key).ok(),
            );
            if works(&env) {
                return Ok(env);
            }
        }
    }
    Err(emcc_missing_message(wanted).into())
}

pub fn run() -> Result {
    let root = paths::root();
    let staged = paths::abs(paths::STAGED_DIR);
    let build = paths::abs(paths::BUILD_DIR);
    let exports = paths::abs(paths::EXPORTS_LIST);
    let tarball = tarball_name();

    let emscripten =
        io::read_to_string(&paths::abs(paths::EMSCRIPTEN_VERSION_FILE))?
            .trim()
            .to_string();
    let env = emscripten_env(&emscripten)?;
    let found = process::version_of("emcc", &env).unwrap_or_default();
    if let Some(warning) =
        version_warning(found.lines().next().unwrap_or(""), &emscripten)
    {
        eprintln!("{warning}");
    }
    let licences = runtime_licences(&env)?;
    io::create_dir_all(&build)?;

    // The release tarball, downloaded once and refused unless its checksum is
    // the one in flags.rs.
    let tarball_path = build.join(&tarball);
    if !tarball_path.exists() {
        let part = build.join(format!("{tarball}.part"));
        let args = download_args(&tarball, &part.to_string_lossy());
        process::run_in(&build, &env, "curl", &args)?;
        io::rename(&part, &tarball_path)?;
    }
    let actual = checksums::sha256_hex(&io::read(&tarball_path)?);
    if let Err(message) = check_tarball(&actual, &tarball) {
        io::remove_if_exists(&tarball_path)?;
        return Err(message.into());
    }

    let tree = tree_dir(&build);
    let stored = std::fs::read_to_string(tree.join(".flags")).ok();
    if tree.is_dir() && !tree_is_current(stored.as_deref()) {
        io::remove_if_exists(&tree)?;
    }
    if !tree.is_dir() {
        let args = process::args(&["-xJf", &tarball, "-C", "."]);
        process::run_in(&build, &env, "tar", &args)?;
    }
    if !tree.join("ffbuild/config.mak").exists() {
        process::run_in(&tree, &env, "emconfigure", &configure_args())?;
        io::write(&tree.join(".flags"), flags_text())?;
    }
    // Checked on every build, cached ones included.
    check_license(&io::read_to_string(&tree.join("config.h"))?)?;

    let jobs = std::thread::available_parallelism().map_or(1, |n| n.get());
    process::run_in(&tree, &env, "emmake", &make_args(jobs))?;
    let module = build.join(paths::MODULE_NAME);
    let link = link_args(&exports.to_string_lossy(), &module.to_string_lossy());
    process::run_in(&tree, &env, "emcc", &link)?;

    io::remove_if_exists(&staged)?;
    io::create_dir_all(&staged)?;
    io::copy(&module, &staged.join(paths::MODULE_NAME))?;
    io::copy(&tarball_path, &staged.join(&tarball))?;
    io::copy(
        &tree.join("COPYING.LGPLv2.1"),
        &staged.join(paths::LICENSE_NAME),
    )?;
    for (name, text) in &licences {
        io::write(&staged.join(name), text)?;
    }
    let sysroot = sysroot_include(&env)?;
    process::run_in(
        &root,
        &env,
        "cargo",
        &bindings_args(
            &tree.to_string_lossy(),
            &sysroot.to_string_lossy(),
            &staged.join(paths::BINDINGS_NAME).to_string_lossy(),
        ),
    )?;
    let doc = build_doc(
        &io::read_to_string(&root.join(paths::BUILD_DOC_TEMPLATE))?,
        &io::read_to_string(&exports)?,
        &emscripten,
    );
    io::write(&staged.join(paths::DOC_NAME), &doc)?;
    let sums = checksums::write(
        &staged,
        &[
            paths::MODULE_NAME,
            tarball.as_str(),
            paths::LICENSE_NAME,
            paths::EMSCRIPTEN_LICENSE_NAME,
            paths::MUSL_COPYRIGHT_NAME,
            paths::DOC_NAME,
            paths::BINDINGS_NAME,
        ],
    )?;
    println!("built {}/:\n{sums}", paths::STAGED_DIR);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_build_doc_fills_in_flags_and_exports() {
        let doc = build_doc(
            "v @EMSCRIPTEN_VERSION@\n```\n@EXPORTS@\n```\n",
            "_a\n_b\n",
            "9.9.9",
        );
        assert_eq!(doc, "v 9.9.9\n```\n_a\n_b\n```\n");
        let doc =
            build_doc("@FFMPEG_CONFIGURE_FLAGS@ | @SIDE_LINK_FLAGS@", "", "1");
        assert!(doc.contains("--disable-everything"), "{doc}");
        assert!(doc.contains("-sSIDE_MODULE=2"), "{doc}");
    }

    #[test]
    fn the_variables_emsdk_changes_are_picked_out() {
        let printed = "PATH=/emsdk/bin:/usr/bin\nHOME=/home/me\nEMSDK=/emsdk\n\
                       BAD-KEY=x\nnot a variable\n=nothing\n";
        let current = |key: &str| match key {
            "HOME" => Some("/home/me".to_string()),
            "PATH" => Some("/usr/bin".to_string()),
            _ => None,
        };
        assert_eq!(
            emsdk_env_changes(printed, current),
            [
                ("PATH".to_string(), "/emsdk/bin:/usr/bin".to_string()),
                ("EMSDK".to_string(), "/emsdk".to_string()),
            ]
        );
    }

    #[test]
    fn the_sdk_script_is_sourced_by_a_path_that_works_in_dash() {
        let args = emsdk_env_args();
        // Relative to the folder it runs in, not an absolute path.
        assert!(args[1].starts_with(". ./emsdk_env.sh"), "{args:?}");
    }

    #[test]
    fn a_missing_emscripten_says_how_to_install_the_wanted_version() {
        let message = emcc_missing_message("4.0.10");
        assert!(message.contains("emsdk install 4.0.10"), "{message}");
        assert!(message.contains(paths::SETUP_SCRIPT), "{message}");
    }

    #[test]
    fn only_a_different_emscripten_version_is_warned_about() {
        assert_eq!(version_warning("emcc 4.0.10 (x)", "4.0.10"), None);
        let warning = version_warning("emcc 3.1.0 (x)", "4.0.10").unwrap();
        assert!(warning.contains("4.0.10") && warning.contains("3.1.0"));
    }

    #[test]
    fn only_the_expected_tarball_is_accepted() {
        assert!(check_tarball(flags::FFMPEG_SHA256, "t").is_ok());
        let error = check_tarball("0000", "ffmpeg-1.tar.xz").unwrap_err();
        assert!(error.contains("ffmpeg-1.tar.xz") && error.contains("0000"));
        assert!(error.contains("run again"), "{error}");
    }

    #[test]
    fn a_tree_is_kept_only_if_configured_with_the_current_flags() {
        assert!(tree_is_current(Some(&flags_text())));
        assert!(!tree_is_current(Some("--other")));
        assert!(!tree_is_current(None));
    }

    #[test]
    fn the_runtime_licences_are_in_the_sdk_root() {
        let root = Path::new("/sdk/upstream/emscripten");
        let sources = licence_sources(root);
        assert_eq!(sources[0].0, root.join("LICENSE"));
        assert_eq!(sources[0].1, "Emscripten-LICENSE.txt");
        assert_eq!(sources[1].0, root.join("system/lib/libc/musl/COPYRIGHT"));
        assert_eq!(sources[1].1, "musl-COPYRIGHT.txt");
    }

    #[test]
    fn an_empty_licence_text_is_refused() {
        let path = Path::new("LICENSE");
        assert!(check_licence_text(path, "MIT License\n").is_ok());
        let error = check_licence_text(path, " \n").unwrap_err();
        assert!(error.contains("LICENSE is empty"), "{error}");
    }

    #[test]
    fn only_an_lgpl_configuration_passes() {
        let ok = "#define A 1\n#define FFMPEG_LICENSE \"LGPL version 2.1 or later\"\n";
        assert!(check_license(ok).is_ok());
        let gpl = "#define FFMPEG_LICENSE \"GPL version 2 or later\"\n";
        let error = check_license(gpl).unwrap_err();
        assert!(error.contains("GPL version 2 or later"), "{error}");
        assert!(error.contains("Check the flags"), "{error}");
        assert!(check_license("").is_err());
    }

    #[test]
    fn the_command_lines_hold_the_flags_and_paths() {
        let configure = configure_args();
        assert_eq!(configure[0], "./configure");
        assert_eq!(configure.len(), flags::CONFIGURE_FLAGS.len() + 1);

        let make = make_args(8);
        assert_eq!(make[..2], ["make", "-j8"]);
        assert!(make.contains(&"libavutil/libavutil.a".to_string()));

        let link = link_args("/x/exports.txt", "/x/out.wasm");
        assert!(
            link.contains(&"-sEXPORTED_FUNCTIONS=@/x/exports.txt".to_string())
        );
        let out = link.iter().position(|arg| arg == "-o").unwrap();
        assert_eq!(link[out + 1], "/x/out.wasm");
        assert!(link.contains(&"-Oz".to_string()));

        let download = download_args("t.tar.xz", "t.part");
        assert_eq!(download[1], "https://ffmpeg.org/releases/t.tar.xz");
        assert_eq!(download[3], "t.part");
    }

    #[test]
    fn the_flags_are_sorted_without_repeats() {
        let flags = flags::CONFIGURE_FLAGS;
        assert!(flags.windows(2).all(|pair| pair[0] < pair[1]), "{flags:?}");
        let everything =
            flags.iter().position(|f| *f == "--disable-everything");
        let first_enable =
            flags.iter().position(|f| f.starts_with("--enable-"));
        assert!(everything < first_enable);
    }
}

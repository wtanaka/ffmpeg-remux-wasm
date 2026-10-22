//! Where things are in the repository, relative to its root (see `root()`),
//! and what they are called. Every path and file name the tasks use is defined
//! here once, built from the two folder names below, so a rename is one edit.
//! (The macros exist because `concat!` can join literals but not constants.)

macro_rules! ffmpeg_dir {
    () => {
        "ffmpeg"
    };
}
macro_rules! ci_dir {
    () => {
        ".ci"
    };
}
/// A file under the CI folder, as a path from the repository root.
macro_rules! ci_file {
    ($name:literal) => {
        concat!(ci_dir!(), "/", $name)
    };
}
/// The contents of a file under the CI folder, read at compile time.
macro_rules! pinned {
    ($name:literal) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../",
            ci_file!($name)
        ))
    };
}
macro_rules! module_name {
    () => {
        "ffmpeg-remux.wasm"
    };
}

/// The Emscripten version the module is built with.
pub const EMSCRIPTEN_VERSION_FILE: &str =
    concat!(ffmpeg_dir!(), "/EMSCRIPTEN_VERSION");
/// The template of the published build instructions.
pub const BUILD_DOC_TEMPLATE: &str =
    concat!(ffmpeg_dir!(), "/FFMPEG-BUILD.md.in");
/// FFmpeg's download and objects (generated).
pub const BUILD_DIR: &str = concat!(ffmpeg_dir!(), "/build");
/// The files a release holds (generated).
pub const DIST_DIR: &str = concat!(ffmpeg_dir!(), "/dist");
/// The program `check-module` loads the module in.
pub const PROBE_SOURCE: &str = concat!(ffmpeg_dir!(), "/tests/probe.rs");
pub const PROBE_SOURCE_NAME: &str = "probe.rs";
/// The functions the module must export.
pub const EXPORTS_LIST: &str = concat!(ffmpeg_dir!(), "/exports.txt");
/// The built module's file name.
pub const MODULE_NAME: &str = module_name!();
/// What a release holds, besides the module and FFmpeg's source.
pub const LICENSE_NAME: &str = "FFmpeg-LGPL-2.1.txt";
/// The licences of the Emscripten runtime and of musl libc, which the module
/// and any program linked with it contain. Copied from the Emscripten SDK.
pub const EMSCRIPTEN_LICENSE_NAME: &str = "Emscripten-LICENSE.txt";
pub const MUSL_COPYRIGHT_NAME: &str = "musl-COPYRIGHT.txt";
pub const DOC_NAME: &str = "FFMPEG-BUILD.md";
/// Rust declarations for the structs consumers read (generated).
pub const BINDINGS_NAME: &str = "ffmpeg_remux_bindings.rs";
pub const SUMS_NAME: &str = "SHA256SUMS";
/// The built module.
pub const MODULE: &str = concat!(ffmpeg_dir!(), "/dist/", module_name!());

/// The manifest whose `version` is the release version.
pub const ROOT_MANIFEST: &str = "Cargo.toml";
/// Sets up a bare Ubuntu machine.
pub const SETUP_SCRIPT: &str = ci_file!("setup-ubuntu.sh");
/// The pinned dprint version.
pub const DPRINT_VERSION_FILE: &str = ci_file!("DPRINT_VERSION");
/// The pinned shfmt version.
pub const SHFMT_VERSION_FILE: &str = ci_file!("SHFMT_VERSION");
/// The pinned git-cliff version.
pub const GIT_CLIFF_VERSION_FILE: &str = ci_file!("GIT_CLIFF_VERSION");
/// The pinned committed version.
pub const COMMITTED_VERSION_FILE: &str = ci_file!("COMMITTED_VERSION");
/// committed's configuration: what a commit message must look like.
pub const COMMITTED_CONFIG: &str = ".github/committed.toml";
/// git-cliff's configuration: how commits become release notes.
pub const CLIFF_CONFIG: &str = ".github/cliff.toml";
/// The pinned SHA-256 of each shfmt release binary.
pub const SHFMT_SHA256_FILE: &str = ci_file!("shfmt.sha256");

/// The folders whose `*.sh` files are formatted and checked.
pub const SHELL_SCRIPT_DIRS: [&str; 1] = [ci_dir!()];

/// Where `package` writes by default: in cargo's target directory (not
/// version-controlled, and removed by `cargo clean`), in its own subfolder
/// because that directory may be shared with other projects.
pub const PACKAGE_ZIP: &str = "xtask/ffmpeg-remux-wasm.zip";

// The pinned versions, read at compile time.
pub const DPRINT_VERSION: &str = pinned!("DPRINT_VERSION");
pub const SHFMT_VERSION: &str = pinned!("SHFMT_VERSION");
pub const GIT_CLIFF_VERSION: &str = pinned!("GIT_CLIFF_VERSION");
pub const COMMITTED_VERSION: &str = pinned!("COMMITTED_VERSION");

/// The repository root.
pub fn root() -> std::path::PathBuf {
    let root =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    // Without the `..` parts, so paths printed in messages read naturally.
    root.canonicalize().unwrap_or(root)
}

/// A path relative to the repository root, as an absolute path.
pub fn abs(relative: &str) -> std::path::PathBuf {
    root().join(relative)
}

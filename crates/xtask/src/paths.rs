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
/// The functions the module must export.
pub const EXPORTS_LIST: &str = concat!(ffmpeg_dir!(), "/exports.txt");
/// The built module's file name.
pub const MODULE_NAME: &str = module_name!();
/// What a release holds, besides the module and FFmpeg's source.
pub const LICENSE_NAME: &str = "FFmpeg-LGPL-2.1.txt";
pub const DOC_NAME: &str = "FFMPEG-BUILD.md";
pub const SUMS_NAME: &str = "SHA256SUMS";
/// The built module.
pub const MODULE: &str = concat!(ffmpeg_dir!(), "/dist/", module_name!());

/// The manifest whose `version` is the release version.
pub const ROOT_MANIFEST: &str = "Cargo.toml";
/// Sets up a bare Ubuntu machine.
pub const SETUP_SCRIPT: &str = concat!(ci_dir!(), "/setup-ubuntu.sh");
/// The pinned dprint version.
pub const DPRINT_VERSION_FILE: &str = concat!(ci_dir!(), "/DPRINT_VERSION");
/// The pinned shfmt version.
pub const SHFMT_VERSION_FILE: &str = concat!(ci_dir!(), "/SHFMT_VERSION");
/// The pinned SHA-256 of each shfmt release binary.
pub const SHFMT_SHA256_FILE: &str = concat!(ci_dir!(), "/shfmt.sha256");

/// The folders whose `*.sh` files are formatted and checked.
pub const SHELL_SCRIPT_DIRS: [&str; 1] = [ci_dir!()];

/// Where `package` writes by default: in cargo's target directory (not
/// version-controlled, and removed by `cargo clean`), in its own subfolder
/// because that directory may be shared with other projects.
pub const PACKAGE_ZIP: &str = "xtask/ffmpeg-remux-wasm.zip";

// The contents of the pinned files, read at compile time.
pub const DPRINT_VERSION: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../",
    ci_dir!(),
    "/DPRINT_VERSION"
));
pub const SHFMT_VERSION: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../",
    ci_dir!(),
    "/SHFMT_VERSION"
));

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

//! `cargo xtask check-module [folder]`: loads the built module (the one in the
//! folder, by default ffmpeg/dist/) in a small program and reads test videos with it, to
//! check that libavformat finds in each what it should. The module's flags
//! decide what libavformat and libavcodec contain, so this finds a missing
//! decoder, parser or demuxer before a program loading the module does.
//!
//! The program (ffmpeg/tests/probe.rs) is Rust, built for
//! wasm32-unknown-emscripten with the bindings that `wasm` generated, so it
//! also checks that their layout matches the module. `rustc` links it against
//! the module, and `node` (from the Emscripten SDK) runs it. The videos are made
//! by synthvid (a library, pinned in Cargo.toml). Needs the module and its
//! bindings in the folder, so run `wasm` first, and the Rust target
//! (`rustup target add`).

use crate::process;
use crate::{io, paths, wasm, Result};
use synthvid_catalog::cover::generate_catalogue;
use synthvid_catalog::generate_entry;

/// Each video (a synthvid catalogue entry) and what the program must print for
/// it, one "<codec>,<width>,<height>" line per stream (see probe.c).
///
/// A motion-JPEG track whose MP4 boxes hold no width or height: only a decoder
/// finds the size, so without the mjpeg decoder libavformat says 0,0.
const VIDEOS: [(&str, &str); 1] = [(
    "0016x0016-10-30fps-solid-disc-fixed-mjpeg-iso-identity-unscaled-no-dim",
    "mjpeg,16,16",
)];

/// The Rust target the program is built for, the one the bindings describe.
const TARGET: &str = "wasm32-unknown-emscripten";

/// `rustc` arguments: build `source` and link it with `module`, so that the
/// module is loaded when `program` (a .js file, run by node) starts. The
/// program reads the videos from the host's files.
fn rustc_args(source: &str, module: &str, program: &str) -> Vec<String> {
    let link = |arg: &str| format!("link-arg={arg}");
    let mut args = process::args(&["--edition", "2021", "--target", TARGET]);
    args.push(source.to_string());
    for arg in [module, "-sMAIN_MODULE=2", "-sNODERAWFS=1"] {
        args.extend(["-C".to_string(), link(arg)]);
    }
    args.extend(["-C".to_string(), link("-sALLOW_MEMORY_GROWTH")]);
    args.extend(process::args(&["-o", program]));
    args
}

/// What to add to an error from building the program.
fn target_hint() -> String {
    format!(
        "If it says the standard library for {TARGET} cannot be found, run \
         `rustup target add {TARGET}`."
    )
}

/// Whether the program found `expected` in `file`, or what to do about it.
fn check_streams(
    file: &str,
    found: &str,
    expected: &str,
) -> std::result::Result<(), String> {
    if found.trim() == expected {
        return Ok(());
    }
    Err(format!(
        "reading {file}, the module found `{}` and not `{expected}`. A stream \
         with a size of 0,0 means the module has no decoder for it: FFmpeg \
         decodes a frame to learn a size the file does not hold, so add \
         --enable-decoder=<codec> to the flags in flags.rs. A missing stream \
         or a failed open means a missing demuxer or parser.",
        found.trim().replace('\n', "; ")
    ))
}

/// The media file of the synthvid catalogue entry named `name`.
fn media_of(name: &str) -> Result<(String, Vec<u8>)> {
    let catalogue = generate_catalogue()
        .map_err(|e| format!("the synthvid catalogue failed: {e}"))?;
    let entry = catalogue
        .iter()
        .find(|entry| entry.manifest_name().as_str() == name)
        .ok_or_else(|| {
            format!(
                "synthvid has no entry named {name}; was it renamed in the \
                 commit that Cargo.toml pins?"
            )
        })?;
    let generated = generate_entry(entry)
        .map_err(|e| format!("synthvid could not make {name}: {e}"))?;
    Ok((
        entry.media_name().to_string(),
        generated.media_bytes().as_bytes().to_vec(),
    ))
}

pub fn run(dir: Option<&str>) -> Result {
    let dir = dir
        .map_or_else(|| paths::abs(paths::DIST_DIR), std::path::PathBuf::from);
    let module = dir.join(paths::MODULE_NAME);
    if !module.is_file() {
        return Err(format!(
            "{} is not built; run `cargo xtask wasm` first",
            module.display()
        )
        .into());
    }
    let env = wasm::sdk_env()?;
    let work = paths::abs(paths::BUILD_DIR).join("module-check");
    io::create_dir_all(&work)?;

    // The module, the bindings and the program go in one folder, where the
    // program finds them by name.
    io::copy(&module, &work.join(paths::MODULE_NAME))?;
    io::copy(
        &dir.join(paths::BINDINGS_NAME),
        &work.join(paths::BINDINGS_NAME),
    )?;
    io::copy(
        &paths::abs(paths::PROBE_SOURCE),
        &work.join(paths::PROBE_SOURCE_NAME),
    )?;
    let args =
        rustc_args(paths::PROBE_SOURCE_NAME, paths::MODULE_NAME, "probe.js");
    process::run_in(&work, &env, "rustc", &args)
        .map_err(|e| format!("{e}. {}", target_hint()))?;

    for (entry, expected) in VIDEOS {
        let (file, bytes) = media_of(entry)?;
        io::write(&work.join(&file), bytes)?;
        let found =
            process::text_in(&work, &env, "node", &["probe.js", &file])?;
        check_streams(&file, &found, expected)?;
        println!("{entry}: {expected}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_program_is_linked_with_the_module_as_a_main_module() {
        let args = rustc_args("p.rs", "m.wasm", "p.js");
        assert!(args.contains(&"wasm32-unknown-emscripten".to_string()));
        assert!(args.contains(&"link-arg=m.wasm".to_string()));
        assert!(args.contains(&"link-arg=-sMAIN_MODULE=2".to_string()));
        assert_eq!(&args[args.len() - 2..], ["-o", "p.js"]);
    }

    #[test]
    fn a_missing_target_says_how_to_add_it() {
        assert!(target_hint().contains("rustup target add"));
    }

    #[test]
    fn matching_streams_pass() {
        assert!(check_streams("a.mp4", "mjpeg,16,16\n", "mjpeg,16,16").is_ok());
    }

    #[test]
    fn a_stream_without_a_size_says_to_enable_a_decoder() {
        let error =
            check_streams("a.mp4", "mjpeg,0,0\n", "mjpeg,16,16").unwrap_err();
        assert!(error.contains("a.mp4"), "{error}");
        assert!(error.contains("`mjpeg,0,0`"), "{error}");
        assert!(error.contains("--enable-decoder"), "{error}");
    }

    #[test]
    fn the_video_is_a_synthvid_entry_with_a_media_file() {
        for (entry, _) in VIDEOS {
            let (file, bytes) = media_of(entry).unwrap();
            assert!(file.starts_with(entry), "{file}");
            assert!(!bytes.is_empty());
        }
        assert!(media_of("no-such-entry")
            .unwrap_err()
            .to_string()
            .contains("no-such-entry"));
    }

    #[test]
    fn the_probe_program_exists() {
        assert!(paths::abs(paths::PROBE_SOURCE).is_file());
    }
}

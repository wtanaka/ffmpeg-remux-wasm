//! `cargo xtask dist`: replaces ffmpeg/dist/ with the build that `wasm` made in
//! ffmpeg/build/staged/. `wasm` runs it last, after the checks of the staged
//! module, so dist/ never holds a module that failed one.

use crate::{io, paths, Result};

pub fn run() -> Result {
    let staged = paths::abs(paths::STAGED_DIR);
    let files = if staged.is_dir() {
        io::list_dir(&staged)?
    } else {
        Vec::new()
    };
    if !files.iter().any(|name| name == paths::MODULE_NAME) {
        return Err(format!(
            "{} has no {}; run `cargo xtask wasm`, which makes it",
            paths::STAGED_DIR,
            paths::MODULE_NAME
        )
        .into());
    }
    let dist = paths::abs(paths::DIST_DIR);
    io::remove_if_exists(&dist)?;
    io::create_dir_all(&dist)?;
    for name in &files {
        io::copy(&staged.join(name), &dist.join(name))?;
    }
    println!("wrote {}/:\n{}", paths::DIST_DIR, files.join("\n"));
    Ok(())
}

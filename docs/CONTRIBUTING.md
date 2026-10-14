# Developing ffmpeg-remux-wasm

## Building

- Needs: the Emscripten SDK at the version in `ffmpeg/EMSCRIPTEN_VERSION`, plus
  `curl`, `make`, `tar`, `xz` and Rust.
- Run `cargo xtask wasm`.
- It downloads the FFmpeg source, checking its SHA-256 against the one in
  `crates/xtask/src/wasm/flags.rs`.
- It builds the module and puts everything a release holds in `ffmpeg/dist/`.
- The first build takes a few minutes.
- The same source, flags and Emscripten version give the same bytes.

## Tasks

- Run `cargo xtask` to list the tasks and what each one does.
- To release: push the commit with the new `version` in the root `Cargo.toml`,
  let CI pass on it, then run `cargo xtask release <full commit hash>`.
- Run `cargo xtask ci` to run everything CI checks before you push.
- Run `cargo test` for the unit tests. Each task's decisions are pure functions,
  tested without running anything; only a thin layer reads files and runs
  programs.
- To add a task: write it in its own file under `crates/xtask/src/`, then add
  its usage and description to `help.rs`, a variant and a parse rule to
  `task.rs`, and a line to `main.rs`.

## Directories

- `ffmpeg/`: the build of `ffmpeg-remux.wasm`.
  - `exports.txt`: the functions the module must export, one per line.
  - `EMSCRIPTEN_VERSION`: the Emscripten version the module is built with.
  - `FFMPEG-BUILD.md.in`: the template of the published build instructions.
  - `build/`: the FFmpeg download and its objects (generated).
  - `dist/`: the files a release holds (generated).
- `crates/xtask/`: the `cargo xtask` tasks.
  - `src/task.rs`, `src/help.rs`: the command line and the task list.
  - `src/paths.rs`: every path and file name, defined once.
  - `src/wasm/`: the module build, with the FFmpeg version and flags in
    `flags.rs`.
- `.ci/`: setup for a bare Ubuntu machine, and the pinned versions of the
  formatters.
- `.github/`: the CI and release workflows, the actions they share (setup,
  upload, Rust build cache), and Dependabot.
- `.cargo/`: the `cargo xtask` alias.
- `docs/`: this file.
- `target/`: cargo's build output (generated); elsewhere if `CARGO_TARGET_DIR`
  is set.

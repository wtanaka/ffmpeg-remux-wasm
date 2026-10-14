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

- `cargo xtask`: list the tasks.
- `cargo xtask ci`: run what CI runs.
- Release: push the bump, let CI pass, `cargo xtask release <sha>`.
- `cargo test`: decisions are pure functions; a thin layer does the I/O.
- New task: its own file, then `help.rs`, `task.rs`, `main.rs`.

## Commit messages

- Conventional Commits, 80 columns: `feat(exports): expose av_dict_get`.
- Imperative, lower case, no full stop. Scopes: `.github/committed.toml`.
- Release notes: `.github/cliff.toml`. `cargo xtask ci lint` checks them.

## Directories

- `ffmpeg/`: the build of `ffmpeg-remux.wasm`.
  - `exports.txt`: the functions the module must export, one per line.
  - `EMSCRIPTEN_VERSION`: the Emscripten version the module is built with.
  - `FFMPEG-BUILD.md.in`: the template of the published build instructions.
  - `build/`: the FFmpeg download and its objects (generated).
  - `dist/`: the files a release holds (generated).
- `crates/xtask/`: the `cargo xtask` tasks.
  - `wasm/`: the module build; the FFmpeg version and flags are in `flags.rs`.
  - `paths.rs`: every path and file name.
- `.ci/`: the setup script and the pinned tool versions.
- `.github/`: workflows, shared actions, Dependabot, git-cliff and committed
  configuration.
- `.cargo/`: the `cargo xtask` alias.
- `docs/`: this file.
- `target/`: cargo's build output (generated); elsewhere if `CARGO_TARGET_DIR`
  is set.

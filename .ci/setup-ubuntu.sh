#!/bin/sh
# Sets up a bare Ubuntu or Debian machine (a CI runner, a cloud shell, a fresh
# container) to build and format this repository: system packages, Rust, the
# Emscripten SDK (ffmpeg/EMSCRIPTEN_VERSION), dprint (.ci/DPRINT_VERSION)
# shfmt (.ci/SHFMT_VERSION, checksums in .ci/shfmt.sha256), git-cliff and
# committed (.ci/GIT_CLIFF_VERSION, .ci/COMMITTED_VERSION).
#
#   sh .ci/setup-ubuntu.sh
#
#   EMSDK_DIR   where the Emscripten SDK goes (default: ~/emsdk)
#
# Afterwards, in each new shell:
#   . "$HOME/.cargo/env"; . "$EMSDK_DIR/emsdk_env.sh"
#   PATH="$HOME/.local/bin:$PATH"
#
# A run that finishes leaves a stamp of the versions it installed. The next run
# compares the stamp with the pinned versions and exits at once if they match:
# that check uses only shell built-ins (no process is started) and opens four
# small files, so a warm CI run pays almost nothing. Delete the stamp, or
# change a version or SETUP_REV, to install again.
set -eu

SETUP_REV=6 # bump when this script changes what it installs
case $0 in
  */*) HERE=${0%/*} ;;
  *) HERE=. ;;
esac
EMSDK_DIR=${EMSDK_DIR:-$HOME/emsdk}
STAMP=${XDG_CACHE_HOME:-$HOME/.cache}/ffmpeg-remux-wasm/setup-stamp

# `read` is a built-in, so unlike `tr` or `cat` it starts no process.
read -r EMSCRIPTEN_VERSION < "$HERE/../ffmpeg/EMSCRIPTEN_VERSION"
read -r DPRINT_VERSION < "$HERE/DPRINT_VERSION"
read -r SHFMT_VERSION < "$HERE/SHFMT_VERSION"
read -r GIT_CLIFF_VERSION < "$HERE/GIT_CLIFF_VERSION"
read -r COMMITTED_VERSION < "$HERE/COMMITTED_VERSION"
WANT="rev=$SETUP_REV emscripten=$EMSCRIPTEN_VERSION dprint=$DPRINT_VERSION shfmt=$SHFMT_VERSION git-cliff=$GIT_CLIFF_VERSION committed=$COMMITTED_VERSION emsdk=$EMSDK_DIR"
HAVE=
[ -f "$STAMP" ] && read -r HAVE < "$STAMP"
if [ "$HAVE" = "$WANT" ]; then
  echo "setup is up to date ($WANT)"
  exit 0
fi

command -v apt-get > /dev/null \
  || {
    echo "this script needs apt-get (Ubuntu or Debian)" >&2
    exit 1
  }

# Root installs packages directly; anyone else needs sudo.
if [ "$(id -u)" -eq 0 ]; then
  SUDO=
elif command -v sudo > /dev/null; then
  SUDO=sudo
else
  echo "run as root, or install sudo: apt-get needs root" >&2
  exit 1
fi

echo "==> system packages"
$SUDO apt-get update
# git, python3 and xz-utils: the Emscripten SDK; build-essential: FFmpeg's
# configure and make; shellcheck: `cargo xtask ci lint`; curl: downloads;
# ca-certificates: https; libclang-dev: the bindings generator (bindgen).
$SUDO env DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
  build-essential ca-certificates curl git libclang-dev make python3 shellcheck \
  xz-utils

echo "==> Rust"
if [ ! -x "$HOME/.cargo/bin/cargo" ] && ! command -v cargo > /dev/null; then
  curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs \
    | sh -s -- -y --profile minimal -c rustfmt
fi
# shellcheck disable=SC1091
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
# The target `cargo xtask check-module` builds its program for.
rustup target add wasm32-unknown-emscripten

echo "==> Emscripten $EMSCRIPTEN_VERSION"
if [ ! -d "$EMSDK_DIR" ]; then
  git clone --depth 1 https://github.com/emscripten-core/emsdk.git "$EMSDK_DIR"
fi
"$EMSDK_DIR/emsdk" install "$EMSCRIPTEN_VERSION"
"$EMSDK_DIR/emsdk" activate "$EMSCRIPTEN_VERSION"

# The tools are downloaded as prebuilt binaries, not compiled.
echo "==> dprint $DPRINT_VERSION"
if ! command -v cargo-binstall > /dev/null; then
  curl -L --proto '=https' --tlsv1.2 -sSf \
    https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh \
    | bash
fi
cargo binstall --no-confirm "dprint@$DPRINT_VERSION"

echo "==> git-cliff $GIT_CLIFF_VERSION and committed $COMMITTED_VERSION"
cargo binstall --no-confirm "git-cliff@$GIT_CLIFF_VERSION" \
  "committed@$COMMITTED_VERSION"

echo "==> shfmt $SHFMT_VERSION"
case $(uname -m) in
  x86_64) ARCH=amd64 ;;
  aarch64 | arm64) ARCH=arm64 ;;
  *)
    echo "no shfmt build for $(uname -m)" >&2
    exit 1
    ;;
esac
SHFMT_FILE=shfmt_v${SHFMT_VERSION}_linux_$ARCH
mkdir -p "$HOME/.local/bin"
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
curl -fsSL -o "$TMP/$SHFMT_FILE" \
  "https://github.com/mvdan/sh/releases/download/v$SHFMT_VERSION/$SHFMT_FILE"
# The release has no checksum file, so the expected checksums are in this
# repository: the download must match before it is installed.
(cd "$TMP" && grep " $SHFMT_FILE\$" "$OLDPWD/$HERE/shfmt.sha256" | sha256sum -c -)
install -m 755 "$TMP/$SHFMT_FILE" "$HOME/.local/bin/shfmt"

mkdir -p "${STAMP%/*}"
printf '%s\n' "$WANT" > "$STAMP"

cat << DONE

Done. In each new shell, run:
  . "\$HOME/.cargo/env"; . "$EMSDK_DIR/emsdk_env.sh"; PATH="\$HOME/.local/bin:\$PATH"
then: cargo xtask wasm
DONE

# ffmpeg-remux-wasm

A copy-only build of [FFmpeg](https://ffmpeg.org)'s libavformat, libavcodec and
libavutil as one Emscripten side module, `ffmpeg-remux.wasm`. It copies streams
without decoding or encoding them.

## Download

- [Latest release](https://github.com/wtanaka/ffmpeg-remux-wasm/releases/latest):
  `ffmpeg-remux.wasm`, the FFmpeg source, the licence, `FFMPEG-BUILD.md` and
  `SHA256SUMS`.
- Built by GitHub Actions. The module and the FFmpeg source are attested: check
  one with `gh attestation verify <file> --repo wtanaka/ffmpeg-remux-wasm`.
- To build it yourself or contribute, see
  [docs/CONTRIBUTING.md](https://github.com/wtanaka/ffmpeg-remux-wasm/blob/main/docs/CONTRIBUTING.md).

## Rust bindings

`ffmpeg_remux_bindings.rs` declares the FFmpeg structs and version constants
that consumers read: `AVFormatContext`, `AVStream`, `AVCodecParameters`,
`AVPacket`, `AVDictionary`, `AVMD5`. The layout is wasm32 and matches only the
`ffmpeg-remux.wasm` of the same release.

## Licence

- Scripts and Rust code: MIT (`LICENSE-MIT`) or Apache-2.0 (`LICENSE-APACHE`),
  at your option.
- `ffmpeg-remux.wasm` and `ffmpeg_remux_bindings.rs`: FFmpeg's own code, LGPL
  2.1 or later (`FFmpeg-LGPL-2.1.txt`). `ffmpeg_remux_bindings.rs` is derived
  from FFmpeg's headers.

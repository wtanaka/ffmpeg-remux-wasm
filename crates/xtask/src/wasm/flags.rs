//! How the module is built: the FFmpeg release and the flags it is configured
//! and linked with. The published build instructions list the same flags.

/// The FFmpeg release, and the SHA-256 of its tarball. Change both together.
pub const FFMPEG_VERSION: &str = "7.1.1";
pub const FFMPEG_SHA256: &str =
    "733984395e0dbbe5c046abda2dc49a5544e7e0e1e2366bba849222ae9e3a03b1";

/// Keep to LGPL code (no --enable-gpl, --enable-nonfree or --enable-version3,
/// and no external libraries), so that a closed-source program can link the
/// module. The build checks the licence that FFmpeg is configured with.
///
/// One flag per line, sorted, so that a duplicate or a missing flag is easy to
/// see. Keep --disable-everything before the --enable-* flags (it sorts first).
pub const CONFIGURE_FLAGS: [&str; 40] = [
    "--ar=emar",
    "--arch=x86_32",
    "--cc=emcc",
    "--cxx=em++",
    "--dep-cc=emcc",
    "--disable-asm",
    "--disable-autodetect",
    "--disable-avdevice",
    "--disable-avfilter",
    "--disable-debug",
    "--disable-doc",
    "--disable-everything",
    "--disable-inline-asm",
    "--disable-network",
    "--disable-programs",
    "--disable-pthreads",
    "--disable-runtime-cpudetect",
    "--disable-stripping",
    "--disable-swresample",
    "--disable-swscale",
    "--disable-x86asm",
    "--enable-bsf=aac_adtstoasc",
    "--enable-bsf=h264_mp4toannexb",
    "--enable-bsf=hevc_mp4toannexb",
    "--enable-cross-compile",
    "--enable-demuxer=concat",
    "--enable-demuxer=mov",
    "--enable-muxer=mov",
    "--enable-muxer=mp4",
    "--enable-parser=aac",
    "--enable-parser=h264",
    "--enable-parser=hevc",
    "--enable-parser=mjpeg",
    "--enable-pic",
    "--enable-protocol=file",
    "--enable-small",
    "--nm=emnm",
    "--objcc=emcc",
    "--ranlib=emranlib",
    "--target-os=none",
];

/// The three libraries as one Emscripten side module. It exports
/// FFmpeg's default-visibility symbols (the module's own data and function
/// pointers resolve through them); exports.txt is the list of functions a
/// program loading it is expected to call, and the build checks that the module
/// exports every one.
pub const SIDE_LINK_FLAGS: [&str; 3] =
    ["-Oz", "-sSIDE_MODULE=2", "-sWASM_BIGINT=1"];

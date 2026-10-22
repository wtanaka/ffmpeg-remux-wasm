//! Prints "<codec>,<width>,<height>" for each stream of the file named on the
//! command line, as libavformat finds them: what a program calling
//! avformat_open_input and avformat_find_stream_info is told.
//!
//! Built for wasm32-unknown-emscripten and linked with ffmpeg-remux.wasm by
//! `cargo xtask check-module`, which puts the generated bindings beside it.

#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]
#![allow(dead_code)]

include!("ffmpeg_remux_bindings.rs");

use std::ffi::{c_char, c_int, CStr, CString};
use std::process::exit;
use std::ptr::{null, null_mut};

extern "C" {
    fn avformat_open_input(
        context: *mut *mut AVFormatContext,
        url: *const c_char,
        format: *const u8,
        options: *mut *mut AVDictionary,
    ) -> c_int;
    fn avformat_find_stream_info(
        context: *mut AVFormatContext,
        options: *mut *mut AVDictionary,
    ) -> c_int;
    fn avformat_close_input(context: *mut *mut AVFormatContext);
    fn avcodec_get_name(id: AVCodecID::Type) -> *const c_char;
}

fn main() {
    let Some(file) = std::env::args().nth(1) else {
        eprintln!("usage: probe <file>");
        exit(2);
    };
    let file = CString::new(file).expect("a file name has no NUL");
    // SAFETY: the calls follow libavformat's documented use: a null context is
    // filled in by avformat_open_input, and the streams are read only after
    // avformat_find_stream_info, within nb_streams.
    unsafe {
        let mut context: *mut AVFormatContext = null_mut();
        let error = avformat_open_input(
            &mut context,
            file.as_ptr(),
            null(),
            null_mut(),
        );
        if error < 0 {
            eprintln!("cannot open the file (error {error})");
            exit(1);
        }
        let error = avformat_find_stream_info(context, null_mut());
        if error < 0 {
            eprintln!("cannot read the streams (error {error})");
            exit(1);
        }
        for index in 0..(*context).nb_streams as usize {
            let stream = *(*context).streams.add(index);
            let parameters = (*stream).codecpar;
            let name = CStr::from_ptr(avcodec_get_name((*parameters).codec_id));
            println!(
                "{},{},{}",
                name.to_string_lossy(),
                (*parameters).width,
                (*parameters).height
            );
        }
        avformat_close_input(&mut context);
    }
}

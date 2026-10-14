//! `cargo xtask check-exports [exports.txt [module.wasm]]`: checks that a built
//! module exports every function in exports.txt (by default ffmpeg/exports.txt
//! and ffmpeg/dist/ffmpeg-remux.wasm). Only the module's export section is
//! read, so this needs nothing but the standard library.

use crate::{io, paths, Result};
use std::path::Path;

/// An unsigned LEB128 number at `*at`.
fn leb(bytes: &[u8], at: &mut usize) -> std::result::Result<usize, String> {
    let (mut value, mut shift) = (0usize, 0u32);
    loop {
        let byte = *bytes.get(*at).ok_or(
            "the file ends in the middle of a section; it looks truncated \
             (was the build interrupted?)",
        )?;
        *at += 1;
        if shift < usize::BITS {
            value |= usize::from(byte & 0x7F) << shift;
        }
        shift += 7;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
}

/// The names a module exports.
fn exports_of(module: &[u8]) -> std::result::Result<Vec<String>, String> {
    if module.len() < 8 || &module[..4] != b"\0asm" {
        return Err(
            "it does not start with the WebAssembly header (`\\0asm`); \
                    is it really a .wasm file?"
                .into(),
        );
    }
    let mut at = 8;
    while at < module.len() {
        let id = module[at];
        at += 1;
        let size = leb(module, &mut at)?;
        let end = at
            .checked_add(size)
            .filter(|end| *end <= module.len())
            .ok_or(
                "a section is longer than the file; it looks truncated or \
                 corrupt",
            )?;
        if id != 7 {
            at = end;
            continue;
        }
        let mut names = Vec::new();
        for _ in 0..leb(module, &mut at)? {
            let length = leb(module, &mut at)?;
            let name_end = at.checked_add(length).filter(|n| *n <= end).ok_or(
                "an export name is longer than its section; the file \
                     looks corrupt",
            )?;
            names.push(
                String::from_utf8_lossy(&module[at..name_end]).into_owned(),
            );
            at = name_end;
            at += 1; // kind
            leb(module, &mut at)?; // index
        }
        return Ok(names);
    }
    Ok(Vec::new())
}

/// The function names a list file asks for: one per line, without the leading
/// underscore Emscripten expects in the link flags.
fn wanted_names(list: &str) -> Vec<String> {
    list.lines()
        .map(|line| line.trim().trim_start_matches('_').to_string())
        .filter(|line| !line.is_empty())
        .collect()
}

/// The wanted names the module doesn't export.
fn missing<'a>(wanted: &'a [String], exported: &[String]) -> Vec<&'a str> {
    wanted
        .iter()
        .filter(|name| !exported.contains(name))
        .map(String::as_str)
        .collect()
}

/// Checks the module against the list. Paths given on the command line are
/// relative to where they were typed; the defaults are the standard locations
/// in the repository.
pub fn run(list: Option<&str>, module: Option<&str>) -> Result {
    let list = list.map_or_else(
        || paths::abs(paths::EXPORTS_LIST),
        |path| Path::new(path).to_path_buf(),
    );
    let module = module.map_or_else(
        || paths::abs(paths::MODULE),
        |path| Path::new(path).to_path_buf(),
    );
    let wanted = wanted_names(&io::read_to_string(&list)?);
    let exported = exports_of(&io::read(&module)?).map_err(|e| {
        format!("{} is not a usable module: {e}", module.display())
    })?;
    let missing = missing(&wanted, &exported);
    if !missing.is_empty() {
        return Err(format!(
            "{} does not export: {}; add them to the link flags (the `wasm` \
             task passes the list via -sEXPORTED_FUNCTIONS) or remove them \
             from {}",
            module.display(),
            missing.join(", "),
            list.display()
        )
        .into());
    }
    println!(
        "check-exports: all {} functions in {} are exported",
        wanted.len(),
        list.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A module that has only an export section, exporting each name as a
    /// function.
    fn module_exporting(names: &[&str]) -> Vec<u8> {
        let mut section = vec![names.len() as u8];
        for (index, name) in names.iter().enumerate() {
            section.push(name.len() as u8);
            section.extend_from_slice(name.as_bytes());
            section.push(0); // a function
            section.push(index as u8);
        }
        let mut module = b"\0asm\x01\0\0\0".to_vec();
        module.push(7); // the export section
        module.push(section.len() as u8);
        module.extend(section);
        module
    }

    #[test]
    fn the_exports_of_a_module_are_listed() {
        let module = module_exporting(&["av_malloc", "av_free"]);
        assert_eq!(exports_of(&module).unwrap(), ["av_malloc", "av_free"]);
        assert_eq!(
            exports_of(b"\0asm\x01\0\0\0").unwrap(),
            Vec::<String>::new()
        );
    }

    #[test]
    fn something_that_is_not_a_module_is_an_error() {
        assert!(exports_of(b"hello world").unwrap_err().contains("header"));
        let mut cut = module_exporting(&["av_malloc"]);
        cut.truncate(cut.len() - 4);
        assert!(exports_of(&cut).is_err());
    }

    #[test]
    fn the_list_drops_underscores_and_blank_lines() {
        assert_eq!(wanted_names("_a\n\n  _b \nc\n"), ["a", "b", "c"]);
    }

    #[test]
    fn missing_exports_are_found() {
        let wanted = wanted_names("_a\n_b\n_c\n");
        let exported = vec!["a".to_string(), "c".to_string()];
        assert_eq!(missing(&wanted, &exported), ["b"]);
        assert!(
            missing(&wanted, &["a".into(), "b".into(), "c".into()]).is_empty()
        );
    }
}

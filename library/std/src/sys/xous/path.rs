use crate::ffi::OsStr;
use crate::io;
use crate::path::{Path, PathBuf, Prefix};

#[inline]
pub fn is_sep_byte(b: u8) -> bool {
    b == b':'
}

#[inline]
pub fn is_verbatim_sep(b: u8) -> bool {
    b == b':'
}

#[inline]
pub fn parse_prefix(_: &OsStr) -> Option<Prefix<'_>> {
    None
}

pub const MAIN_SEP_STR: &str = ":";
pub const MAIN_SEP: char = ':';

/// Make a POSIX path absolute without changing its semantics.
pub(crate) fn absolute(path: &Path) -> io::Result<PathBuf> {
    Ok(path.to_owned())
}

/// Split a path into its constituant Basis and Dict, if the path is legal.
pub(crate) fn split_basis_and_dict<'a, F: Fn() -> Option<&'a str>>(
    src: &'a str,
    default: F,
) -> Result<(Option<&'a str>, Option<&'a str>), ()> {
    let mut basis = None;
    let dict;
    if let Some(src) = src.strip_prefix(crate::path::MAIN_SEPARATOR) {
        if let Some((maybe_basis, maybe_dict)) = src.split_once(crate::path::MAIN_SEPARATOR) {
            if !maybe_basis.is_empty() {
                basis = Some(maybe_basis);
            } else {
                basis = default();
            }

            if maybe_dict.is_empty() {
                dict = None;
            } else {
                dict = Some(maybe_dict);
            }
        } else {
            if !src.is_empty() {
                basis = Some(src);
            }
            dict = None;
        }
    } else {
        if src.is_empty() {
            return Ok((basis, Some("")));
        }
        dict = Some(src);
    }

    if let Some(basis) = &basis {
        if basis.ends_with(crate::path::MAIN_SEPARATOR) {
            return Err(());
        }
    }
    if let Some(dict) = &dict {
        if dict.ends_with(crate::path::MAIN_SEPARATOR) {
            return Err(());
        }
    }
    Ok((basis, dict))
}

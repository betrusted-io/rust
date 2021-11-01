use crate::ffi::OsStr;
use crate::mem;
use crate::path::Prefix;

/// # Safety
///
/// `bytes` must be a valid wtf8 encoded slice
#[inline]
unsafe fn bytes_as_os_str(bytes: &[u8]) -> &OsStr {
    // &OsStr is layout compatible with &Slice, which is compatible with &Wtf8,
    // which is compatible with &[u8].
    unsafe { mem::transmute(bytes) }
}

#[inline]
pub fn is_sep_byte(_b: u8) -> bool {
    false
}

#[inline]
pub fn is_verbatim_sep(_b: u8) -> bool {
    false
}

pub fn parse_prefix(prefix: &OsStr) -> Option<Prefix<'_>> {
    let b = prefix.bytes();
    let mut components = b.splitn(2, |x| *x == b'|');
    let p = components.next();
    let remainder = components.next();
    if remainder.is_some() {
        Some(Prefix::DeviceNS(unsafe { bytes_as_os_str(p.unwrap()) }))
    } else {
        None
    }
}

pub const MAIN_SEP_STR: &str = "|";
pub const MAIN_SEP: char = '|';

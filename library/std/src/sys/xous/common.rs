use crate::io as std_io;

pub mod memchr {
    pub use core::slice::memchr::{memchr, memrchr};
}

// This is not necessarily correct. May want to consider making it part of the
// spec definition?
use crate::os::raw::c_char;

extern "C" {
    fn main() -> u32;
}

#[no_mangle]
pub fn _start() {
    xous::syscall::terminate_process(unsafe { main() });
}

#[cfg(not(test))]
pub unsafe fn init(_argc: isize, _argv: *const *const u8) {
}

// SAFETY: must be called only once during runtime cleanup.
// NOTE: this is not guaranteed to run, for example when the program aborts.
pub unsafe fn cleanup() {}

pub fn unsupported<T>() -> std_io::Result<T> {
    Err(unsupported_err())
}

pub fn unsupported_err() -> std_io::Error {
    std_io::Error::new(std_io::ErrorKind::Other, "operation not supported on this platform")
}

pub fn decode_error_kind(_code: i32) -> crate::io::ErrorKind {
    crate::io::ErrorKind::Other
}

pub fn abort_internal() -> ! {
    core::intrinsics::abort();
}

pub fn hashmap_random_keys() -> (u64, u64) {
    (1, 2)
}

// This enum is used as the storage for a bunch of types which can't actually
// exist.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum Void {}

pub unsafe fn strlen(mut s: *const c_char) -> usize {
    // SAFETY: The caller must guarantee `s` points to a valid 0-terminated string.
    unsafe {
        let mut n = 0;
        while *s != 0 {
            n += 1;
            s = s.offset(1);
        }
        n
    }
}

use super::unsupported;
use crate::error::Error as StdError;
use crate::ffi::{OsStr, OsString};
use crate::fmt;
use crate::io;
use crate::marker::PhantomData;
use crate::path::{self, PathBuf};

#[cfg(not(test))]
mod c_compat {
    extern "C" {
        fn main() -> u32;
    }

    #[no_mangle]
    pub extern "C" fn abort() {}

    // Used in debugging

    #[no_mangle]
    pub fn debug_print_u8(s: &[u8]) -> usize {
        #[repr(align(4096))]
        struct LendBuffer([u8; 4096]);
        let mut lend_buffer = LendBuffer([0u8; 4096]);
        let connection = crate::os::xous::services::log_server();
        for chunk in s.chunks(lend_buffer.0.len()) {
            for (dest, src) in lend_buffer.0.iter_mut().zip(chunk) {
                *dest = *src;
            }
            crate::os::xous::ffi::lend(connection, 1, &lend_buffer.0, 0, chunk.len()).unwrap();
        }
        s.len()
    }

    #[no_mangle]
    pub extern "C" fn _start() {
        #[no_mangle]
        #[used]
        pub static IMAGE_BASE: usize = 0;

        #[no_mangle]
        // #[used]
        pub static mut EH_FRM_HDR_OFFSET: usize = 0x074f_72a8;

        #[no_mangle]
        // #[used]
        pub static EH_FRM_HDR_LEN: usize = 0xd15f_027a;

        #[no_mangle]
        // #[used]
        pub static mut EH_FRM_OFFSET: usize = 0x138f_dc0e;

        #[no_mangle]
        // #[used]
        pub static EH_FRM_LEN: usize = 0x8e41_1040;

        unsafe { EH_FRM_OFFSET = EH_FRM_OFFSET.wrapping_sub(&IMAGE_BASE as *const usize as usize) };
        unsafe {
            EH_FRM_HDR_OFFSET = EH_FRM_HDR_OFFSET.wrapping_sub(&IMAGE_BASE as *const usize as usize)
        };

        if cfg!(test) {
            // Adjust the memory limit to give us 4 MB of heap for tests
            use crate::os::xous::ffi::{adjust_limit, Limits::HeapMaximum};
            let current_heap_maximum = adjust_limit(HeapMaximum, 0, 0).unwrap();
            adjust_limit(HeapMaximum, current_heap_maximum, 1024 * 1024 * 4).unwrap();
        }

        crate::os::xous::ffi::exit(unsafe { main() });
    }

    // This function is needed by the panic runtime. The symbol is named in
    // pre-link args for the target specification, so keep that in sync.
    #[no_mangle]
    // NB. used by both libunwind and libpanic_abort
    pub extern "C" fn __rust_abort() -> ! {
        crate::os::xous::ffi::exit(101);
    }
}

pub fn errno() -> i32 {
    0
}

pub fn error_string(errno: i32) -> String {
    core::convert::Into::<crate::os::xous::ffi::Error>::into(errno).to_string()
}

pub fn getcwd() -> io::Result<PathBuf> {
    unsupported()
}

pub fn chdir(_: &path::Path) -> io::Result<()> {
    unsupported()
}

pub struct SplitPaths<'a>(!, PhantomData<&'a ()>);

pub fn split_paths(_unparsed: &OsStr) -> SplitPaths<'_> {
    panic!("unsupported")
}

impl<'a> Iterator for SplitPaths<'a> {
    type Item = PathBuf;
    fn next(&mut self) -> Option<PathBuf> {
        self.0
    }
}

#[derive(Debug)]
pub struct JoinPathsError;

pub fn join_paths<I, T>(_paths: I) -> Result<OsString, JoinPathsError>
where
    I: Iterator<Item = T>,
    T: AsRef<OsStr>,
{
    Err(JoinPathsError)
}

impl fmt::Display for JoinPathsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        "not supported on this platform yet".fmt(f)
    }
}

impl StdError for JoinPathsError {
    #[allow(deprecated)]
    fn description(&self) -> &str {
        "not supported on this platform yet"
    }
}

pub fn current_exe() -> io::Result<PathBuf> {
    unsupported()
}

pub struct Env(!);

impl Iterator for Env {
    type Item = (OsString, OsString);
    fn next(&mut self) -> Option<(OsString, OsString)> {
        self.0
    }
}

pub fn env() -> Env {
    panic!("not supported on this platform")
}

pub fn getenv(_: &OsStr) -> Option<OsString> {
    None
}

pub fn setenv(_: &OsStr, _: &OsStr) -> io::Result<()> {
    Err(io::const_io_error!(io::ErrorKind::Unsupported, "cannot set env vars on this platform"))
}

pub fn unsetenv(_: &OsStr) -> io::Result<()> {
    Err(io::const_io_error!(io::ErrorKind::Unsupported, "cannot unset env vars on this platform"))
}

pub fn temp_dir() -> PathBuf {
    panic!("no filesystem on this platform")
}

pub fn home_dir() -> Option<PathBuf> {
    None
}

pub fn exit(code: i32) -> ! {
    crate::os::xous::ffi::exit(code as u32);
}

pub fn getpid() -> u32 {
    panic!("no pids on this platform")
}

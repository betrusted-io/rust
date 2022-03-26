#![deny(unsafe_op_in_unsafe_fn)]

pub mod alloc;
pub mod args;
pub mod cmath;
pub mod condvar;
pub mod env;
#[path = "../unsupported/fs.rs"]
pub mod fs;
#[path = "../unsupported/io.rs"]
pub mod io;
pub mod mutex;
pub mod net;
pub mod os;
#[path = "../unix/os_str.rs"]
pub mod os_str;
pub mod path;
#[path = "../unsupported/pipe.rs"]
pub mod pipe;
#[path = "../unsupported/process.rs"]
pub mod process;
pub mod rwlock;
pub mod stdio;
pub mod services;
pub mod thread;
pub mod thread_local_dtor;
pub mod thread_local_key;
pub mod time;

mod common;
pub use common::*;

// This function is needed by the panic runtime. The symbol is named in
// pre-link args for the target specification, so keep that in sync.
#[cfg(not(test))]
#[no_mangle]
// NB. used by both libunwind and libpanic_abort
pub extern "C" fn __rust_abort() {
    xous::syscall::terminate_process(1);
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn abort() {
    xous::syscall::terminate_process(1);
}

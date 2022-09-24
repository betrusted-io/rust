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
pub mod mem;
pub mod mutex;
#[path = "../unsupported/net.rs"]
pub mod net;
pub mod os;
#[path = "../unix/path.rs"]
pub mod path;
#[path = "../unsupported/pipe.rs"]
pub mod pipe;
#[path = "../unsupported/process.rs"]
pub mod process;
pub mod rwlock;
#[path = "../unsupported/stack_overflow.rs"]
pub mod stack_overflow;
pub mod stdio;
pub mod thread;
#[path = "../unsupported/thread_local_dtor.rs"]
pub mod thread_local_dtor;
#[path = "../unsupported/thread_local_key.rs"]
pub mod thread_local_key;
#[path = "../unsupported/time.rs"]
pub mod time;

mod common;
pub use common::*;

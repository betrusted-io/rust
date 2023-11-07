#![deny(unsafe_op_in_unsafe_fn)]

pub mod alloc;
pub mod args;
pub mod env;
pub mod fs;
#[path = "../unsupported/io.rs"]
pub mod io;
pub mod locks;
pub mod net;
pub mod os;
pub mod path;
#[path = "../unsupported/pipe.rs"]
pub mod pipe;
#[path = "../unsupported/process.rs"]
pub mod process;
pub mod stdio;
pub mod thread;
pub mod thread_local_key;
pub mod thread_parking;
pub mod time;

#[path = "../unsupported/common.rs"]
mod common;
pub use common::*;

mod senres;

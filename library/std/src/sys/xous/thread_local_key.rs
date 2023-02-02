use crate::ptr;
use crate::sync::atomic::AtomicUsize;
use crate::sync::atomic::Ordering;
use core::arch::asm;

use crate::os::xous::ffi::MemoryFlags;

mod sync_bitset;
use sync_bitset::{SyncBitset, SYNC_BITSET_INIT};

#[cfg_attr(test, linkage = "available_externally")]
#[export_name = "_ZN16__rust_internals3std3sys3xous3thread_local_key14TLS_KEY_IN_USEE"]
static TLS_KEY_IN_USE: SyncBitset = SYNC_BITSET_INIT;
macro_rules! dup {
    ((* $($exp:tt)*) $($val:tt)*) => (dup!( ($($exp)*) $($val)* $($val)* ));
    (() $($val:tt)*) => ([$($val),*])
}
#[cfg_attr(test, linkage = "available_externally")]
#[export_name = "_ZN16__rust_internals3std3sys3xous3thread_local_key14TLS_DESTRUCTORE"]
static TLS_DESTRUCTOR: [AtomicUsize; TLS_KEYS] = dup!((* * * * * * *) (AtomicUsize::new(0)));

#[cfg(target_pointer_width = "64")]
const USIZE_BITS: usize = 64;
#[cfg(target_pointer_width = "32")]
const USIZE_BITS: usize = 32;

const TLS_KEYS: usize = 128; // Same as POSIX minimum
const TLS_KEYS_BITSET_SIZE: usize = (TLS_KEYS + (USIZE_BITS - 1)) / USIZE_BITS;

/// Thread Local Storage
/// Currently, we are limited to 1023 TLS entries. The entries
/// live in a page of memory that's unique per-process, and is
/// stored in the `$tp` register. If this register is 0, then
/// TLS has not been initialized and thread cleanup can be skipped.
///
/// The index into this register is the `key`. This key is identical
/// between all threads, but indexes a different offset within this
/// pointer.

pub type Key = usize;
pub type Dtor = unsafe extern "C" fn(*mut u8);

const TLS_MEMORY_SIZE: usize = 4096;

fn tls_ptr_addr() -> *mut *mut u8 {
    let mut tp: usize;
    unsafe {
        asm!(
            "mv {}, tp",
            out(reg) tp,
        );
    }
    core::ptr::from_exposed_addr_mut::<*mut u8>(tp)
}

/// Create an area of memory that's unique per thread. This area will
/// contain all thread local pointers.
fn tls_ptr() -> *mut *mut u8 {
    let mut tp = tls_ptr_addr();

    // If the TP register is `0`, then this thread hasn't initialized
    // its TLS yet. Allocate a new page to store this memory.
    if tp.is_null() {
        let tp_range: &mut [*mut *mut u8] = crate::os::xous::ffi::map_memory(
            None,
            None,
            TLS_MEMORY_SIZE / core::mem::size_of::<usize>(),
            MemoryFlags::R | MemoryFlags::W,
        )
        .expect("Unable to allocate memory for thread local storage");

        for element in tp_range.iter() {
            assert!(element.is_null());
        }

        tp = tp_range.as_mut_ptr() as *mut *mut u8;

        unsafe {
            // Set the thread's `$tp` register
            asm!(
                "mv tp, {}",
                in(reg) tp as usize,
            );
        }
    }
    tp
}

#[repr(C)]
pub struct Tls {
    data: [core::cell::Cell<*mut u8>; TLS_KEYS],
}

unsafe fn current<'a>() -> &'a Tls {
    // FIXME: Needs safety information. See entry.S for `set_tls_ptr` definition.
    unsafe { &*(tls_ptr() as *const Tls) }
}

#[inline]
pub unsafe fn create(dtor: Option<Dtor>) -> Key {
    let index =
        if let Some(index) = TLS_KEY_IN_USE.set() { index } else { rtabort!("TLS limit exceeded") };
    TLS_DESTRUCTOR[index].store(dtor.map_or(0, |f| f as usize), Ordering::Relaxed);
    unsafe { current() }.data[index].set(ptr::null_mut());
    index
}

#[inline]
pub unsafe fn set(key: Key, value: *mut u8) {
    rtassert!(TLS_KEY_IN_USE.get(key));
    unsafe { current() }.data[key].set(value);
}

#[inline]
pub unsafe fn get(key: Key) -> *mut u8 {
    rtassert!(TLS_KEY_IN_USE.get(key));
    unsafe { current() }.data[key].get()
}

#[inline]
pub unsafe fn destroy(key: Key) {
    TLS_KEY_IN_USE.clear(key);
}

pub unsafe fn destroy_tls() {
    let tp = tls_ptr_addr();

    // If the pointer address is 0, then this thread has no TLS.
    if tp.is_null() {
        return;
    }
    unsafe { run_dtors() };

    // Finally, free the TLS array
    let tp = tp as *mut Tls as *mut usize;
    crate::os::xous::ffi::unmap_memory(unsafe {
        core::slice::from_raw_parts_mut(tp, TLS_MEMORY_SIZE / core::mem::size_of::<usize>())
    })
    .unwrap();
}

unsafe fn run_dtors() {
    let tls = unsafe { current() };
    let value_with_destructor = |key: usize| {
        let ptr = TLS_DESTRUCTOR[key].load(Ordering::Relaxed);
        unsafe { core::mem::transmute::<_, Option<unsafe extern "C" fn(*mut u8)>>(ptr) }
            .map(|dtor| (&tls.data[key], dtor))
    };

    let mut any_non_null_dtor = true;
    while any_non_null_dtor {
        any_non_null_dtor = false;
        for (value, dtor) in TLS_KEY_IN_USE.iter().filter_map(&value_with_destructor) {
            let value = value.replace(ptr::null_mut());
            if !value.is_null() {
                any_non_null_dtor = true;
                unsafe { dtor(value) }
            }
        }
    }
}

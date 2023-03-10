use core::arch::asm;
use core::cell::Cell;
use core::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};

const TLS_KEY_COUNT: usize = 128;
const TLS_MEMORY_SIZE: usize = 4096;

pub type Key = usize;
pub type Dtor = unsafe extern "C" fn(*mut u8);

/// Common data that is shared among all threads. This could go into a `gp` regsiter,
/// but for now we just put it in the data section.
#[derive(Debug)]
struct TlsCommon {
    allocation: [AtomicU8; TLS_KEY_COUNT],
    destructors: [AtomicUsize; TLS_KEY_COUNT],
}

/// Per-thread storage. The index into `data` is managed by the `keys` entry of
/// TlsCommon.
#[repr(C, align(4096))]
#[derive(Debug)]
struct Tls {
    data: [Cell<*mut u8>; TLS_KEY_COUNT],
    used: [AtomicBool; TLS_KEY_COUNT],
}

static TLS_COMMON: TlsCommon = TlsCommon {
    allocation: unsafe { core::mem::transmute([0u8; TLS_KEY_COUNT]) },
    destructors: unsafe { core::mem::transmute([0usize; TLS_KEY_COUNT]) },
};

fn tls_ptr_addr() -> *const Tls {
    let mut tp: usize;
    unsafe {
        asm!(
            "mv {}, tp",
            out(reg) tp,
        );
    }
    core::ptr::from_exposed_addr_mut::<Tls>(tp)
}

/// Create an area of memory that's unique per thread. This area will
/// contain all thread local pointers.
fn tls_ptr() -> *const Tls {
    let mut tp = tls_ptr_addr();

    // If the TP register is `0`, then this thread hasn't initialized
    // its TLS yet. Allocate a new page to store this memory.
    if tp.is_null() {
        let syscall = xous::SysCall::MapMemory(
            None,
            None,
            xous::MemorySize::new(TLS_MEMORY_SIZE).unwrap(),
            xous::MemoryFlags::R | xous::MemoryFlags::W,
        );

        let Ok(xous::Result::MemoryRange(mem)) = xous::rsyscall(syscall) else {
            panic!("unable to allocate memory for thread local storage")
        };

        tp = mem.as_mut_ptr() as *const Tls;
        // unsafe { (tp as *mut usize).write_volatile(0) };
        let tp_usize = tp as usize;
        assert!((tp_usize & 0x3ff) == 0);

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

fn current<'a>() -> &'a Tls {
    unsafe { &*tls_ptr() }
}

#[inline]
/// Create a brand-new "Key". A "Key" is a global index into a local array. Keys
/// are shared among all threads and point to the same index. What's different
/// is the `$tp` pointer, which gives a different table for each thread.
///
/// When a key is created, an optional destructor is passed. This destructor os
/// added to a table that's the same size as the maximum number of keys.
pub unsafe fn create(dtor: Option<Dtor>) -> Key {
    // Implementation detail: skip key 0
    for (index, (allocated, destructor)) in
        TLS_COMMON.allocation.iter().zip(TLS_COMMON.destructors.iter()).enumerate()
    {
        // Find an entry in the `allocated` list that is currently 0 and set it to 1,
        // indicating it's in use. This will keep track of the number of threads that
        // are using this key, and when it reaches 0 it will be available for use again.
        if allocated.compare_exchange(0, 1, Ordering::Relaxed, Ordering::Relaxed).is_ok() {
            destructor.store(dtor.map_or(0, |f| f as usize), Ordering::Relaxed);
            return index + 1;
        }
    }
    rtabort!("TLS limit exceeded: {:x?}", TLS_COMMON);
}

#[inline]
pub unsafe fn set(key: Key, value: *mut u8) {
    let index = key - 1;
    let tls = current();

    // If this is the first access to this key in this thread, increment the
    // common in-use counter.
    if !tls.used[index].swap(true, Ordering::Relaxed) {
        TLS_COMMON.allocation[index].fetch_add(1, Ordering::Relaxed);
    }

    tls.data[index].set(value);
}

#[inline]
pub unsafe fn get(key: Key) -> *mut u8 {
    let index = key - 1;
    let tls = current();

    // If this is the first access to this key in this thread, increment the
    // common in-use counter.
    if !tls.used[index].swap(true, Ordering::Relaxed) {
        rtassert!(TLS_COMMON.allocation[index].fetch_add(1, Ordering::Relaxed) != 0);
    }
    tls.data[index].get()
}

#[inline]
pub unsafe fn destroy(key: Key) {
    if key == 0 {
        return;
    }
    let index = key - 1;
    rtassert!(TLS_COMMON.allocation[index].fetch_sub(1, Ordering::SeqCst) == 1);
}

static LAST_TP: AtomicUsize = AtomicUsize::new(0);

pub unsafe fn destroy_tls() {
    let tp = tls_ptr_addr();
    let tp_usize = tp as usize;
    if tp_usize & 0x3ff != 0 {
        rtprintpanic!("Something broke!");
        loop {}
    }
    // assert!((tp_usize & 0x3ff) == 0);

    // If the pointer address is 0, then this thread has no TLS.
    if tp.is_null() {
        return;
    }
    unsafe { run_dtors() };

    // Finally, free the TLS array
    let tp = tp as *mut Tls as *mut u8;
    let previous_tp = LAST_TP.swap(tp_usize, Ordering::Relaxed);
    if tp_usize == previous_tp {
        rtprintpanic!("Tried to destroy_tls() twice with the same TLS! {:08x}", previous_tp);
        loop {}
    }
    let tls_memory = unsafe { xous::MemoryRange::new(tp as usize, TLS_MEMORY_SIZE).unwrap() };
    let syscall = xous::SysCall::UnmapMemory(tls_memory);
    xous::rsyscall(syscall).unwrap();

    unsafe { asm!("mv tp, x0") };
}

unsafe fn run_dtors() {
    let tls = current();
    for (idx, (((data, in_use), allocation), destructor)) in tls
        .data
        .iter()
        .zip(tls.used.iter())
        .zip(TLS_COMMON.allocation.iter())
        .zip(TLS_COMMON.destructors.iter())
        .enumerate()
    {
        // Skip keys that aren't in use by this thread
        let beforehand = in_use.load(Ordering::Relaxed);
        if !in_use.swap(false, Ordering::Relaxed) {
            continue;
        }

        let data = data.replace(core::ptr::null_mut());
        if !data.is_null() {
            let destructor = destructor.load(Ordering::Relaxed);
            if let Some(destructor) = unsafe {
                core::mem::transmute::<_, Option<unsafe extern "C" fn(*mut u8)>>(destructor)
            } {
                unsafe { destructor(data) };
            }
        }

        // Remove one key from the global in-use pool, panicking if it wasn't
        // actually in use.
        if allocation.fetch_sub(1, Ordering::Relaxed) == 0 {
            rtprintpanic!(
                "allocation at {:08x} went negative ({:?}) at index {}? {:?} --- {:?}",
                tls as *const Tls as usize,
                beforehand,
                idx,
                tls,
                TLS_COMMON
            );
            rtassert!(1 == 0);
        }
        // rtassert!(allocation.fetch_sub(1, Ordering::Relaxed) != 0);
    }
}

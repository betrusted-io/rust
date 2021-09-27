use crate::mem::ManuallyDrop;
use crate::ptr;
use crate::sync::atomic::AtomicPtr;
use crate::sync::atomic::Ordering::SeqCst;

pub type Key = usize;
pub type Dtor = unsafe extern "C" fn(*mut u8);

const TLS_MEMORY_SIZE: usize = 4096;

fn tls_ptr_addr() -> usize {
    let mut tp: usize;
    unsafe {
        asm!(
            "mv {}, tp",
            out(reg) tp,
        );
    }
    tp
}

fn tls_ptr() -> *mut usize {
    let mut tp = tls_ptr_addr();

    // If the TP register is `0`, then this thread hasn't initialized
    // its TLS yet. Allocate a new page to store this memory.
    if tp == 0 {
        let syscall = xous::SysCall::MapMemory(
            None,
            None,
            xous::MemorySize::new(TLS_MEMORY_SIZE).unwrap(),
            0b110,
        );
        if let Ok(xous::Result::MemoryRange(mem)) = xous::rsyscall(syscall) {
            tp = mem.as_ptr() as usize;
            unsafe {
                (tp as *mut usize).write_volatile(1);
                asm!(
                    "mv tp, {}",
                    in(reg) tp,
                );
            }
        } else {
            panic!("Unable to allocate memory for thread local storage");
        }
    }
    tp as *mut usize
}

fn get_dtors() -> &'static mut AtomicPtr<Node> {
    let tp = tls_ptr();
    unsafe { &mut *(tp.add(1).read_volatile() as *mut AtomicPtr<Node>) }
}

/// Allocate a new TLS value.
/// The current TLS value is stored at the value pointed to by the
/// `tp` register. As all accesses are defined as offsets of this
/// address, and the destructor node is at offset 1, we begin at `2`.
fn tls_alloc() -> usize {
    let tp = tls_ptr();
    let new_tls_val = unsafe { tp.read_volatile() } + 1;
    unsafe { tp.write_volatile(new_tls_val) };
    new_tls_val
}

#[inline]
pub unsafe fn create(dtor: Option<Dtor>) -> Key {
    let key = tls_alloc();
    if let Some(f) = dtor {
        unsafe { register_dtor(key, f) };
    }
    key
}

#[inline]
pub unsafe fn set(key: Key, value: *mut u8) {
    assert!((key < 1022) && (key > 1));
    unsafe { tls_ptr().add(key).write_volatile(value as usize) };
}

#[inline]
pub unsafe fn get(key: Key) -> *mut u8 {
    assert!((key < 1022) && (key > 1));
    unsafe { tls_ptr().add(key).read_volatile() as *mut u8 }
}

#[inline]
pub unsafe fn destroy(_key: Key) {
    panic!("can't destroy keys on Xous");
}

#[inline]
pub fn requires_synchronized_create() -> bool {
    true
}

// -------------------------------------------------------------------------
// Dtor registration (stolen from Windows)
//
// Xous has no native support for running destructors so we manage our own
// list of destructors to keep track of how to destroy keys. We then install a
// callback later to get invoked whenever a thread exits, running all
// appropriate destructors.
//
// Currently unregistration from this list is not supported. A destructor can be
// registered but cannot be unregistered. There's various simplifying reasons
// for doing this, the big ones being:
//
// 1. Currently we don't even support deallocating TLS keys, so normal operation
//    doesn't need to deallocate a destructor.
// 2. There is no point in time where we know we can unregister a destructor
//    because it could always be getting run by some remote thread.
//
// Typically processes have a statically known set of TLS keys which is pretty
// small, and we'd want to keep this memory alive for the whole process anyway
// really.
//
// Perhaps one day we can fold the `Box` here into a static allocation,
// expanding the `StaticKey` structure to contain not only a slot for the TLS
// key but also a slot for the destructor queue on windows. An optimization for
// another day!

struct Node {
    dtor: Dtor,
    key: Key,
    next: *mut Node,
}

pub unsafe fn register_dtor(key: Key, dtor: Dtor) {
    let mut node = ManuallyDrop::new(Box::new(Node { key, dtor, next: ptr::null_mut() }));

    let mut head = get_dtors().load(SeqCst);
    loop {
        node.next = head;
        match get_dtors().compare_exchange(head, &mut **node, SeqCst, SeqCst) {
            Ok(_) => return, // nothing to drop, we successfully added the node to the list
            Err(cur) => head = cur,
        }
    }
}

pub unsafe fn destroy_tls() {
    let tp = tls_ptr_addr();

    // If the pointer address is 0, then this thread has no TLS.
    if tp == 0 {
        return;
    }
    unsafe { run_dtors() };

    // Finally, free the TLS array
    let tls_memory = xous::MemoryRange::new(tp, TLS_MEMORY_SIZE).unwrap();
    let syscall = xous::SysCall::UnmapMemory(tls_memory);
    xous::rsyscall(syscall).unwrap();
}

unsafe fn run_dtors() {
    let mut any_run = true;
    for _ in 0..5 {
        if !any_run {
            break;
        }
        any_run = false;
        let mut cur = get_dtors().load(SeqCst);
        while !cur.is_null() {
            let ptr = unsafe { get((*cur).key) };

            // If this node still has a data pointer attached to it, attempt to run
            // the destructor.
            if !ptr.is_null() {
                unsafe { set((*cur).key, ptr::null_mut()) };
                unsafe { ((*cur).dtor)(ptr as *mut _) };
                any_run = true;
            }

            cur = unsafe { (*cur).next };
        }
    }
}

use crate::cell::UnsafeCell;
use crate::sync::atomic::{AtomicU32, AtomicUsize, Ordering::SeqCst};
use crate::sys::thread;
use xous::syscall::yield_slice;

pub struct Mutex {
    locked: AtomicUsize,
}

pub type MovableMutex = Mutex;

impl Mutex {
    pub const fn new() -> Mutex {
        Mutex { locked: AtomicUsize::new(0) }
    }

    #[inline]
    pub unsafe fn init(&mut self) {}

    #[inline]
    pub unsafe fn lock(&self) {
        while ! unsafe { self.try_lock() } {
            yield_slice();
        }
    }

    #[inline]
    pub unsafe fn unlock(&self) {
        let prev = self.locked.swap(0, SeqCst);
        debug_assert_eq!(prev, 1);
    }

    #[inline]
    pub unsafe fn try_lock(&self) -> bool {
        self.locked.compare_exchange(0, 1, SeqCst, SeqCst).is_ok()
    }

    #[inline]
    pub unsafe fn destroy(&self) {}
}

// All empty stubs because this platform does not yet support threads, so lock
// acquisition always succeeds.
pub struct ReentrantMutex {
    owner: AtomicU32,
    recursions: UnsafeCell<u32>,
}

unsafe impl Send for ReentrantMutex {}
unsafe impl Sync for ReentrantMutex {}

impl ReentrantMutex {
    pub const unsafe fn uninitialized() -> ReentrantMutex {
        ReentrantMutex { owner: AtomicU32::new(0), recursions: UnsafeCell::new(0) }
    }

    pub unsafe fn init(&self) {}

    pub unsafe fn lock(&self) {
        let me = thread::my_id();
        while let Err(_owner) = unsafe { self._try_lock(me) } {
            yield_slice();
        }
    }

    #[inline]
    pub unsafe fn try_lock(&self) -> bool {
        unsafe { self._try_lock(thread::my_id()).is_ok() }
    }

    #[inline]
    unsafe fn _try_lock(&self, id: u32) -> Result<(), u32> {
        let id = id.checked_add(1).unwrap();
        match self.owner.compare_exchange(0, id, SeqCst, SeqCst) {
            // we transitioned from unlocked to locked
            Ok(_) => {
                debug_assert_eq!(unsafe { *self.recursions.get() }, 0);
                Ok(())
            }

            // we currently own this lock, so let's update our count and return
            // true.
            Err(n) if n == id => {
                unsafe { *self.recursions.get() += 1 };
                Ok(())
            }

            // Someone else owns the lock, let our caller take care of it
            Err(other) => Err(other),
        }
    }

    pub unsafe fn unlock(&self) {
        // If we didn't ever recursively lock the lock then we fully unlock the
        // mutex and wake up a waiter, if any. Otherwise we decrement our
        // recursive counter and let some one else take care of the zero.
        match unsafe { *self.recursions.get() } {
            0 => {
                self.owner.swap(0, SeqCst);
                // // SAFETY: the caller must gurantee that `self.ptr()` is valid i32.
                // unsafe {
                //     wasm32::memory_atomic_notify(self.ptr() as *mut i32, 1);
                // } // wake up one waiter, if any
            }
            ref mut n => *n -= 1,
        }
    }

    pub unsafe fn destroy(&self) {}
}

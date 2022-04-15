use crate::cell::UnsafeCell;
use crate::sync::atomic::{AtomicU32, AtomicUsize, Ordering::SeqCst};
use crate::sys::services::ticktimer;
use crate::sys::thread;
use xous::syscall::yield_slice;

pub struct Mutex {
    /// The "locked" value indicates how many threads are waiting on this
    /// Mutex. Possible values are:
    ///     0: The lock is unlocked
    ///     1: The lock is locked and unpoisoned
    ///   >=2: The lock is locked and poisoned
    ///
    /// A lock is "poisoned" when there is heavy contention for a lock,
    /// or it is locked for long periods of time. Rather than spinning,
    /// these locks send a Message to the ticktimer server requesting
    /// that they be woken up when a lock is unlocked.
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
        // Try multiple times to acquire the lock without resorting to the ticktimer
        // server. For locks that are held for a short amount of time, this will
        // result in the ticktimer server never getting invoked. This will result
        // in the lock being either 0 or 1
        for _attempts in 0..3 {
            if unsafe { self.try_lock() } {
                return;
            }
            yield_slice();
        }

        // Try one more time to lock. If the lock is released between the previous code and
        // here, then the inner `locked` value will be 1 at the end of this. If it was not
        // locked, then the value will be more than 1, for example if there are multiple other
        // threads waiting on this lock.
        if unsafe { self.try_lock_or_poison() } {
            return;
        }

        // The lock is now "poisoned". When the lock is released, a Message will get sent to the
        // ticktimer server to wake it up. Note that this may already have happened, so the actual
        // value of `lock` may be anything (0, 1, 2, ...).
        xous::send_message(
            ticktimer(),
            xous::Message::new_blocking_scalar(
                6, /* LockMutex */
                self as *const Mutex as usize,
                0,
                0,
                0,
            ),
        )
        .expect("Ticktimer: failure to send LockMutex command");
    }

    #[inline]
    pub unsafe fn unlock(&self) {
        let prev = self.locked.fetch_sub(1, SeqCst);

        // If the previous value was 1, then this was a "fast path" unlock, so no
        // need to involve the Ticktimer server
        if prev == 1 {
            return;
        }

        // If it was 0, then something has gone seriously wrong and the counter
        // has just wrapped around.
        if prev == 0 {
            panic!("mutex lock count underflowed");
        }

        // Unblock one thread that is waiting on this message.
        xous::send_message(
            ticktimer(),
            xous::Message::new_scalar(
                7, /* UnlockMutex */
                self as *const Mutex as usize,
                0,
                0,
                0,
            ),
        )
        .expect("Ticktimer: failure to send UnlockMutex command");
    }

    #[inline]
    pub unsafe fn try_lock(&self) -> bool {
        self.locked.compare_exchange(0, 1, SeqCst, SeqCst).is_ok()
    }

    #[inline]
    pub unsafe fn try_lock_or_poison(&self) -> bool {
        self.locked.fetch_add(1, SeqCst) == 0
    }

    #[inline]
    pub unsafe fn destroy(&self) {}
}

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

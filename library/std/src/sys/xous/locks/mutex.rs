use crate::sync::atomic::{AtomicUsize, Ordering::SeqCst};
use crate::sys::services::ticktimer;
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
}

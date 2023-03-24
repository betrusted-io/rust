use crate::os::xous::ffi::{blocking_scalar, do_yield};
use crate::os::xous::services::{ticktimer_server, TicktimerScalar};
use crate::sync::atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed, Ordering::SeqCst};

pub struct Mutex {
    /// The "locked" value indicates how many threads are waiting on this
    /// Mutex. Possible values are:
    ///     0: The lock is unlocked
    ///     1: The lock is locked and uncontended
    ///   >=2: The lock is locked and contended
    ///
    /// A lock is "contended" when there is more than one thread waiting
    /// for a lock, or it is locked for long periods of time. Rather than
    /// spinning, these locks send a Message to the ticktimer server
    /// requesting that they be woken up when a lock is unlocked.
    locked: AtomicUsize,

    /// Whether this Mutex ever was contended, and therefore made a trip
    /// to the ticktimer server. If this was never set, then we were never
    /// on the slow path and can skip deregistering the mutex.
    contended: AtomicBool,

    initialized: AtomicBool,
}

impl Mutex {
    #[inline]
    #[rustc_const_stable(feature = "const_locks", since = "1.63.0")]
    pub const fn new() -> Mutex {
        Mutex {
            locked: AtomicUsize::new(0),
            contended: AtomicBool::new(true),
            initialized: AtomicBool::new(false),
        }
    }

    pub(crate) fn index(&self) -> usize {
        self as *const Mutex as usize
    }

    fn initialize(&self) {
        if self.initialized.compare_exchange(false, true, Relaxed, Relaxed).is_ok() {
            blocking_scalar(ticktimer_server(), TicktimerScalar::UnlockMutex(self.index()).into())
                .expect("failure to send UnlockMutex command");
        }
    }

    #[inline]
    pub unsafe fn lock(&self) {
        self.initialize();
        // Try multiple times to acquire the lock without resorting to the ticktimer
        // server. For locks that are held for a short amount of time, this will
        // result in the ticktimer server never getting invoked. The `locked` value
        // will be either 0 or 1.
        for _attempts in 0..3 {
            if self.locked.compare_exchange(0, 1, SeqCst, SeqCst).is_ok() {
                return;
            }
            do_yield();
        }

        // Try one more time to lock. If the lock is released between the previous code and
        // here, then the inner `locked` value will be 1 at the end of this. If it was not
        // locked, then the value will be more than 1, for example if there are multiple other
        // threads waiting on this lock.
        if unsafe { self.try_lock_or_poison() } {
            return;
        }

        // When this mutex is dropped, we will need to deregister it with the server.
        self.contended.store(true, Relaxed);

        // The lock is now "contended". When the lock is released, a Message will get sent to the
        // ticktimer server to wake it up. Note that this may already have happened, so the actual
        // value of `lock` may be anything (0, 1, 2, ...).
        let adjust =
            blocking_scalar(ticktimer_server(), TicktimerScalar::LockMutex(self.index()).into())
                .expect("failure to send LockMutex command")[0];

        // The "lock" call returns an adjustment value. This is because some paths such as condvars
        // don't flow through the "unlock()" path and instead go directly to the ticktimer server,
        // bypassing the unlock flow in order to force contention.
        if adjust > 0 {
            // println!("!!!!!!!!!!!!!!!!!!!!!!!! removing {} entries from mutex", adjust);
            self.locked.fetch_sub(adjust, SeqCst);
        }
        assert_ne!(self.locked.load(Relaxed), 0);
    }

    pub(crate) fn contended(&self) -> bool {
        self.contended.load(SeqCst)
    }

    pub(crate) fn set_contended(&self) {
        self.contended.store(true, SeqCst);
    }

    #[inline]
    pub unsafe fn unlock(&self) {
        self.initialize();
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
        blocking_scalar(ticktimer_server(), TicktimerScalar::UnlockMutex(self.index()).into())
            .expect("failure to send UnlockMutex command");
    }

    #[inline]
    pub unsafe fn try_lock(&self) -> bool {
        self.initialize();
        if !self.contended() {
            return self.locked.compare_exchange(0, 1, SeqCst, SeqCst).is_ok();
        }

        // For a contended lock, the lock may have been released by sending a condvar message,
        // in which case we will need to consult the ticktimer server to try and unlock it.
        let result =
            blocking_scalar(ticktimer_server(), TicktimerScalar::TryLockMutex(self.index()).into())
                .expect("failure to send TryLockMutex command");
        let success = result[1] == 0;
        let adjust = result[0];

        if success {
            self.locked
                .compare_exchange(0, 1, SeqCst, SeqCst)
                .expect("someone else grabbed the lock");
        }

        if adjust > 0 {
            // println!("!!!!!!!!!!!!!!!!!!!!!!!! removing {} entries from mutex", adjust);
            self.locked.fetch_sub(adjust, SeqCst);
        }
        success
    }

    #[inline]
    unsafe fn try_lock_or_poison(&self) -> bool {
        self.locked.fetch_add(1, SeqCst) == 0
    }
}

impl Drop for Mutex {
    fn drop(&mut self) {
        // Ensure this Mutex is unlocked prior to dropping it
        assert_eq!(self.locked.load(Relaxed), 0);

        // If there was Mutex contention, then we involved the ticktimer. Free
        // the resources associated with this Mutex as it is deallocated.
        if self.contended() {
            blocking_scalar(ticktimer_server(), TicktimerScalar::FreeMutex(self.index()).into())
                .ok();
        }
    }
}

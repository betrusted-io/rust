use crate::os::xous::ffi::do_yield;
use crate::sync::atomic::{AtomicIsize, Ordering::SeqCst};

pub struct RwLock {
    /// The "mode" value indicates how many threads are waiting on this
    /// Mutex. Possible values are:
    ///    -1: The lock is locked for writing
    ///     0: The lock is unlocked
    ///   >=1: The lock is locked for reading
    ///
    /// This currently spins waiting for the lock to be freed. An
    /// optimization would be to involve the ticktimer server to
    /// coordinate unlocks.
    mode: AtomicIsize,
}

unsafe impl Send for RwLock {}
unsafe impl Sync for RwLock {}

impl RwLock {
    #[inline]
    #[rustc_const_stable(feature = "const_locks", since = "1.63.0")]
    pub const fn new() -> RwLock {
        RwLock { mode: AtomicIsize::new(0) }
    }

    #[inline]
    pub unsafe fn read(&self) {
        while !unsafe { self.try_read() } {
            do_yield();
        }
    }

    #[inline]
    pub unsafe fn try_read(&self) -> bool {
        // Attempt to lock. If the `current` value has changed, then this
        // operation will fail and we will not obtain the lock even if we
        // could potentially keep it.
        self.mode
            .fetch_update(SeqCst, SeqCst, |val| if val < 0 { None } else { Some(val + 1) })
            .is_ok()
    }

    #[inline]
    pub unsafe fn write(&self) {
        while !unsafe { self.try_write() } {
            do_yield();
        }
    }

    #[inline]
    pub unsafe fn try_write(&self) -> bool {
        self.mode.compare_exchange(0, -1, SeqCst, SeqCst).is_ok()
    }

    #[inline]
    pub unsafe fn read_unlock(&self) {
        assert!(self.mode.fetch_sub(1, SeqCst) > 0);
    }

    #[inline]
    pub unsafe fn write_unlock(&self) {
        assert_eq!(self.mode.compare_exchange(-1, 0, SeqCst, SeqCst), Ok(-1));
    }

    // only used by __rust_rwlock_unlock below
    #[inline]
    #[cfg_attr(test, allow(dead_code))]
    unsafe fn unlock(&self) {
        match self.mode.load(SeqCst) {
            0 => 0,
            x if x > 0 => self.mode.fetch_sub(1, SeqCst),
            _ => self.mode.fetch_add(1, SeqCst),
        };
    }
}

// The following functions are needed by libunwind. These symbols are named
// in pre-link args for the target specification, so keep that in sync.
#[cfg(not(test))]
const EINVAL: i32 = 22;

#[cfg(not(test))]
#[no_mangle]
pub unsafe extern "C" fn __rust_rwlock_rdlock(p: *mut RwLock) -> i32 {
    if p.is_null() {
        return EINVAL;
    }
    unsafe { (*p).read() };
    return 0;
}

#[cfg(not(test))]
#[no_mangle]
pub unsafe extern "C" fn __rust_rwlock_wrlock(p: *mut RwLock) -> i32 {
    if p.is_null() {
        return EINVAL;
    }
    unsafe { (*p).write() };
    return 0;
}

#[cfg(not(test))]
#[no_mangle]
pub unsafe extern "C" fn __rust_rwlock_unlock(p: *mut RwLock) -> i32 {
    if p.is_null() {
        return EINVAL;
    }
    unsafe { (*p).unlock() };
    return 0;
}

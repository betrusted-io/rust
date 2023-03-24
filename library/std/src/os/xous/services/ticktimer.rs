use crate::os::xous::ffi::Connection;
use core::sync::atomic::{AtomicU32, Ordering};

pub(crate) enum TicktimerScalar {
    ElapsedMs,
    SleepMs(usize),
    LockMutex(usize /* cookie */),
    TryLockMutex(usize /* cookie */),
    UnlockMutex(usize /* cookie */),
    // RegisterWaitForCondition(usize /* cookie */),
    // PerformRegisteredWaitForCondition(usize /* cookie */, usize /* timeout (ms) */),
    // PerformWaitForCondition(usize /* cookie */, usize /* timeout (ms) */),
    WaitForConditionWithMutex(
        usize, /* cookie */
        usize, /* timeout (ms) */
        usize, /* mutex index */
    ),
    NotifyCondition(usize /* cookie */, usize /* count */),
    // NotifyConditionBuffered(usize /* cookie */, usize /* count */),
    FreeMutex(usize /* cookie */),
    FreeCondition(usize /* cookie */),
}

impl Into<[usize; 5]> for TicktimerScalar {
    fn into(self) -> [usize; 5] {
        match self {
            TicktimerScalar::ElapsedMs => [0, 0, 0, 0, 0],
            TicktimerScalar::SleepMs(msecs) => [1, msecs, 0, 0, 0],
            TicktimerScalar::LockMutex(cookie) => [6, cookie, 0, 0, 0],
            TicktimerScalar::TryLockMutex(cookie) => [6, cookie, 1, 0, 0],
            TicktimerScalar::UnlockMutex(cookie) => [7, cookie, 0, 0, 0],
            // TicktimerScalar::PerformWaitForCondition(cookie, timeout_ms) => {
            //     [8, cookie, timeout_ms, 0, 0]
            // }
            TicktimerScalar::WaitForConditionWithMutex(cookie, timeout_ms, mutex) => {
                [8, cookie, timeout_ms, 0, mutex]
            }
            // TicktimerScalar::PerformRegisteredWaitForCondition(cookie, timeout_ms) => {
            //     [8, cookie, timeout_ms, 1, 0]
            // }
            TicktimerScalar::NotifyCondition(cookie, count) => [9, cookie, count, 0, 0],
            // TicktimerScalar::NotifyConditionBuffered(cookie, count) => [9, cookie, count, 1, 0],
            TicktimerScalar::FreeMutex(cookie) => [10, cookie, 0, 0, 0],
            TicktimerScalar::FreeCondition(cookie) => [11, cookie, 0, 0, 0],
            // TicktimerScalar::RegisterWaitForCondition(cookie) => [12, cookie, 0, 0, 0],
        }
    }
}

/// Return a `Connection` to the ticktimer server. This server is used for synchronization
/// primitives such as sleep, Mutex, and Condvar.
pub(crate) fn ticktimer_server() -> Connection {
    static TICKTIMER_SERVER_CONNECTION: AtomicU32 = AtomicU32::new(0);
    let cid = TICKTIMER_SERVER_CONNECTION.load(Ordering::Relaxed);
    if cid != 0 {
        return cid.into();
    }

    let cid = crate::os::xous::ffi::connect("ticktimer-server".try_into().unwrap()).unwrap();
    TICKTIMER_SERVER_CONNECTION.store(cid.into(), Ordering::Relaxed);
    cid
}

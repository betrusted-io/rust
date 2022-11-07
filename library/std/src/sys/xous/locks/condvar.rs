use super::mutex::Mutex;
use crate::os::xous::ffi::{blocking_scalar, scalar};
use crate::os::xous::services::ticktimer_server;
use crate::sync::atomic::{AtomicUsize, Ordering::SeqCst};
use crate::time::Duration;

// The implementation is inspired by Andrew D. Birrell's paper
// "Implementing Condition Variables with Semaphores"

pub struct Condvar {
    counter: AtomicUsize,
}

unsafe impl Send for Condvar {}
unsafe impl Sync for Condvar {}

impl Condvar {
    #[inline]
    #[rustc_const_stable(feature = "const_locks", since = "1.63.0")]
    pub const fn new() -> Condvar {
        Condvar { counter: AtomicUsize::new(0) }
    }

    pub unsafe fn notify_one(&self) {
        if self.counter.load(SeqCst) > 0 {
            self.counter.fetch_sub(1, SeqCst);
            scalar(ticktimer_server(), [9 /* NotifyCondition */, self.index(), 1, 0, 0])
                .expect("failure to send NotifyCondition command");
        }
    }

    pub unsafe fn notify_all(&self) {
        let counter = self.counter.swap(0, SeqCst);
        scalar(ticktimer_server(), [9 /* NotifyCondition */, self.index(), counter, 0, 0])
            .expect("failure to send NotifyCondition command");
    }

    fn index(&self) -> usize {
        self as *const Condvar as usize
    }

    pub unsafe fn wait(&self, mutex: &Mutex) {
        self.counter.fetch_add(1, SeqCst);
        unsafe { mutex.unlock() };
        blocking_scalar(ticktimer_server(), [8 /* WaitForCondition */, self.index(), 0, 0, 0])
            .expect("Ticktimer: failure to send WaitForCondition command");
        unsafe { mutex.lock() };
    }

    pub unsafe fn wait_timeout(&self, mutex: &Mutex, dur: Duration) -> bool {
        self.counter.fetch_add(1, SeqCst);
        unsafe { mutex.unlock() };
        let millis = dur.as_millis() as usize;
        let result = blocking_scalar(
            ticktimer_server(),
            [8 /* WaitForCondition */, self.index(), millis, 0, 0],
        )
        .expect("Ticktimer: failure to send WaitForCondition command");
        unsafe { mutex.lock() };

        result[0] == 0
    }
}

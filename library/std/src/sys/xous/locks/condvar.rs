use core::sync::atomic::AtomicUsize;

use super::mutex::Mutex;
use crate::os::xous::ffi::blocking_scalar;
use crate::os::xous::services::{ticktimer_server, TicktimerScalar};
use crate::sync::atomic::{AtomicIsize, Ordering::Relaxed, Ordering::SeqCst};
use crate::time::Duration;

// The implementation is inspired by Andrew D. Birrell's paper
// "Implementing Condition Variables with Semaphores"

pub struct Condvar {
    counter: AtomicIsize,
    index: AtomicUsize,
}

static RUNNING_CONDVAR_INDEX: AtomicUsize = AtomicUsize::new(1);

unsafe impl Send for Condvar {}
unsafe impl Sync for Condvar {}

impl Condvar {
    #[inline]
    #[rustc_const_stable(feature = "const_locks", since = "1.63.0")]
    pub const fn new() -> Condvar {
        Condvar { counter: AtomicIsize::new(0), index: AtomicUsize::new(0) }
    }

    pub fn notify_one(&self) {
        let result = blocking_scalar(
            ticktimer_server(),
            TicktimerScalar::NotifyCondition(self.index(), 1).into(),
        );
        let notify_count =
            result.expect("failure to send NotifyCondition command")[1].try_into().unwrap();
        self.counter.fetch_sub(notify_count, Relaxed);

        // if notify_count == 0 {
        //     println!("wanted to notify one, but notified 0 waiters for {:08x}", self.index());
        // }
        // if notify_count > 0 {
        //     crate::thread::yield_now();
        // }
    }

    pub fn notify_all(&self) {
        let result = blocking_scalar(
            ticktimer_server(),
            TicktimerScalar::NotifyCondition(self.index(), 0).into(),
        );

        let notify_count =
            result.expect("failure to send NotifyCondition command")[1].try_into().unwrap();
        self.counter.fetch_sub(notify_count, Relaxed);

        // if notify_count == 0 {
        //     println!("wanted to notify all, but notified 0 waiters for {:08x}", self.index());
        // }

        // // If we notified at least one other thread, then yield to that other thread. The other thread
        // // will wake up then try to grab the lock. However, the lock is still held by us, so it will
        // // block a second time. Then when we return from this function we will release the contended
        // // lock which will allow the notified threads to run.
        // if notify_count > 0 {
        //     crate::thread::yield_now();
        // }
    }

    fn index(&self) -> usize {
        // self as *const Condvar as usize
        let mut index = self.index.load(Relaxed);
        if index == 0 {
            index = RUNNING_CONDVAR_INDEX.fetch_add(1, SeqCst);
            self.index.store(index, Relaxed);
        }
        index
    }

    pub unsafe fn wait(&self, mutex: &Mutex) {
        self.counter.fetch_add(1, Relaxed);
        // blocking_scalar(
        //     ticktimer_server(),
        //     TicktimerScalar::RegisterWaitForCondition(self.index()).into(),
        // )
        // .unwrap();

        // // Unlocking the Mutex here allows other threads to Notify this thread. This may actually happen
        // // before we call PerformWaitForCondition, however since we pre-registered that will just
        // // result in an immediate return.
        // unsafe { mutex.unlock() };

        mutex.set_contended();
        let result = blocking_scalar(
            ticktimer_server(),
            TicktimerScalar::WaitForConditionWithMutex(self.index(), 0, mutex.index()).into(),
        )
        .expect("Ticktimer: failure to send WaitForCondition command");

        // Re-lock the Mutex. If multiple threads were awoken at the same time, this lock will be
        // very contended.
        unsafe { mutex.lock() };

        // Make sure we didn't time out, since that wouldn't make sense for this kind of wait
        assert_eq!(result[0], 0);
    }

    pub unsafe fn wait_timeout(&self, mutex: &Mutex, dur: Duration) -> bool {
        self.counter.fetch_add(1, Relaxed);

        // Ensure the millis value is nonzero
        let mut millis = dur.as_millis().try_into().unwrap();
        millis |= 1usize;

        mutex.set_contended();
        let result = blocking_scalar(
            ticktimer_server(),
            TicktimerScalar::WaitForConditionWithMutex(self.index(), millis, mutex.index()).into(),
        )
        .expect("Ticktimer: failure to send WaitForCondition command");
        unsafe { mutex.lock() };

        let success = result[0] == 0;

        // If we awoke due to a timeout, decrement the wake count, as that would not have
        // been done in the `notify()` call.
        if !success {
            self.counter.fetch_sub(1, Relaxed);
        }
        success
    }
}

impl Drop for Condvar {
    fn drop(&mut self) {
        let count = self.counter.load(SeqCst);
        if count != 0 {
            println!("!!! error: count was {} and not 0 !!!", count);
        }
        blocking_scalar(ticktimer_server(), TicktimerScalar::FreeCondition(self.index()).into())
            .ok();
    }
}

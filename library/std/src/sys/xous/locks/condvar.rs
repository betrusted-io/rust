use crate::sync::atomic::{AtomicUsize, Ordering::SeqCst};
use super::mutex::Mutex;
use crate::sys::services::ticktimer;
use crate::time::Duration;

// The implementation is inspired by Andrew D. Birrell's paper
// "Implementing Condition Variables with Semaphores"

pub struct Condvar {
    counter: AtomicUsize,
}

pub type MovableCondvar = Condvar;

unsafe impl Send for Condvar {}
unsafe impl Sync for Condvar {}

impl Condvar {
    pub const fn new() -> Condvar {
        Condvar { counter: AtomicUsize::new(0) }
    }

    pub unsafe fn init(&mut self) {}

    pub unsafe fn notify_one(&self) {
        if self.counter.load(SeqCst) > 0 {
            self.counter.fetch_sub(1, SeqCst);
            xous::send_message(
                ticktimer(),
                xous::Message::new_scalar(
                    9, /* NotifyCondition */
                    self.counter.as_mut_ptr() as usize,
                    1,
                    0,
                    0,
                ),
            )
            .expect("Ticktimer: failure to send NotifyCondition command");
        }
    }

    pub unsafe fn notify_all(&self) {
        let counter = self.counter.swap(0, SeqCst);
        xous::send_message(
            ticktimer(),
            xous::Message::new_scalar(
                9, /* NotifyCondition */
                self.counter.as_mut_ptr() as usize,
                counter,
                0,
                0,
            ),
        )
        .expect("Ticktimer: failure to send NotifyCondition command");
    }

    pub unsafe fn wait(&self, mutex: &Mutex) {
        self.counter.fetch_add(1, SeqCst);
        unsafe { mutex.unlock() };
        xous::send_message(
            ticktimer(),
            xous::Message::new_blocking_scalar(
                8, /* WaitForCondition */
                self.counter.as_mut_ptr() as usize,
                0,
                0,
                0,
            ),
        )
        .expect("Ticktimer: failure to send WaitForCondition command");
        unsafe { mutex.lock() };
    }

    pub unsafe fn wait_timeout(&self, mutex: &Mutex, dur: Duration) -> bool {
        self.counter.fetch_add(1, SeqCst);
        unsafe { mutex.unlock() };
        let millis = dur.as_millis() as usize;
        let result = xous::send_message(
            ticktimer(),
            xous::Message::new_blocking_scalar(
                8, /* WaitForCondition */
                self.counter.as_mut_ptr() as usize,
                millis,
                0,
                0,
            ),
        )
        .expect("Ticktimer: failure to send WaitForCondition command");
        unsafe { mutex.lock() };

        xous::Result::Scalar1(0) == result
    }

    pub unsafe fn destroy(&self) {}
}

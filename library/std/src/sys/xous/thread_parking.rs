use crate::os::xous::ffi::{blocking_scalar, scalar};
use crate::os::xous::services::{ticktimer_server, TicktimerScalar};
use crate::pin::Pin;
use crate::ptr;
use crate::sync::atomic::{
    AtomicI8,
    Ordering::{Acquire, Release},
};
use crate::time::Duration;

const NOTIFIED: i8 = 1;
const EMPTY: i8 = 0;
const PARKED: i8 = -1;

pub struct Parker {
    state: AtomicI8,
}

impl Parker {
    pub unsafe fn new_in_place(parker: *mut Parker) {
        unsafe { parker.write(Parker { state: AtomicI8::new(EMPTY) }) }
    }

    fn index(&self) -> usize {
        ptr::from_ref(self).addr()
    }

    pub unsafe fn park(self: Pin<&Self>) {
        // Change NOTIFIED to EMPTY and EMPTY to PARKED.
        let state = self.state.fetch_sub(1, Acquire);
        if state == NOTIFIED {
            return;
        }
        assert!(state == NOTIFIED || state == EMPTY);

        // The state was set to PARKED. Wait until the `unpark` wakes us up.
        blocking_scalar(
            ticktimer_server(),
            TicktimerScalar::WaitForCondition(self.index(), 0).into(),
        )
        .expect("failed to send WaitForCondition command");

        let new_state = self.state.swap(EMPTY, Acquire);
        assert!(new_state == PARKED || new_state == EMPTY || new_state == NOTIFIED);
    }

    pub unsafe fn park_timeout(self: Pin<&Self>, timeout: Duration) {
        // Change NOTIFIED to EMPTY and EMPTY to PARKED.
        let state = self.state.fetch_sub(1, Acquire);
        if state == NOTIFIED {
            return;
        }
        assert!(state == NOTIFIED || state == EMPTY);

        // A value of zero indicates an indefinite wait. Clamp the number of
        // milliseconds to the allowed range.
        let millis = usize::max(timeout.as_millis().try_into().unwrap_or(usize::MAX), 1);

        let _was_timeout = blocking_scalar(
            ticktimer_server(),
            TicktimerScalar::WaitForCondition(self.index(), millis).into(),
        )
        .expect("failed to send WaitForCondition command")[0]
            != 0;

        let new_state = self.state.swap(EMPTY, Acquire);
        assert!(new_state == PARKED || new_state == EMPTY || new_state == NOTIFIED);
    }

    pub fn unpark(self: Pin<&Self>) {
        // If the state is `NOTIFIED`, then another thread has notified
        // the target thread.
        // If the state is `EMPTY` then there is nothing to wake up.
        if self.state.swap(NOTIFIED, Release) != PARKED {
            return;
        }

        // The thread is parked, wake it up. Keep trying until we wake something up.
        // This will happen when the `NotifyCondition` call returns the fact that
        // 1 condition was notified.
        while blocking_scalar(
            ticktimer_server(),
            TicktimerScalar::NotifyCondition(self.index(), 1).into(),
        )
        .expect("failed to send NotifyCondition command")[0]
            == 1
        {
            // The target thread hasn't yet hit the `WaitForCondition` call.
            // Yield to let the target thread run some more.
            crate::thread::yield_now();
        }
    }
}

impl Drop for Parker {
    fn drop(&mut self) {
        scalar(ticktimer_server(), TicktimerScalar::FreeCondition(self.index()).into()).ok();
    }
}

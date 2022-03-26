use crate::time::Duration;
use crate::sys::services::{ticktimer, systime};

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct Instant(Duration);

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct SystemTime(Duration);

pub const UNIX_EPOCH: SystemTime = SystemTime(Duration::from_secs(0));

impl Instant {
    pub fn now() -> Instant {
        match xous::send_message(
            ticktimer(),
            xous::Message::new_blocking_scalar(
                0, /* ElapsedMs */
                0,
                0,
                0,
                0,
            ),
        )
        .expect("Ticktimer: failure to request elapsed_ms") {
            xous::Result::Scalar2(lower, upper) => {
                Instant {
                    0: Duration::from_millis(lower as u64 | (upper as u64) << 32)
                }
            }
            _ => panic!("Ticktimer: incorrect response when requesting elapsed_ms")
        }
    }

    pub fn checked_sub_instant(&self, other: &Instant) -> Option<Duration> {
        self.0.checked_sub(other.0)
    }

    pub fn checked_add_duration(&self, other: &Duration) -> Option<Instant> {
        Some(Instant(self.0.checked_add(*other)?))
    }

    pub fn checked_sub_duration(&self, other: &Duration) -> Option<Instant> {
        Some(Instant(self.0.checked_sub(*other)?))
    }
}

impl SystemTime {
    pub fn now() -> SystemTime {
        match xous::send_message(
            systime(),
            xous::Message::new_blocking_scalar(
                3, /* GetUtcTimeMs */
                0,
                0,
                0,
                0,
            ),
        )
        .expect("Systime: failure to request UTC time in ms") {
            xous::Result::Scalar2(upper, lower) => {
                SystemTime {
                    0: Duration::from_millis((upper as u64) << 32 | lower as u64)
                }
            }
            _ => panic!("Ticktimer: incorrect response when requesting elapsed_ms")
        }
    }

    pub fn sub_time(&self, other: &SystemTime) -> Result<Duration, Duration> {
        self.0.checked_sub(other.0).ok_or_else(|| other.0 - self.0)
    }

    pub fn checked_add_duration(&self, other: &Duration) -> Option<SystemTime> {
        Some(SystemTime(self.0.checked_add(*other)?))
    }

    pub fn checked_sub_duration(&self, other: &Duration) -> Option<SystemTime> {
        Some(SystemTime(self.0.checked_sub(*other)?))
    }
}

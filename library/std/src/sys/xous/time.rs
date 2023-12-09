use crate::os::xous::ffi::blocking_scalar;
use crate::os::xous::services::{
    systime_server, ticktimer_server, SystimeScalar::GetUtcTimeMs, TicktimerScalar::ElapsedMs,
};
use crate::time::Duration;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct Instant(Duration);

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct SystemTime(Duration);

pub const UNIX_EPOCH: SystemTime = SystemTime(Duration::from_secs(0));

impl Instant {
    pub fn now() -> Instant {
        let result = blocking_scalar(ticktimer_server(), ElapsedMs.into())
            .expect("failed to request elapsed_ms");
        /* if let Some(scalar) = msg.body.scalar_message_mut() {
              let time = ticktimer.elapsed_ms() as i64;
              scalar.arg1 = (time & 0xFFFF_FFFFi64) as usize;
              scalar.arg2 = ((time >> 32) & 0xFFF_FFFFi64) as usize;
        */
        let lower = result[0]; // corresponds to ScalarMessage.arg1
        let upper = result[1]; // corresponds to ScalarMessage.arg2
        Instant { 0: Duration::from_millis(lower as u64 | (upper as u64) << 32) }
    }

    pub fn checked_sub_instant(&self, other: &Instant) -> Option<Duration> {
        self.0.checked_sub(other.0)
    }

    pub fn checked_add_duration(&self, other: &Duration) -> Option<Instant> {
        self.0.checked_add(*other).map(Instant)
    }

    pub fn checked_sub_duration(&self, other: &Duration) -> Option<Instant> {
        self.0.checked_sub(*other).map(Instant)
    }
}

impl SystemTime {
    pub fn now() -> SystemTime {
        let result = blocking_scalar(systime_server(), GetUtcTimeMs.into())
            .expect("failed to request utc time in ms");
        /*
            Some(TimeOp::GetUtcTimeMs) => xous::msg_blocking_scalar_unpack!(msg, _, _, _, _, {
                let t =
                    start_rtc_secs as i64 * 1000i64
                    + (tt.elapsed_ms() - start_tt_ms) as i64;
                log::debug!("hw only UTC ms {}", t);
                xous::return_scalar2(msg.sender,
                    (((t as u64) >> 32) & 0xFFFF_FFFF) as usize,
                    (t as u64 & 0xFFFF_FFFF) as usize,
                ).expect("couldn't respond to GetUtcTimeMs");
            }),
        */
        let upper = result[0]; // val1
        let lower = result[1]; // val2
        SystemTime { 0: Duration::from_millis((upper as u64) << 32 | lower as u64) }
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

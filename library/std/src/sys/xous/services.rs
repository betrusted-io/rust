use core::sync::atomic::{AtomicU32, Ordering};

static TICKTIMER_CID: AtomicU32 = AtomicU32::new(0);

pub(crate) fn ticktimer() -> xous::CID {
    // Sleep is done by connecting to the ticktimer server and sending
    // a blocking message.
    let cid = TICKTIMER_CID.load(Ordering::Relaxed);
    if cid != 0 {
        return cid;
    }

    let cid = xous::connect(xous::SID::from_bytes(b"ticktimer-server").unwrap()).unwrap();
    TICKTIMER_CID.store(cid, Ordering::Relaxed);
    cid
}
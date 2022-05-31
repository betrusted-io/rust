use core::sync::atomic::{AtomicU32, Ordering};
use xous::services::nameserver as ns;

pub(crate) fn network() -> xous::CID {
    static NETWORK_CID: AtomicU32 = AtomicU32::new(0);
    let cid = NETWORK_CID.load(Ordering::Relaxed);
    if cid != 0 {
        return cid;
    }

    let cid = ns::connect("_Middleware Network Server_").unwrap();
    NETWORK_CID.store(cid, Ordering::Relaxed);
    cid
}

pub(crate) fn dns() -> xous::CID {
    static DNS_CID: AtomicU32 = AtomicU32::new(0);
    let cid = DNS_CID.load(Ordering::Relaxed);
    if cid != 0 {
        return cid;
    }

    let cid = ns::connect("_DNS Resolver Middleware_").unwrap();
    DNS_CID.store(cid, Ordering::Relaxed);
    cid
}

pub(crate) fn ticktimer() -> xous::CID {
    // Sleep is done by connecting to the ticktimer server and sending
    // a blocking message.
    static TICKTIMER_CID: AtomicU32 = AtomicU32::new(0);
    let cid = TICKTIMER_CID.load(Ordering::Relaxed);
    if cid != 0 {
        return cid;
    }

    let cid = xous::connect(xous::SID::from_bytes(b"ticktimer-server").unwrap()).unwrap();
    TICKTIMER_CID.store(cid, Ordering::Relaxed);
    cid
}

pub(crate) fn systime() -> xous::CID {
    static SYSTIME_CID: AtomicU32 = AtomicU32::new(0);
    let cid = SYSTIME_CID.load(Ordering::Relaxed);
    if cid != 0 {
        return cid;
    }

    let cid = xous::connect(xous::SID::from_bytes(b"timeserverpublic").unwrap()).unwrap();
    SYSTIME_CID.store(cid, Ordering::Relaxed);
    cid
}

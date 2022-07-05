use core::sync::atomic::{AtomicU32, Ordering};
use xous::services::nameserver as ns;

pub(crate) fn network() -> xous::CID {
    static CID: AtomicU32 = AtomicU32::new(0);
    let cid = CID.load(Ordering::Relaxed);
    if cid != 0 {
        return cid;
    }

    let cid = ns::connect("_Middleware Network Server_").unwrap();
    CID.store(cid, Ordering::Relaxed);
    cid
}

pub(crate) fn dns() -> xous::CID {
    static CID: AtomicU32 = AtomicU32::new(0);
    let cid = CID.load(Ordering::Relaxed);
    if cid != 0 {
        return cid;
    }

    let cid = ns::connect("_DNS Resolver Middleware_").unwrap();
    CID.store(cid, Ordering::Relaxed);
    cid
}

pub(crate) fn pddb() -> xous::CID {
    static CID: AtomicU32 = AtomicU32::new(0);
    let cid = CID.load(Ordering::Relaxed);
    if cid != 0 {
        return cid;
    }

    let cid = ns::connect("_Plausibly Deniable Database_").unwrap();
    CID.store(cid, Ordering::Relaxed);
    cid
}

pub(crate) fn ticktimer() -> xous::CID {
    // Sleep is done by connecting to the ticktimer server and sending
    // a blocking message.
    static CID: AtomicU32 = AtomicU32::new(0);
    let cid = CID.load(Ordering::Relaxed);
    if cid != 0 {
        return cid;
    }

    let cid = xous::connect(xous::SID::from_bytes(b"ticktimer-server").unwrap()).unwrap();
    CID.store(cid, Ordering::Relaxed);
    cid
}

pub(crate) fn systime() -> xous::CID {
    static CID: AtomicU32 = AtomicU32::new(0);
    let cid = CID.load(Ordering::Relaxed);
    if cid != 0 {
        return cid;
    }

    let cid = xous::connect(xous::SID::from_bytes(b"timeserverpublic").unwrap()).unwrap();
    CID.store(cid, Ordering::Relaxed);
    cid
}

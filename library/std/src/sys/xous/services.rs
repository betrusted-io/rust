use core::sync::atomic::{AtomicU32, Ordering};

static TICKTIMER_CID: AtomicU32 = AtomicU32::new(0);
static NETWORK_CID: AtomicU32 = AtomicU32::new(0);
static NAMESERVER_CID: AtomicU32 = AtomicU32::new(0);
static DNS_CID: AtomicU32 = AtomicU32::new(0);

mod ns {
    // By making this repr(C), the layout of this struct becomes well-defined
    // and no longer shifts around.
    // By marking it as `align(4096) we define that it will be page-aligned,
    // meaning it can be sent between processes.
    #[repr(C, align(4096))]
    struct ConnectRequest {
        name: [u8; 64],
        len: u32,
        _padding: [u8; 4096 - 4 - 64],
    }

    impl Default for ConnectRequest {
        fn default() -> Self {
            ConnectRequest { name: [0u8; 64], len: 0, _padding: [0u8; 4096 - 4 - 64] }
        }
    }

    impl ConnectRequest {
        pub fn new(name: &str) -> Option<Self> {
            let mut cr: ConnectRequest = Default::default();
            let name_bytes = name.as_bytes();

            // Ensure the bytes won't blow out the buffer
            if name_bytes.len() > 64 {
                return None;
            }

            // Copy the string into our backing store.
            for (&src_byte, dest_byte) in name_bytes.iter().zip(&mut cr.name) {
                *dest_byte = src_byte;
            }

            // Set the string length to the length of the passed-in String,
            // or the maximum possible length. Which ever is smaller.
            cr.len = 64usize.min(name.as_bytes().len()) as u32;

            // If the string is not valid, set its length to 0.
            if core::str::from_utf8(&cr.name[0..cr.len as usize]).is_err() {
                return None;
            }

            if cr.len == 0 {
                None
            } else {
                Some(cr)
            }
        }
    }

    pub fn connect(name: &str) -> Option<xous::CID> {
        let mut request = ConnectRequest::new(name)?;
        let ns_cid = super::nameserver();
        let memory_range = unsafe {
            xous::MemoryRange::new(
                &mut request as *mut ConnectRequest as usize,
                core::mem::size_of::<ConnectRequest>(),
            )
            .unwrap()
        };
        let response = xous::send_message(
            ns_cid,
            xous::Message::new_lend_mut(
                6, /* BlockingConnect */
                memory_range,
                None,
                xous::MemoryAddress::new(request.len as usize),
            ),
        )
        .expect("unable to perform lookup");
        if let xous::Result::MemoryReturned(_, _) = response {
            let response_ptr = &request as *const ConnectRequest as *const u32;
            let result = unsafe { response_ptr.read() };

            if result == 0 {
                let cid = unsafe { response_ptr.add(1).read() };
                let mut token = [0u32; 4];
                token[0] = unsafe { response_ptr.add(2).read() };
                token[1] = unsafe { response_ptr.add(3).read() };
                token[2] = unsafe { response_ptr.add(4).read() };
                token[3] = unsafe { response_ptr.add(5).read() };
                // println!("Successfully connected to {}. CID: {}, token: {:?}", name, cid, token);
                Some(cid)
            } else {
                let _error = unsafe { response_ptr.add(1).read() };
                // println!("Error connecting to {}. Type: {}  Code: {}", name, result, _error);
                None
            }
        } else {
            None
        }
    }
}

// pub(crate) fn connect(name: &str) -> Option<xous::CID> {
//     ns::connect(name)
// }

pub(crate) fn nameserver() -> xous::CID {
    let cid = NAMESERVER_CID.load(Ordering::Relaxed);
    if cid != 0 {
        return cid;
    }

    let cid = xous::connect(xous::SID::from_bytes(b"xous-name-server").unwrap()).unwrap();
    NAMESERVER_CID.store(cid, Ordering::Relaxed);
    cid
}

pub(crate) fn network() -> xous::CID {
    let cid = NETWORK_CID.load(Ordering::Relaxed);
    if cid != 0 {
        return cid;
    }

    let cid = ns::connect("_Middleware Network Server_").unwrap();
    NETWORK_CID.store(cid, Ordering::Relaxed);
    cid
}

pub(crate) fn dns() -> xous::CID {
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
    let cid = TICKTIMER_CID.load(Ordering::Relaxed);
    if cid != 0 {
        return cid;
    }

    let cid = xous::connect(xous::SID::from_bytes(b"ticktimer-server").unwrap()).unwrap();
    TICKTIMER_CID.store(cid, Ordering::Relaxed);
    cid
}

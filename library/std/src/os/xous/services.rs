use crate::os::xous::ffi::Connection;
use core::sync::atomic::{AtomicU32, Ordering};

mod ns {
    use crate::os::xous::ffi::{lend_mut, Connection};
    // By making this repr(C), the layout of this struct becomes well-defined
    // and no longer shifts around.
    // By marking it as `align(4096)` we define that it will be page-aligned,
    // meaning it can be sent between processes. We make sure to pad out the
    // entire struct so that memory isn't leaked to the nameserver.
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

            // Set the string length to the length of the passed-in String,
            // or the maximum possible length. Which ever is smaller.
            cr.len = cr.name.len().min(name_bytes.len()) as u32;

            // Copy the string into our backing store.
            for (&src_byte, dest_byte) in name_bytes.iter().zip(&mut cr.name) {
                *dest_byte = src_byte;
            }

            Some(cr)
        }
    }

    pub fn connect_with_name(name: &str) -> Option<Connection> {
        let mut request = ConnectRequest::new(name)?;
        lend_mut(
            super::nameserver(),
            6, /* BlockingConnect */
            &mut request as *mut _ as *mut u8,
            core::mem::size_of::<ConnectRequest>(),
            0,
            request.len as usize,
        )
        .expect("unable to perform lookup");

        let response_ptr = &request as *const ConnectRequest as *const u32;
        let result = unsafe { response_ptr.read() };

        if result == 0 {
            let cid = unsafe { response_ptr.add(1).read() }.into();
            // let mut token = [0u32; 4];
            // token[0] = unsafe { response_ptr.add(2).read() };
            // token[1] = unsafe { response_ptr.add(3).read() };
            // token[2] = unsafe { response_ptr.add(4).read() };
            // token[3] = unsafe { response_ptr.add(5).read() };
            // println!("Successfully connected to {}. CID: {}, token: {:?}", name, cid, token);
            Some(cid)
        } else {
            // let error = unsafe { response_ptr.add(1).read() };
            // println!("Error connecting to {}. Type: {}  Code: {}", name, result, error);
            None
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
pub fn connect(name: &str) -> Option<Connection> {
    ns::connect_with_name(name)
}

pub(crate) fn nameserver() -> Connection {
    static NAMESERVER_CONNECTION: AtomicU32 = AtomicU32::new(0);

    let cid = NAMESERVER_CONNECTION.load(Ordering::Relaxed);
    if cid != 0 {
        return cid.into();
    }

    let cid = crate::os::xous::ffi::connect("xous-name-server".try_into().unwrap()).unwrap();
    NAMESERVER_CONNECTION.store(cid.into(), Ordering::Relaxed);
    cid
}

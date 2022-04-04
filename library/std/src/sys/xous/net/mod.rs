use crate::sys::unsupported;
use crate::io;
use crate::net::SocketAddr;
use crate::fmt;

mod dns;
mod tcpstream;
pub use tcpstream::*;
mod udp;
pub use udp::*;

macro_rules! unimpl {
    () => {
        return Err(io::Error::new_const(
            io::ErrorKind::Unsupported,
            &"This function is not yet implemented",
        ));
    };
}

// this structure needs to be synchronized with what's in net/src/api.rs
#[repr(C)]
#[derive(Debug)]
enum NetError {
    // Ok = 0,
    Unaddressable = 1,
    SocketInUse = 2,
    // AccessDenied = 3,
    Invalid = 4,
    // Finished = 5,
    LibraryError = 6,
    // AlreadyUsed = 7,
    TimedOut = 8,
    WouldBlock = 9,
}

#[repr(C, align(4096))]
struct ConnectRequest {
    raw: [u8; 4096],
}

#[repr(C, align(4096))]
struct SendData {
    raw: [u8; 4096],
}

#[repr(C, align(4096))]
pub struct ReceiveData {
    raw: [u8; 4096],
}

#[repr(C, align(4096))]
pub struct GetAddress {
    raw: [u8; 4096],
}

pub struct TcpListener(!);

impl TcpListener {
    pub fn bind(_: io::Result<&SocketAddr>) -> io::Result<TcpListener> {
        unsupported()
    }

    pub fn socket_addr(&self) -> io::Result<SocketAddr> {
        self.0
    }

    pub fn accept(&self) -> io::Result<(TcpStream, SocketAddr)> {
        self.0
    }

    pub fn duplicate(&self) -> io::Result<TcpListener> {
        self.0
    }

    pub fn set_ttl(&self, _: u32) -> io::Result<()> {
        unimpl!();
    }

    pub fn ttl(&self) -> io::Result<u32> {
        unimpl!();
    }

    pub fn set_only_v6(&self, _: bool) -> io::Result<()> {
        unimpl!();
    }

    pub fn only_v6(&self) -> io::Result<bool> {
        unimpl!();
    }

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        unimpl!();
    }

    pub fn set_nonblocking(&self, _: bool) -> io::Result<()> {
        unimpl!();
    }
}

impl fmt::Debug for TcpListener {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0
    }
}

pub use dns::LookupHost;

#[allow(nonstandard_style)]
pub mod netc {
    pub const AF_INET: u8 = 0;
    pub const AF_INET6: u8 = 1;
    pub type sa_family_t = u8;

    #[derive(Copy, Clone)]
    pub struct in_addr {
        pub s_addr: u32,
    }

    #[derive(Copy, Clone)]
    pub struct sockaddr_in {
        pub sin_family: sa_family_t,
        pub sin_port: u16,
        pub sin_addr: in_addr,
    }

    #[derive(Copy, Clone)]
    pub struct in6_addr {
        pub s6_addr: [u8; 16],
    }

    #[derive(Copy, Clone)]
    pub struct sockaddr_in6 {
        pub sin6_family: sa_family_t,
        pub sin6_port: u16,
        pub sin6_addr: in6_addr,
        pub sin6_flowinfo: u32,
        pub sin6_scope_id: u32,
    }

    #[derive(Copy, Clone)]
    pub struct sockaddr {}

    pub type socklen_t = usize;
}

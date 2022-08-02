use super::super::services;
use super::*;
use crate::fmt;
use crate::io;
use crate::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use crate::sync::Arc;
use core::sync::atomic::{AtomicUsize, AtomicBool, Ordering};
use core::convert::TryInto;

macro_rules! unimpl {
    () => {
        return Err(io::const_io_error!(
            io::ErrorKind::Unsupported,
            &"This function is not yet implemented",
        ));
    };
}

#[derive(Clone)]
pub struct TcpListener {
    fd: Arc<AtomicUsize>,
    local: SocketAddr,
    handle_count: Arc<AtomicUsize>,
    nonblocking: Arc<AtomicBool>,
}

impl TcpListener {
    pub fn bind(socketaddr: io::Result<&SocketAddr>) -> io::Result<TcpListener> {
        let addr = socketaddr?;

        let fd = TcpListener::bind_inner(addr)?;
        return Ok(TcpListener {
            fd: Arc::new(AtomicUsize::new(fd)),
            local: *addr,
            handle_count: Arc::new(AtomicUsize::new(1)),
            nonblocking: Arc::new(AtomicBool::new(false)),
        });
    }

    /// This returns the raw fd of a Listener, so that it can also be used by the
    /// accept routine to replenish the Listener object after its handle has been converted into
    /// a TcpStream object.
    fn bind_inner(addr: &SocketAddr) -> io::Result<usize> {
        // Construct the request
        let mut connect_request = ConnectRequest { raw: [0u8; 4096] };

        // Serialize the StdUdpBind structure. This is done "manually" because we don't want to
        // make an auto-serdes (like bincode or rkyv) crate a dependency of Xous.
        let port_bytes = addr.port().to_le_bytes();
        connect_request.raw[0] = port_bytes[0];
        connect_request.raw[1] = port_bytes[1];
        match addr.ip() {
            IpAddr::V4(addr) => {
                connect_request.raw[2] = 4;
                for (dest, src) in connect_request.raw[3..].iter_mut().zip(addr.octets()) {
                    *dest = src;
                }
            }
            IpAddr::V6(addr) => {
                connect_request.raw[2] = 6;
                for (dest, src) in connect_request.raw[3..].iter_mut().zip(addr.octets()) {
                    *dest = src;
                }
            }
        }

        let buf = unsafe {
            xous::MemoryRange::new(
                &mut connect_request as *mut ConnectRequest as usize,
                core::mem::size_of::<ConnectRequest>(),
            )
            .unwrap()
        };

        let response = xous::send_message(
            services::network(),
            xous::Message::new_lend_mut(
                44, /* StdTcpListen */
                buf,
                None,
                xous::MemorySize::new(4096),
            ),
        );

        if let Ok(xous::Result::MemoryReturned(_, valid)) = response {
            // The first four bytes should be zero upon success, and will be nonzero
            // for an error.
            let response = buf.as_slice::<u8>();
            if response[0] != 0 || valid.is_none() {
                let errcode = response[1];
                if errcode == NetError::SocketInUse as u8 {
                    return Err(io::const_io_error!(io::ErrorKind::ResourceBusy, &"Socket in use"));
                } else if errcode == NetError::Invalid as u8 {
                    return Err(io::const_io_error!(
                        io::ErrorKind::AddrNotAvailable,
                        &"Port can't be 0 or invalid address"
                    ));
                } else if errcode == NetError::LibraryError as u8 {
                    return Err(io::const_io_error!(io::ErrorKind::Other, &"Library error"));
                } else {
                    return Err(io::const_io_error!(
                        io::ErrorKind::Other,
                        &"Unable to connect or internal error"
                    ));
                }
            }
            let fd = response[1] as usize;
            // println!("TcpListening with file handle of {}\r\n", fd);
            return Ok(fd);
        }
        Err(io::const_io_error!(io::ErrorKind::InvalidInput, &"Invalid response"))
    }

    pub fn socket_addr(&self) -> io::Result<SocketAddr> {
        Ok(self.local)
    }

    pub fn accept(&self) -> io::Result<(TcpStream, SocketAddr)> {
        let mut receive_request = ReceiveData { raw: [0u8; 4096] };

        let range = unsafe {
            xous::MemoryRange::new(&mut receive_request as *mut ReceiveData as usize, 4096).unwrap()
        };
        if self.nonblocking.load(Ordering::Relaxed) {
            // nonblocking
            receive_request.raw[0] = 0;
        } else {
            // blocking
            receive_request.raw[0] = 1;
        }

        if let Ok(xous::Result::MemoryReturned(_offset, _valid)) = xous::send_message(
            services::network(),
            xous::Message::new_lend_mut(
                45 | (self.fd.load(Ordering::Relaxed) << 16), /* StdTcpAccept */
                range,
                None,
                None,
            ),
        ) {
            if receive_request.raw[0] != 0 {
                // error case
                if receive_request.raw[1] == NetError::TimedOut as u8 {
                    return Err(io::const_io_error!(io::ErrorKind::TimedOut, &"accept timed out",));
                } else if receive_request.raw[1] == NetError::WouldBlock as u8 {
                    return Err(io::const_io_error!(
                        io::ErrorKind::WouldBlock,
                        &"accept would block",
                    ));
                } else if receive_request.raw[1] == NetError::LibraryError as u8 {
                    return Err(io::const_io_error!(io::ErrorKind::Other, &"Library error"));
                } else {
                    return Err(io::const_io_error!(io::ErrorKind::Other, &"library error",));
                }
            } else {
                // accept successful
                let rr = &receive_request.raw;
                let stream_fd = u16::from_le_bytes(rr[1..3].try_into().unwrap());
                let port = u16::from_le_bytes(rr[20..22].try_into().unwrap());
                let addr = if rr[3] == 4 {
                    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(rr[4], rr[5], rr[6], rr[7])), port)
                } else if rr[3] == 6 {
                    SocketAddr::new(
                        IpAddr::V6(Ipv6Addr::new(
                            u16::from_be_bytes(rr[4..6].try_into().unwrap()),
                            u16::from_be_bytes(rr[6..8].try_into().unwrap()),
                            u16::from_be_bytes(rr[8..10].try_into().unwrap()),
                            u16::from_be_bytes(rr[10..12].try_into().unwrap()),
                            u16::from_be_bytes(rr[12..14].try_into().unwrap()),
                            u16::from_be_bytes(rr[14..16].try_into().unwrap()),
                            u16::from_be_bytes(rr[16..18].try_into().unwrap()),
                            u16::from_be_bytes(rr[18..20].try_into().unwrap()),
                        )),
                        port,
                    )
                } else {
                    return Err(io::const_io_error!(io::ErrorKind::Other, &"library error",));
                };

                // replenish the listener
                let new_fd = TcpListener::bind_inner(&self.local)?;
                self.fd.store(new_fd, Ordering::Relaxed);

                // now return a stream converted from the old stream's fd
                Ok((
                    TcpStream::from_listener(
                        stream_fd as usize,
                        self.local.port(),
                        port,
                        addr,
                    ),
                    addr
                ))
            }
        } else {
            Err(io::const_io_error!(io::ErrorKind::InvalidInput, &"Unable to accept"))
        }
    }

    pub fn duplicate(&self) -> io::Result<TcpListener> {
        self.handle_count.fetch_add(1, Ordering::Relaxed);
        Ok(self.clone())
    }

    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        xous::send_message(
            services::network(),
            xous::Message::new_blocking_scalar(
                37 | ((self.fd.load(Ordering::Relaxed) as usize) << 16), //StdSetTtl = 37
                ttl as usize,
                0,
                0,
                0,
            ),
        )
        .or(Err(io::const_io_error!(io::ErrorKind::InvalidInput, &"Unexpected return value")))
        .map(|_| ())
    }

    pub fn ttl(&self) -> io::Result<u32> {
        xous::send_message(
            services::network(),
            xous::Message::new_blocking_scalar(
                36 | ((self.fd.load(Ordering::Relaxed) as usize) << 16), //StdGetTtl = 36
                0,
                0,
                0,
                0,
            ),
        )
        .or(Err(io::const_io_error!(io::ErrorKind::InvalidInput, &"Unexpected return value")))
        .and_then(|res| {
            if let xous::Result::Scalar1(ttl) = res {
                Ok(ttl as u32)
            } else {
                Err(io::const_io_error!(io::ErrorKind::InvalidInput, &"Unexpected return value"))
            }
        })
    }

    pub fn set_only_v6(&self, _: bool) -> io::Result<()> {
        unimpl!();
    }

    pub fn only_v6(&self) -> io::Result<bool> {
        unimpl!();
    }

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        // this call doesn't have a meaning on our platform, but we can at least not panic if it's used.
        Ok(None)
    }

    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.nonblocking.store(nonblocking, Ordering::Relaxed);
        Ok(())
    }
}

impl fmt::Debug for TcpListener {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TCP listening on {:?}", self.local)
    }
}

impl Drop for TcpListener {
    fn drop(&mut self) {
        if self.handle_count.fetch_sub(1, Ordering::Relaxed) == 1 {
            // only drop if we're the last clone
            match xous::send_message(
                services::network(),
                xous::Message::new_blocking_scalar(
                    34 | ((self.fd.load(Ordering::Relaxed) as usize) << 16), // StdTcpClose - re-using an implementation
                    0,
                    0,
                    0,
                    0,
                ),
            ) {
                Ok(xous::Result::Scalar1(result)) => {
                    if result != 0 {
                        println!("TcpListener drop failure err code {}\r\n", result);
                    }
                }
                _ => {
                    println!("TcpListener drop failure - internal error\r\n");
                }
            }
        }
    }
}

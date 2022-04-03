use super::super::services;
use crate::sys::unsupported;
use crate::io;
use crate::time::Duration;
use crate::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use crate::fmt;
use crate::cell::Cell;
use crate::sync::Arc;
use core::sync::atomic::{AtomicUsize, Ordering};
use core::convert::TryInto;
use super::*;

macro_rules! unimpl {
    () => {
        return Err(io::Error::new_const(
            io::ErrorKind::Unsupported,
            &"This function is not yet implemented",
        ));
    };
}

pub struct UdpSocket {
    fd: usize,
    local: SocketAddr,
    remote: Option<SocketAddr>,
    // in milliseconds. The setting applies only to `recv` calls after the timeout is set.
    read_timeout: Cell<u64>,
    // in milliseconds. The setting applies only to `send` calls after the timeout is set.
    write_timeout: Cell<u64>,
    handle_count: Arc<AtomicUsize>,
    nonblocking: Cell<bool>,
}

impl UdpSocket {
    pub fn bind(socketaddr: io::Result<&SocketAddr>) -> io::Result<UdpSocket> {
        let addr = socketaddr?;
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
                40, /* StdUdpBind */
                buf,
                None,
                xous::MemorySize::new(4096),
            ),
        );

        if let Ok(xous::Result::MemoryReturned(_, valid)) = response {
            // The first four bytes should be zero upon success, and will be nonzero
            // for an error.
            let response = buf.as_slice::<u16>();
            if response[0] != 0 || valid.is_none() {
                // errcode is a u8 but stuck in a u16 where the upper byte is invalid. Mask & decode accordingly.
                let errcode = (response[4] & 0xff) as u8;
                if errcode == NetError::SocketInUse as u8 {
                    return Err(io::Error::new_const(io::ErrorKind::ResourceBusy, &"Socket in use"));
                } else if errcode == NetError::Invalid as u8 {
                    return Err(io::Error::new_const(io::ErrorKind::InvalidInput, &"Port can't be 0 or invalid address"));
                } else {
                    return Err(io::Error::new_const(io::ErrorKind::Other, &"Unable to connect or internal error"));
                }
            }
            let fd = response[1] as usize;
            println!(
                 "Connected with file handle of {}",
                 fd
            );
            return Ok(UdpSocket {
                fd,
                local: *addr,
                remote: None,
                read_timeout: Cell::new(0),
                write_timeout: Cell::new(0),
                handle_count: Arc::new(AtomicUsize::new(1)),
                nonblocking: Cell::new(false),
            });
        }
        Err(io::Error::new_const(io::ErrorKind::InvalidInput, &"Invalid response"))
    }

    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        match self.remote {
            Some(dest) => {
                Ok(dest)
            }
            None => {
                Err(io::Error::new_const(io::ErrorKind::NotConnected, &"No peer specified"))
            }
        }
    }

    pub fn socket_addr(&self) -> io::Result<SocketAddr> {
        Ok(self.local)
    }

    pub fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        let mut receive_request = ReceiveData { raw: [0u8; 4096] };

        let range = unsafe {
            xous::MemoryRange::new(&mut receive_request as *mut ReceiveData as usize, 4096).unwrap()
        };
        if self.nonblocking.get() {
            // nonblocking
            receive_request.raw[0] = 0;
        } else {
            // blocking
            receive_request.raw[0] = 1;
            for (&s, d) in self.read_timeout.get().to_le_bytes().iter().zip(receive_request.raw[1..9].iter_mut()) {
                *d = s;
            }
        }
        if let Ok(xous::Result::MemoryReturned(_offset, _valid)) = xous::send_message(
            services::network(),
            xous::Message::new_lend_mut(
                42 | (self.fd << 16), /* StdUdpRx */
                range,
                None,
                None,
            ),
        ) {
            if receive_request.raw[0] != 0 {
                if receive_request.raw[1] == NetError::TimedOut as u8 {
                    return Err(io::Error::new_const(
                        io::ErrorKind::TimedOut,
                        &"recv timed out",
                    ));
                } else if receive_request.raw[1] == NetError::WouldBlock as u8 {
                    return Err(io::Error::new_const(
                        io::ErrorKind::WouldBlock,
                        &"recv would block",
                    ));
                } else {
                    return Err(io::Error::new_const(
                        io::ErrorKind::Other,
                        &"library error",
                    ));
                }
            } else {
                let rr = &receive_request.raw;
                let rxlen = u16::from_le_bytes(rr[1..3].try_into().unwrap());
                let port = u16::from_le_bytes(rr[20..22].try_into().unwrap());
                let addr =
                    if rr[3] == 4 {
                        SocketAddr::new(
                            IpAddr::V4(Ipv4Addr::new(
                                rr[4], rr[5], rr[6], rr[7],
                            )),
                            port,
                        )
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
                        return Err(io::Error::new_const(
                            io::ErrorKind::Other,
                            &"library error",
                        ));
                    };
                for (&s, d) in rr[22..22 + rxlen as usize].iter().zip(buf.iter_mut()) {
                    *d = s;
                }
                Ok((rxlen as usize, addr))
            }
        } else {
            Err(io::Error::new_const(io::ErrorKind::InvalidInput, &"Unable to recv"))
        }
    }

    pub fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.recv_from(buf).map(|(len, _addr)| len)
    }

    pub fn peek_from(&self, _: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        unimpl!();
    }

    pub fn send_to(&self, _: &[u8], _: &SocketAddr) -> io::Result<usize> {
        unimpl!();
    }

    pub fn duplicate(&self) -> io::Result<UdpSocket> {
        unimpl!();
    }

    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        if let Some(d) = timeout {
            if d.is_zero() {
                return Err(io::Error::new_const(io::ErrorKind::InvalidInput, &"Zero duration is invalid"));
            }
        }
        self.read_timeout
            .set(timeout.map(|t| t.as_millis().min(u64::MAX as u128) as u64).unwrap_or_default());
        Ok(())
    }

    pub fn set_write_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        if let Some(d) = timeout {
            if d.is_zero() {
                return Err(io::Error::new_const(io::ErrorKind::InvalidInput, &"Zero duration is invalid"));
            }
        }
        self.write_timeout
            .set(timeout.map(|t| t.as_millis().min(u64::MAX as u128) as u64).unwrap_or_default());
        Ok(())
    }

    pub fn read_timeout(&self) -> io::Result<Option<Duration>> {
        match self.read_timeout.get() {
            0 => Ok(None),
            t => Ok(Some(Duration::from_millis(t as u64))),
        }
    }

    pub fn write_timeout(&self) -> io::Result<Option<Duration>> {
        match self.write_timeout.get() {
            0 => Ok(None),
            t => Ok(Some(Duration::from_millis(t as u64))),
        }
    }

    pub fn set_broadcast(&self, _: bool) -> io::Result<()> {
        unimpl!();
    }

    pub fn broadcast(&self) -> io::Result<bool> {
        unimpl!();
    }

    pub fn set_multicast_loop_v4(&self, _: bool) -> io::Result<()> {
        unimpl!();
    }

    pub fn multicast_loop_v4(&self) -> io::Result<bool> {
        unimpl!();
    }

    pub fn set_multicast_ttl_v4(&self, _: u32) -> io::Result<()> {
        unimpl!();
    }

    pub fn multicast_ttl_v4(&self) -> io::Result<u32> {
        unimpl!();
    }

    pub fn set_multicast_loop_v6(&self, _: bool) -> io::Result<()> {
        unimpl!();
    }

    pub fn multicast_loop_v6(&self) -> io::Result<bool> {
        unimpl!();
    }

    pub fn join_multicast_v4(&self, _: &Ipv4Addr, _: &Ipv4Addr) -> io::Result<()> {
        unimpl!();
    }

    pub fn join_multicast_v6(&self, _: &Ipv6Addr, _: u32) -> io::Result<()> {
        unimpl!();
    }

    pub fn leave_multicast_v4(&self, _: &Ipv4Addr, _: &Ipv4Addr) -> io::Result<()> {
        unimpl!();
    }

    pub fn leave_multicast_v6(&self, _: &Ipv6Addr, _: u32) -> io::Result<()> {
        unimpl!();
    }

    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        xous::send_message(
            self.fd as _,
            xous::Message::new_blocking_scalar(
                37 | ((self.fd as usize) << 16), //StdSetTtl = 37
                ttl as usize,
                0,
                0,
                0,
            ),
        )
        .or(Err(io::Error::new_const(io::ErrorKind::InvalidInput, &"Unexpected return value")))
        .map(|_| ())
    }

    pub fn ttl(&self) -> io::Result<u32> {
        unimpl!();
    }

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        unimpl!();
    }

    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.nonblocking.set(nonblocking);
        Ok(())
    }

    pub fn peek(&self, _: &mut [u8]) -> io::Result<usize> {
        unimpl!();
    }

    pub fn send(&self, _: &[u8]) -> io::Result<usize> {
        unimpl!();
    }

    pub fn connect(&self, _: io::Result<&SocketAddr>) -> io::Result<()> {
        unimpl!();
    }
}

impl fmt::Debug for UdpSocket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "UDP listening on {:?} to {:?}",
            self.local, self.remote,
        )
    }
}
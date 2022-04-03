use super::super::services;
use crate::sys::unsupported;
use crate::io;
use crate::time::Duration;
use crate::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use crate::fmt;
use crate::cell::Cell;
use crate::sync::Arc;
use core::sync::atomic::{AtomicUsize, Ordering};
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
    local: SocketAddr,
    remote: Option<SocketAddr>,
    // in milliseconds. The setting applies only to `recv` calls after the timeout is set.
    read_timeout: Cell<u32>,
    // in milliseconds. The setting applies only to `send` calls after the timeout is set.
    write_timeout: Cell<u32>,
    handle_count: Arc<AtomicUsize>,
    nonblocking: bool,
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
                local: *addr,
                remote: None,
                read_timeout: Cell::new(0),
                write_timeout: Cell::new(0),
                handle_count: Arc::new(AtomicUsize::new(1)),
                nonblocking: false
            });
        }
        Err(io::Error::new_const(io::ErrorKind::InvalidInput, &"Invalid response"))
    }

    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        unimpl!();
    }

    pub fn socket_addr(&self) -> io::Result<SocketAddr> {
        unimpl!();
    }

    pub fn recv_from(&self, _: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        unimpl!();
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

    pub fn set_read_timeout(&self, _: Option<Duration>) -> io::Result<()> {
        unimpl!();
    }

    pub fn set_write_timeout(&self, _: Option<Duration>) -> io::Result<()> {
        unimpl!();
    }

    pub fn read_timeout(&self) -> io::Result<Option<Duration>> {
        unimpl!();
    }

    pub fn write_timeout(&self) -> io::Result<Option<Duration>> {
        unimpl!();
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

    pub fn set_ttl(&self, _: u32) -> io::Result<()> {
        unimpl!();
    }

    pub fn ttl(&self) -> io::Result<u32> {
        unimpl!();
    }

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        unimpl!();
    }

    pub fn set_nonblocking(&self, _: bool) -> io::Result<()> {
        unimpl!();
    }

    pub fn recv(&self, _: &mut [u8]) -> io::Result<usize> {
        unimpl!();
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
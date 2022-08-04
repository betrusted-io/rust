use super::super::services;
use super::*;
use crate::fmt;
use crate::io::{self, IoSlice, IoSliceMut};
use crate::net::{IpAddr, Ipv4Addr, Shutdown, SocketAddr, SocketAddrV4, SocketAddrV6};
use crate::sync::Arc;
use crate::time::Duration;
use core::sync::atomic::{AtomicU32, AtomicUsize, AtomicBool, Ordering};

macro_rules! unimpl {
    () => {
        return Err(io::const_io_error!(
            io::ErrorKind::Unsupported,
            &"This function is not yet implemented",
        ));
    };
}

#[derive(Clone)]
pub struct TcpStream {
    fd: usize,
    local_port: u16,
    remote_port: u16,
    peer_addr: SocketAddr,
    // milliseconds
    read_timeout: Arc<AtomicU32>,
    // milliseconds
    write_timeout: Arc<AtomicU32>,
    handle_count: Arc<AtomicUsize>,
    nonblocking: Arc<AtomicBool>,
}

fn sockaddr_to_buf(duration: Duration, addr: &SocketAddr, buf: &mut [u8]) {
    // Construct the request.
    let port_bytes = addr.port().to_le_bytes();
    buf[0] = port_bytes[0];
    buf[1] = port_bytes[1];
    for (dest, src) in buf[2..].iter_mut().zip((duration.as_millis() as u64).to_le_bytes()) {
        *dest = src;
    }
    match addr.ip() {
        IpAddr::V4(addr) => {
            buf[10] = 4;
            for (dest, src) in buf[11..].iter_mut().zip(addr.octets()) {
                *dest = src;
            }
        }
        IpAddr::V6(addr) => {
            buf[10] = 6;
            for (dest, src) in buf[11..].iter_mut().zip(addr.octets()) {
                *dest = src;
            }
        }
    }
}

impl TcpStream {
    pub (crate) fn from_listener(
        fd: usize,
        local_port: u16,
        remote_port: u16,
        peer_addr: SocketAddr
    ) -> TcpStream {
        TcpStream {
            fd,
            local_port,
            remote_port,
            peer_addr,
            read_timeout: Arc::new(AtomicU32::new(0)),
            write_timeout: Arc::new(AtomicU32::new(0)),
            handle_count: Arc::new(AtomicUsize::new(1)),
            nonblocking: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn connect(socketaddr: io::Result<&SocketAddr>) -> io::Result<TcpStream> {
        Self::connect_timeout(socketaddr?, Duration::ZERO)
    }

    pub fn connect_timeout(addr: &SocketAddr, duration: Duration) -> io::Result<TcpStream> {
        let mut connect_request = ConnectRequest { raw: [0u8; 4096] };

        // Construct the request.
        sockaddr_to_buf(duration, &addr, &mut connect_request.raw);

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
                30, /* StdTcpConnect */
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
                    return Err(io::const_io_error!(
                        io::ErrorKind::ResourceBusy,
                        &"Socket in use",
                    ));
                } else if errcode == NetError::Unaddressable as u8 {
                    return Err(io::const_io_error!(
                        io::ErrorKind::AddrNotAvailable,
                        &"Invalid address",
                    ));
                } else {
                    return Err(io::const_io_error!(
                        io::ErrorKind::InvalidInput,
                        &"Unable to connect or internal error",
                    ));
                }
            }
            let fd = response[1] as usize;
            let local_port = response[2];
            let remote_port = response[3];
            // println!(
            //     "Connected with local port of {}, remote port of {}, file handle of {}",
            //     local_port, remote_port, fd
            // );
            return Ok(TcpStream {
                fd,
                local_port,
                remote_port,
                peer_addr: *addr,
                read_timeout: Arc::new(AtomicU32::new(0)),
                write_timeout: Arc::new(AtomicU32::new(0)),
                handle_count: Arc::new(AtomicUsize::new(1)),
                nonblocking: Arc::new(AtomicBool::new(false)),
            });
        }
        Err(io::const_io_error!(io::ErrorKind::InvalidInput, &"Invalid response"))
    }

    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        if let Some(to) = timeout {
            if to.is_zero() {
                return Err(io::const_io_error!(
                    io::ErrorKind::InvalidInput,
                    &"Zero is an invalid timeout",
                ));
            }
        }
        self.read_timeout.store(
            timeout.map(|t| t.as_millis().min(u32::MAX as u128) as u32).unwrap_or_default(),
            Ordering::Relaxed,
        );
        Ok(())
    }

    pub fn set_write_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        if let Some(to) = timeout {
            if to.is_zero() {
                return Err(io::const_io_error!(
                    io::ErrorKind::InvalidInput,
                    &"Zero is an invalid timeout",
                ));
            }
        }
        self.write_timeout.store(
            timeout.map(|t| t.as_millis().min(u32::MAX as u128) as u32).unwrap_or_default(),
            Ordering::Relaxed,
        );
        Ok(())
    }

    pub fn read_timeout(&self) -> io::Result<Option<Duration>> {
        match self.read_timeout.load(Ordering::Relaxed) {
            0 => Ok(None),
            t => Ok(Some(Duration::from_millis(t as u64))),
        }
    }

    pub fn write_timeout(&self) -> io::Result<Option<Duration>> {
        match self.write_timeout.load(Ordering::Relaxed) {
            0 => Ok(None),
            t => Ok(Some(Duration::from_millis(t as u64))),
        }
    }

    pub fn peek(&self, buf: &mut [u8]) -> io::Result<usize> {
        let mut receive_request = ReceiveData { raw: [0u8; 4096] };
        let data_to_read = buf.len().min(receive_request.raw.len());

        let range = unsafe {
            xous::MemoryRange::new(&mut receive_request as *mut ReceiveData as usize, 4096).unwrap()
        };

        if let Ok(xous::Result::MemoryReturned(offset, valid)) = xous::send_message(
            services::network(),
            xous::Message::new_lend_mut(
                32 | (self.fd << 16)
                | if self.nonblocking.load(Ordering::SeqCst) { 0x8000 } else { 0 }, /* StdTcpPeek */
                range,
                None,
                xous::MemorySize::new(data_to_read),
            ),
        ) {
            // println!("offset: {:?}, valid: {:?}", offset, valid);
            if offset.is_some() {
                let length = valid.map_or(0, |v| v.get());
                for (dest, src) in buf.iter_mut().zip(receive_request.raw[..length].iter()) {
                    *dest = *src;
                }
                Ok(length)
            } else {
                let result = range.as_slice::<u32>();
                if result[0] != 0 {
                    if result[1] == 8 { // timed out
                        return Err(io::const_io_error!(
                            io::ErrorKind::TimedOut,
                            &"Timeout",
                        ));
                    }
                    if result[1] == 9 { // would block
                        return Err(io::const_io_error!(
                            io::ErrorKind::WouldBlock,
                            &"Would block",
                        ));
                    }
                }
                Err(io::const_io_error!(io::ErrorKind::Other, &"recv_slice peek failure"))
            }
        } else {
            Err(io::const_io_error!(io::ErrorKind::InvalidInput, &"Library failure: wrong message type or messaging error"))
        }
    }

    pub fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        let mut receive_request = ReceiveData { raw: [0u8; 4096] };
        let data_to_read = buf.len().min(receive_request.raw.len());

        let range = unsafe {
            xous::MemoryRange::new(&mut receive_request as *mut ReceiveData as usize, 4096).unwrap()
        };

        if let Ok(xous::Result::MemoryReturned(offset, valid)) = xous::send_message(
            services::network(),
            xous::Message::new_lend_mut(
                33 | (self.fd << 16)
                | if self.nonblocking.load(Ordering::SeqCst) { 0x8000 } else { 0 }, /* StdTcpRx */
                range,
                // Reuse the `offset` as the read timeout
                xous::MemoryAddress::new(self.read_timeout.load(Ordering::Relaxed) as usize),
                xous::MemorySize::new(data_to_read),
            ),
        ) {
            // println!("offset: {:?}, valid: {:?}", offset, valid);
            if offset.is_some() {
                let length = valid.map_or(0, |v| v.get());
                for (dest, src) in buf.iter_mut().zip(receive_request.raw[..length].iter()) {
                    *dest = *src;
                }
                Ok(length)
            } else {
                let result = range.as_slice::<u32>();
                if result[0] != 0 {
                    if result[1] == 8 { // timed out
                        return Err(io::const_io_error!(
                            io::ErrorKind::TimedOut,
                            &"Timeout",
                        ));
                    }
                    if result[1] == 9 { // would block
                        return Err(io::const_io_error!(
                            io::ErrorKind::WouldBlock,
                            &"Would block",
                        ));
                    }
                }
                Err(io::const_io_error!(io::ErrorKind::Other, &"recv_slice failure"))
            }
        } else {
            Err(io::const_io_error!(io::ErrorKind::InvalidInput, &"Library failure: wrong message type or messaging error"))
        }
    }

    pub fn read_vectored(&self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        crate::io::default_read_vectored(|b| self.read(b), bufs)
    }

    pub fn is_read_vectored(&self) -> bool {
        false
    }

    pub fn write(&self, buf: &[u8]) -> io::Result<usize> {
        let mut send_request = SendData { raw: [0u8; 4096] };
        for (dest, src) in send_request.raw.iter_mut().zip(buf) {
            *dest = *src;
        }

        let range = unsafe {
            xous::MemoryRange::new(
                &mut send_request as *mut SendData as usize,
                core::mem::size_of::<SendData>(),
            )
            .unwrap()
        };

        let response = xous::send_message(
            services::network(),
            xous::Message::new_lend_mut(
                31 | (self.fd << 16), /* StdTcpTx */
                range,
                // Reuse the offset as the timeout
                xous::MemoryAddress::new(self.write_timeout.load(Ordering::Relaxed) as usize),
                xous::MemorySize::new(buf.len().min(send_request.raw.len())),
            ),
        )
        .or(Err(io::const_io_error!(io::ErrorKind::InvalidInput, &"Internal error")))?;

        if let xous::Result::MemoryReturned(_offset, _valid) = response {
            let result = range.as_slice::<u32>();
            if result[0] != 0 {
                if result[1] == 8 { // timed out
                    return Err(io::const_io_error!(
                        io::ErrorKind::BrokenPipe,
                        &"Timeout or connection closed",
                    ));
                } else if result[1] == 9 { // would block
                    return Err(io::const_io_error!(
                        io::ErrorKind::WouldBlock,
                        &"Would block",
                    ));
                } else {
                    return Err(io::const_io_error!(
                        io::ErrorKind::InvalidInput,
                        &"Error when sending",
                    ));
                }
            }
            Ok(result[1] as usize)
        } else {
            Err(io::const_io_error!(io::ErrorKind::InvalidInput, &"Unexpected return value"))
        }
    }

    pub fn write_vectored(&self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        crate::io::default_write_vectored(|b| self.write(b), bufs)
    }

    pub fn is_write_vectored(&self) -> bool {
        false
    }

    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        Ok(self.peer_addr)
    }

    pub fn socket_addr(&self) -> io::Result<SocketAddr> {
        let mut get_addr = GetAddress { raw: [0u8; 4096] };
        let range = unsafe {
            xous::MemoryRange::new(
                &mut get_addr as *mut GetAddress as usize,
                core::mem::size_of::<GetAddress>(),
            )
            .unwrap()
        };

        match xous::send_message(
            services::network(),
            xous::Message::new_lend_mut(
                35 | (self.fd << 16), /* StdGetAddress */
                range,
                None,
                None,
            ),
        ) {
            Ok(xous::Result::MemoryReturned(_offset, _valid)) => {
                let mut i = get_addr.raw.iter();
                match *i.next().unwrap() {
                    4 => Ok(SocketAddr::V4(SocketAddrV4::new(
                        Ipv4Addr::new(
                            *i.next().unwrap(),
                            *i.next().unwrap(),
                            *i.next().unwrap(),
                            *i.next().unwrap(),
                        ),
                        self.local_port,
                    ))),
                    6 => {
                        let mut new_addr = [0u8; 16];
                        for (src, octet) in i.zip(new_addr.iter_mut()) {
                            *octet = *src;
                        }
                        Ok(SocketAddr::V6(SocketAddrV6::new(
                            new_addr.into(),
                            self.local_port,
                            0,
                            0,
                        )))
                    }
                    _ => Err(io::const_io_error!(io::ErrorKind::InvalidInput, &"Internal error")),
                }
            }
            _ => Err(io::const_io_error!(io::ErrorKind::InvalidInput, &"Internal error")),
        }
    }

    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        let shutdown_code = match how {
            crate::net::Shutdown::Read => 1,
            crate::net::Shutdown::Write => 2,
            crate::net::Shutdown::Both => 3,
        };

        xous::send_message(
            services::network(),
            xous::Message::new_blocking_scalar(
                46 | ((self.fd as usize) << 16), // StdTcpStreamShutdown
                shutdown_code,
                0,
                0,
                0,
            ),
        )
        .or(Err(io::const_io_error!(io::ErrorKind::InvalidInput, &"Unexpected return value")))
        .map(|_| ())
    }

    pub fn duplicate(&self) -> io::Result<TcpStream> {
        self.handle_count.fetch_add(1, Ordering::Relaxed);
        Ok(self.clone())
    }

    pub fn set_linger(&self, _: Option<Duration>) -> io::Result<()> {
        unimpl!();
    }

    pub fn linger(&self) -> io::Result<Option<Duration>> {
        unimpl!();
    }

    pub fn set_nodelay(&self, enabled: bool) -> io::Result<()> {
        xous::send_message(
            services::network(),
            xous::Message::new_blocking_scalar(
                39 | ((self.fd as usize) << 16), //StdSetNodelay = 39
                if enabled { 1 } else { 0 },
                0,
                0,
                0,
            ),
        )
        .or(Err(io::const_io_error!(io::ErrorKind::InvalidInput, &"Unexpected return value")))
        .map(|_| ())
    }

    pub fn nodelay(&self) -> io::Result<bool> {
        let result = xous::send_message(
            services::network(),
            xous::Message::new_blocking_scalar(
                38 | ((self.fd as usize) << 16), //StdGetNodelay = 38
                0,
                0,
                0,
                0,
            ),
        )
        .or(Err(io::const_io_error!(io::ErrorKind::InvalidInput, &"Unexpected return value")))?;
        if let xous::Result::Scalar1(enabled) = result {
            Ok(enabled != 0)
        } else {
            Err(io::const_io_error!(io::ErrorKind::InvalidInput, &"Unexpected return value"))
        }
    }

    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        xous::send_message(
            services::network(),
            xous::Message::new_blocking_scalar(
                37 | ((self.fd as usize) << 16), //StdSetTtl = 37
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
                36 | ((self.fd as usize) << 16), //StdGetTtl = 36
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

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        // this call doesn't have a meaning on our platform, but we can at least not panic if it's used.
        Ok(None)
    }

    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.nonblocking.store(nonblocking, Ordering::SeqCst);
        Ok(())
    }
}

impl fmt::Debug for TcpStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TCP connection to {:?} port {} to local port {}",
            self.peer_addr, self.remote_port, self.local_port
        )
    }
}

impl Drop for TcpStream {
    fn drop(&mut self) {
        if self.handle_count.fetch_sub(1, Ordering::Relaxed) == 1 {
            // only drop if we're the last clone
            match xous::send_message(
                services::network(),
                xous::Message::new_blocking_scalar(
                    34 | ((self.fd as usize) << 16), // StdTcpClose
                    0,
                    0,
                    0,
                    0,
                ),
            ) {
                Ok(xous::Result::Scalar1(result)) => {
                    if result != 0 {
                        println!("TcpStream drop failure err code {}\r\n", result);
                    }
                }
                _ => {
                    println!("TcpStream drop failure - internal error\r\n");
                }
            }
        }
    }
}
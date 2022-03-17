use crate::io;
use crate::net::{Ipv4Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use core::convert::{TryFrom, TryInto};

use super::super::services;

pub struct Dns {
    cid: xous::CID,
}

#[derive(Debug)]
pub struct DnsError {
    pub code: u8,
}

#[derive(Debug)]
#[repr(C, align(4096))]
pub struct LookupHost {
    data: [u8; 4096],
    port: u16,
    offset: usize,
    count: usize,
}

impl LookupHost {
    pub fn port(&self) -> u16 {
        self.port
    }
}

impl Iterator for LookupHost {
    type Item = SocketAddr;
    fn next(&mut self) -> Option<SocketAddr> {
        if self.offset >= self.data.len() {
            return None;
        }
        match self.data.get(self.offset) {
            Some(&4) => {
                self.offset += 1;
                if self.offset + 4 > self.data.len() {
                    return None;
                }
                let result = Some(SocketAddr::V4(SocketAddrV4::new(
                    Ipv4Addr::new(
                        self.data[self.offset],
                        self.data[self.offset + 1],
                        self.data[self.offset + 2],
                        self.data[self.offset + 3],
                    ),
                    self.port,
                )));
                self.offset += 4;
                result
            }
            Some(&6) => {
                self.offset += 1;
                if self.offset + 16 > self.data.len() {
                    return None;
                }
                let mut new_addr = [0u8; 16];
                for (src, octet) in self.data[(self.offset + 1)..(self.offset + 16 + 1)]
                    .iter()
                    .zip(new_addr.iter_mut())
                {
                    *octet = *src;
                }
                let result =
                    Some(SocketAddr::V6(SocketAddrV6::new(new_addr.into(), self.port, 0, 0)));
                self.offset += 16;
                result
            }
            _ => None,
        }
    }
}

impl Dns {
    pub fn new() -> Dns {
        Dns { cid: services::dns() }
    }

    pub fn lookup(&self, query: &str, port: u16) -> Result<LookupHost, DnsError> {
        let mut result = LookupHost { data: [0u8; 4096], offset: 0, count: 0, port };

        // Copy the query into the message that gets sent to the DNS server
        for (query_byte, result_byte) in query.as_bytes().iter().zip(result.data.iter_mut()) {
            *result_byte = *query_byte;
        }

        let buf = unsafe {
            xous::MemoryRange::new(&mut result as *mut LookupHost as usize, 4096).unwrap()
        };
        let response = xous::send_message(
            self.cid,
            xous::Message::new_lend_mut(
                6, /* RawLookup */
                buf,
                None,
                xous::MemorySize::new(query.as_bytes().len()),
            ),
        );
        if let Ok(xous::Result::MemoryReturned(_, _)) = response {
            // The first element in the Status message is the result code.
            let data = buf.as_slice::<u8>();

            if data[0] != 0 {
                Err(DnsError { code: data[1] })
            } else {
                assert_eq!(result.offset, 0);
                result.count = data[1] as usize;

                // Advance the offset to the first record
                result.offset = 2;
                Ok(result)
            }
        } else {
            Err(DnsError { code: 0 })
        }
    }
}

impl TryFrom<&str> for LookupHost {
    type Error = io::Error;

    fn try_from(s: &str) -> io::Result<LookupHost> {
        macro_rules! try_opt {
            ($e:expr, $msg:expr) => {
                match $e {
                    Some(r) => r,
                    None => return Err(io::const_io_error!(io::ErrorKind::InvalidInput, &$msg)),
                    // None => return Err(io::Error::new(io::ErrorKind::AddrInUse, "error")),
                }
            };
        }

        // split the string by ':' and convert the second part to u16
        let (host, port_str) = try_opt!(s.rsplit_once(':'), "invalid socket address");
        let port: u16 = try_opt!(port_str.parse().ok(), "invalid port value");
        (host, port).try_into()
    }
}

impl TryFrom<(&str, u16)> for LookupHost {
    type Error = io::Error;

    fn try_from(v: (&str, u16)) -> io::Result<LookupHost> {
        // println!("Trying to look up {}:{}", v.0, v.1);
        Dns::new()
            .lookup(v.0, v.1)
            .map_err(|_e| io::const_io_error!(io::ErrorKind::InvalidInput, &"DNS failure"))
    }
}

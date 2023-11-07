use crate::io::SeekFrom;
use crate::os::xous::ffi::Connection;
use crate::os::xous::services::connect;
use core::sync::atomic::{AtomicU32, Ordering};

#[repr(usize)]
pub(crate) enum PddbBlockingScalar {
    SeekKeyStd(u16 /* fd */, SeekFrom),
    CloseKeyStd(u16 /* fd */),
}

#[repr(usize)]
pub(crate) enum PddbLendMut {
    // // IsMounted = 0,
    // // TryMount = 1,

    // // WriteKeyFlush = 18,
    // // KeyDrop = 20,

    // // ListBasisStd = 26,
    // // ListDictStd = 28,
    // // ListKeyStd = 29,
    OpenKeyStd,
    ReadKeyStd(u16 /* fd */),
    // CloseKeyStd = 34,
    DeleteKeyStd = 35,
    // // LatestBasisStd = 36,
    ListPathStd = 37,
    StatPathStd = 38,

    /// Create a dict
    CreateDictStd,

    /// Remove an empty dict
    DeleteDictStd,
}

#[repr(usize)]
pub(crate) enum PddbLend {
    WriteKeyStd(u16 /* fd */),
}

impl Into<usize> for PddbLendMut {
    fn into(self) -> usize {
        match self {
            PddbLendMut::OpenKeyStd => 30,
            PddbLendMut::ReadKeyStd(fd) => 31 | ((fd as usize) << 16),
            PddbLendMut::DeleteKeyStd => 35,
            PddbLendMut::ListPathStd => 37,
            PddbLendMut::StatPathStd => 38,
            PddbLendMut::CreateDictStd => 40,
            PddbLendMut::DeleteDictStd => 41,
        }
    }
}

impl Into<usize> for PddbLend {
    fn into(self) -> usize {
        match self {
            PddbLend::WriteKeyStd(fd) => 32 | ((fd as usize) << 16),
        }
    }
}

impl<'a> Into<[usize; 5]> for PddbBlockingScalar {
    fn into(self) -> [usize; 5] {
        match self {
            PddbBlockingScalar::SeekKeyStd(fd, from) => {
                let (a1, a2, a3) = match from {
                    SeekFrom::Start(off) => {
                        (0, (off as usize & 0xffff_ffff), ((off >> 32) as usize) & 0xffff_ffff)
                    }
                    SeekFrom::Current(off) => {
                        (1, (off as usize & 0xffff_ffff), ((off >> 32) as usize) & 0xffff_ffff)
                    }
                    SeekFrom::End(off) => {
                        (2, (off as usize & 0xffff_ffff), ((off >> 32) as usize) & 0xffff_ffff)
                    }
                };
                [39 | ((fd as usize) << 16), a1, a2, a3, 0]
            }
            PddbBlockingScalar::CloseKeyStd(fd) => [34 | ((fd as usize) << 16), 0, 0, 0, 0],
        }
    }
}

/// Return a `Connection` to the PDDB database server. This server is used for
/// communicating with the persistent database.
pub(crate) fn pddb_server() -> Connection {
    static PDDB_CONNECTION: AtomicU32 = AtomicU32::new(0);
    let cid = PDDB_CONNECTION.load(Ordering::Relaxed);
    if cid != 0 {
        return cid.into();
    }

    let cid = connect("_Plausibly Deniable Database_").unwrap();
    PDDB_CONNECTION.store(cid.into(), Ordering::Relaxed);
    cid
}

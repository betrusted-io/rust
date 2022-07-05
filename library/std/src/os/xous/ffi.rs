#![stable(feature = "rust1", since = "1.0.0")]

#[stable(feature = "rust1", since = "1.0.0")]
#[derive(Debug, Copy, Clone)]
pub struct Connection(u32);
#[stable(feature = "rust1", since = "1.0.0")]
impl From<u32> for Connection {
    fn from(src: u32) -> Connection {
        Connection(src)
    }
}
#[stable(feature = "rust1", since = "1.0.0")]
impl TryFrom<usize> for Connection {
    type Error = core::num::TryFromIntError;
    fn try_from(src: usize) -> Result<Self, Self::Error> {
        Ok(Connection(src.try_into()?))
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Into<u32> for Connection {
    fn into(self) -> u32 {
        self.0
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
#[derive(Debug, Copy, Clone)]
pub struct MessageId(u32);
#[stable(feature = "rust1", since = "1.0.0")]
impl From<u32> for MessageId {
    fn from(src: u32) -> MessageId {
        MessageId(src)
    }
}
#[stable(feature = "rust1", since = "1.0.0")]
impl TryFrom<usize> for MessageId {
    type Error = core::num::TryFromIntError;
    fn try_from(src: usize) -> Result<Self, Self::Error> {
        Ok(MessageId(src.try_into()?))
    }
}
#[stable(feature = "rust1", since = "1.0.0")]
impl Into<u32> for MessageId {
    fn into(self) -> u32 {
        self.0
    }
}
#[stable(feature = "rust1", since = "1.0.0")]
impl TryInto<usize> for MessageId {
    type Error = core::num::TryFromIntError;
    fn try_into(self) -> Result<usize, Self::Error> {
        Ok(self.0.try_into()?)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
#[derive(Debug)]
pub enum ServerAddressError {
    InvalidLength,
}

#[stable(feature = "rust1", since = "1.0.0")]
pub struct ServerAddress([u32; 4]);
#[stable(feature = "rust1", since = "1.0.0")]
impl TryFrom<&str> for ServerAddress {
    type Error = ServerAddressError;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let b = value.as_bytes();
        if b.len() == 0 || b.len() > 16 {
            return Err(Self::Error::InvalidLength);
        }

        let mut this_temp = [0u8; 16];
        for (dest, src) in this_temp.iter_mut().zip(b.iter()) {
            *dest = *src;
        }

        let mut this = [0u32; 4];
        for (dest, src) in this.iter_mut().zip(this_temp.chunks_exact(4)) {
            *dest = u32::from_le_bytes(src.try_into().unwrap());
        }
        Ok(ServerAddress(this))
    }
}

#[path = "../unix/ffi/os_str.rs"]
mod os_str;

#[stable(feature = "rust1", since = "1.0.0")]
pub use self::os_str::{OsStrExt, OsStringExt};

#[stable(feature = "rust1", since = "1.0.0")]
/// Copies of these invocation types here for when we're running
/// in environments without libxous.
pub enum InvokeType {
    LendMut = 1,
    Lend = 2,
    Move = 3,
    Scalar = 4,
    BlockingScalar = 5,
}

#[stable(feature = "rust1", since = "1.0.0")]
/// Copies of these invocation types here for when we're running
/// in environments without libxous.
pub enum Syscall {
    ReceiveMessage = 15,
    SendMessage = 16,
    Connect = 17,
    ReturnMemory = 20,
    ReturnScalar = 40,
}

#[stable(feature = "rust1", since = "1.0.0")]
/// Copies of these invocation types here for when we're running
/// in environments without libxous.
pub enum SyscallResult {
    Ok = 0,
    ConnectionID = 7,
    Message = 9,
    Scalar1 = 14,
    Scalar2 = 15,
    MemoryReturned = 18,
}

#[stable(feature = "rust1", since = "1.0.0")]
pub fn lend_mut(
    connection: Connection,
    opcode: usize,
    data: *mut u8,
    size: usize,
    arg1: usize,
    arg2: usize,
) -> Result<(usize, usize), usize> {
    let mut a0 = Syscall::SendMessage as usize;
    let mut a1: usize = connection.0.try_into().unwrap();
    let mut a2 = InvokeType::LendMut as usize;
    let a3 = opcode;
    let a4 = data as usize;
    let a5 = size;
    let a6 = arg1;
    let a7 = arg2;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") a0,
            inlateout("a1") a1,
            inlateout("a2") a2,
            inlateout("a3") a3 => _,
            inlateout("a4") a4 => _,
            inlateout("a5") a5 => _,
            inlateout("a6") a6 => _,
            inlateout("a7") a7 => _,
        )
    };

    let result = a0;

    if result == SyscallResult::MemoryReturned as usize {
        Ok((a1, a2))
    } else {
        println!("Unexpected memory return value: {} ({})", result, a1);
        Err(a1)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
pub fn lend(
    connection: Connection,
    opcode: usize,
    data: *const u8,
    size: usize,
    arg1: usize,
    arg2: usize,
) -> Result<(usize, usize), usize> {
    let mut a0 = Syscall::SendMessage as usize;
    let a1: usize = connection.0.try_into().unwrap();
    let a2 = InvokeType::Lend as usize;
    let a3 = opcode;
    let a4 = data as usize;
    let a5 = size;
    let mut a6 = arg1;
    let mut a7 = arg2;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") a0,
            inlateout("a1") a1 => _,
            inlateout("a2") a2 => _,
            inlateout("a3") a3 => _,
            inlateout("a4") a4 => _,
            inlateout("a5") a5 => _,
            inlateout("a6") a6,
            inlateout("a7") a7,
        )
    };

    let result = a0;

    if result == SyscallResult::MemoryReturned as usize {
        Ok((a6, a7))
    } else {
        println!("Unexpected memory return value: {} ({})", result, a1);
        Err(a1)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
pub fn return_memory(
    message_id: MessageId,
    memory: &[u8],
    arg1: usize,
    arg2: usize,
) -> Result<(), usize> {
    let a0 = Syscall::ReturnMemory as usize;
    let a1: usize = message_id.try_into().unwrap();
    let a6 = 0;
    let a7 = 0;

    let mut result: usize;
    let mut error: usize;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") a0 => result,
            inlateout("a1") a1 => error,
            inlateout("a2") memory.as_ptr() => _,
            inlateout("a3") memory.len()=> _,
            inlateout("a4") arg1 => _,
            inlateout("a5") arg2 => _,
            inlateout("a6") a6 => _,
            inlateout("a7") a7 => _,
        )
    };
    if result == SyscallResult::MemoryReturned as usize {
        Ok(())
    } else {
        Err(error)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
pub fn return_scalar(message_id: MessageId, args: [usize; 5]) -> Result<(), usize> {
    let a0 = Syscall::ReturnMemory as usize;
    let a1: usize = message_id.try_into().unwrap();
    let a2 = args.get(0).map(|v| *v).unwrap_or_default();
    let a3 = args.get(1).map(|v| *v).unwrap_or_default();
    let a4 = args.get(2).map(|v| *v).unwrap_or_default();
    let a5 = args.get(3).map(|v| *v).unwrap_or_default();
    let a6 = args.get(4).map(|v| *v).unwrap_or_default();
    let a7 = 0;

    let mut result: usize;
    let mut error: usize;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") a0 => result,
            inlateout("a1") a1 => error,
            inlateout("a2") a2 => _,
            inlateout("a3") a3 => _,
            inlateout("a4") a4 => _,
            inlateout("a5") a5 => _,
            inlateout("a6") a6 => _,
            inlateout("a7") a7 => _,
        )
    };
    if result == SyscallResult::Ok as usize {
        Ok(())
    } else {
        Err(error)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
pub fn connect(address: ServerAddress) -> Result<Connection, usize> {
    let a0 = Syscall::Connect as usize;
    let a1: usize = address.0[0].try_into().unwrap();
    let a2: usize = address.0[1].try_into().unwrap();
    let a3: usize = address.0[2].try_into().unwrap();
    let a4: usize = address.0[3].try_into().unwrap();
    let a5 = 0;
    let a6 = 0;
    let a7 = 0;

    let mut result: usize;
    let mut value: usize;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") a0 => result,
            inlateout("a1") a1 => value,
            inlateout("a2") a2 => _,
            inlateout("a3") a3 => _,
            inlateout("a4") a4 => _,
            inlateout("a5") a5 => _,
            inlateout("a6") a6 => _,
            inlateout("a7") a7 => _,
        )
    };
    if result == SyscallResult::ConnectionID as usize {
        Ok(value.try_into().unwrap())
    } else {
        Err(value)
    }
}

use crate::io;
use xous::{
    connect, map_memory, send_message, try_send_message, MemoryRange, MemorySize, Message,
    ScalarMessage, CID, SID,
};

pub struct Stdin;
pub struct Stdout {
    mem: Option<MemoryRange>,
}
pub struct Stderr;

static mut LOG_SERVER_CONNECTION: Option<CID> = None;
const MEM_READ_WRITE: usize = 0b0000_0010 | 0b0000_0100;

impl Stdin {
    pub const fn new() -> Stdin {
        Stdin
    }
}

impl io::Read for Stdin {
    fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        Ok(0)
    }
}

impl Stdout {
    pub const fn new() -> Stdout {
        Stdout { mem: None }
    }
    fn ensure_connection(&mut self) {
        unsafe {
            // Accessing a global mutable is safe, because this call is idempotent.
            // If there is a fight between threads, the result will be the same.
            if LOG_SERVER_CONNECTION.is_none() {
                LOG_SERVER_CONNECTION =
                    Some(connect(SID::from_bytes(b"xous-log-server ").unwrap()).unwrap());
            }
        }
        if self.mem.is_none() {
            self.mem = Some(map_memory(None, None, 4096, MEM_READ_WRITE).unwrap());
        }
    }
}

impl io::Write for Stdout {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.ensure_connection();
        let mem = &self.mem.unwrap();
        let connection = unsafe { LOG_SERVER_CONNECTION.unwrap() };
        let s = unsafe { core::slice::from_raw_parts_mut(mem.as_mut_ptr(), 4096) };
        for chunk in buf.chunks(s.len()) {
            for (dest, src) in s.iter_mut().zip(chunk) {
                *dest = *src;
            }
            let message = Message::new_lend(1, *mem, None, MemorySize::new(chunk.len()));
            send_message(connection, message).unwrap();
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        // self.ensure_connection();
        // unsafe { STDOUT_STRING_BUFFER.as_mut().unwrap() }
        //     .lend(unsafe { LOG_SERVER_CONNECTION.unwrap() }, 1)
        //     .unwrap();
        Ok(())
    }
}

impl Stderr {
    pub const fn new() -> Stderr {
        Stderr
    }
}

impl io::Write for Stderr {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub const STDIN_BUF_SIZE: usize = 0;

pub fn is_ebadf(_err: &io::Error) -> bool {
    true
}

pub struct PanicWriter {
    conn: CID,
}

impl PanicWriter {
    // Group `usize` bytes into a `usize` and return it, beginning
    // from `offset` * sizeof(usize) bytes from the start. For example,
    // `group_or_null([1,2,3,4,5,6,7,8], 1)` on a 32-bit system will
    // return a usize with 5678 packed into it.
    fn group_or_null(data: &[u8], offset: usize) -> usize {
        let start = offset * core::mem::size_of::<usize>();
        let mut out_array = [0u8; core::mem::size_of::<usize>()];
        for i in 0..core::mem::size_of::<usize>() {
            out_array[i] = if i + start < data.len() { data[start + i] } else { 0 };
        }
        usize::from_le_bytes(out_array)
    }
}

impl io::Write for PanicWriter {
    fn write(&mut self, s: &[u8]) -> core::result::Result<usize, io::Error> {
        for c in s.chunks(core::mem::size_of::<usize>() * 4) {
            // Text is grouped into 4x `usize` words. The id is 1100 plus
            // the number of characters in this message.
            let panic_msg = ScalarMessage {
                id: 1100 + c.len(),
                arg1: Self::group_or_null(&c, 0),
                arg2: Self::group_or_null(&c, 1),
                arg3: Self::group_or_null(&c, 2),
                arg4: Self::group_or_null(&c, 3),
            };
            try_send_message(self.conn, Message::Scalar(panic_msg)).ok();
        }
        Ok(s.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub fn panic_output() -> Option<impl io::Write> {
    if let Ok(connection) = xous::try_connect(SID::from_bytes(b"xous-log-server ").unwrap()) {
        let pw = PanicWriter { conn: connection };

        // Send the "We're panicking" message (1000).
        try_send_message(connection, Message::new_scalar(1000, 0, 0, 0, 0)).ok();
        Some(pw)
    } else {
        None
    }
}

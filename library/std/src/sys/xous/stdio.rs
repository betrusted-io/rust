use crate::io;
use xous::{
    connect, map_memory, send_message, try_send_message, MemoryRange, MemorySize, Message,
    ScalarMessage, CID, SID,
};

/// Messages will get split into chunks that are, at most, this
/// number of bytes.
const MESSAGE_CHUNK_SIZE: usize = 4096;

pub struct Stdin;
pub struct Stdout {
    mem: Option<MemoryRange>,
}
pub struct Stderr;

static mut LOG_SERVER_CONNECTION: Option<CID> = None;

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
            self.mem = Some(
                map_memory(
                    None,
                    None,
                    MESSAGE_CHUNK_SIZE,
                    xous::MemoryFlags::R | xous::MemoryFlags::W,
                )
                .unwrap(),
            );
        }
    }
}

impl io::Write for Stdout {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.ensure_connection();
        let mem = &self.mem.unwrap();
        let connection = unsafe { LOG_SERVER_CONNECTION.unwrap() };
        let s = unsafe { core::slice::from_raw_parts_mut(mem.as_mut_ptr(), MESSAGE_CHUNK_SIZE) };
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

#[derive(Copy, Clone)]
pub struct PanicWriter {
    conn: CID,
    gfx_conn: Option<CID>,
}

#[repr(C, align(4096))]
pub struct PanicNotifier {
    raw: [u8; 4096],
}

impl PanicWriter {
    // Group `usize` bytes into a `usize` and return it, beginning
    // from `offset` * sizeof(usize) bytes from the start. For example,
    // `group_or_null([1,2,3,4,5,6,7,8], 1)` on a 32-bit system will
    // return a usize with 5678 packed into it.
    fn group_or_null(data: &[u8], offset: usize) -> usize {
        let start = offset * core::mem::size_of::<usize>();
        let mut out_array = [0u8; core::mem::size_of::<usize>()];
        if start < data.len() {
            for (dest, src) in out_array.iter_mut().zip(&data[start..]) {
                *dest = *src;
            }
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
        // serialze the text to the graphics panic handler, only if we were able
        // to acquire a connection to it. Text length is encoded in the `valid` field,
        // the data itself in the buffer. Typically several messages are require to
        // fully transmit the entire panic message.
        if let Some(conn) = self.gfx_conn {
            let mut request = PanicNotifier { raw: [0u8; 4096] };
            for (&s, d) in s.iter().zip(request.raw.iter_mut()) {
                *d = s;
            }
            let buf = unsafe {
                xous::MemoryRange::new(
                    &mut request as *mut PanicNotifier as usize,
                    core::mem::size_of::<PanicNotifier>(),
                )
                .unwrap()
            };
            try_send_message(
                conn,
                xous::Message::new_lend(
                    0, // append panic text
                    buf,
                    None,
                    xous::MemorySize::new(s.len()),
                ),
            )
            .ok();
        }
        Ok(s.len())
    }

    // Tests show that this does not seem to be reliably called at the end of a panic
    // print, so, we can't rely on this to e.g. trigger a graphics update.
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

use crate::cell::RefCell;
thread_local! { static PANIC_WRITER: RefCell<Option<PanicWriter>> = RefCell::new(None) }

pub fn panic_output() -> Option<impl io::Write> {
    PANIC_WRITER.with(|pwr| {
        if pwr.borrow().is_none() {
            // Generally this won't fail because every server has already allocated this connection.
            let connection = xous::connect(SID::from_bytes(b"xous-log-server ").unwrap()).unwrap();

            // This is possibly fallible in the case that the connection table is full,
            // and we can't make the connection to the graphics server. Most servers do not already
            // have this connection.
            let gfx_conn = xous::try_connect(SID::from_bytes(b"panic-to-screen!").unwrap()).ok();

            let pw = PanicWriter { conn: connection, gfx_conn };

            // Send the "We're panicking" message (1000).
            try_send_message(connection, Message::new_scalar(1000, 0, 0, 0, 0)).ok();
            *pwr.borrow_mut() = Some(pw);
        }
        *pwr.borrow()
    })
}

use crate::cell::RefCell;
use crate::io;
thread_local! { static PANIC_WRITER: RefCell<Option<PanicWriter>> = RefCell::new(None) }

pub struct Stdin;
pub struct Stdout {}
pub struct Stderr;

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
        Stdout {}
    }
}

impl io::Write for Stdout {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        #[repr(align(4096))]
        struct LendBuffer([u8; 4096]);
        let mut lend_buffer = LendBuffer([0u8; 4096]);
        let connection = crate::os::xous::services::log_server();
        for chunk in buf.chunks(lend_buffer.0.len()) {
            for (dest, src) in lend_buffer.0.iter_mut().zip(chunk) {
                *dest = *src;
            }
            crate::os::xous::ffi::lend(connection, 1, &lend_buffer.0, 0, chunk.len()).unwrap();
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
    conn: crate::os::xous::ffi::Connection,
    gfx_conn: Option<crate::os::xous::ffi::Connection>,
}

impl io::Write for PanicWriter {
    fn write(&mut self, s: &[u8]) -> core::result::Result<usize, io::Error> {
        for c in s.chunks(core::mem::size_of::<usize>() * 4) {
            // Text is grouped into 4x `usize` words. The id is 1100 plus
            // the number of characters in this message.
            // Ignore errors since we're already panicking.
            crate::os::xous::ffi::try_scalar(
                self.conn,
                crate::os::xous::services::LogScalar::AppendPanicMessage(&c).into(),
            )
            .ok();
        }

        // Serialze the text to the graphics panic handler, only if we were able
        // to acquire a connection to it. Text length is encoded in the `valid` field,
        // the data itself in the buffer. Typically several messages are require to
        // fully transmit the entire panic message.
        if let Some(connection) = self.gfx_conn {
            #[repr(align(4096))]
            struct Request([u8; 4096]);
            let mut request = Request([0u8; 4096]);
            for (&s, d) in s.iter().zip(request.0.iter_mut()) {
                *d = s;
            }
            crate::os::xous::ffi::try_lend(
                connection,
                0, /* AppendPanicText */
                &request.0,
                0,
                s.len(),
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

pub fn panic_output() -> Option<impl io::Write> {
    PANIC_WRITER.with(|pwr| {
        if pwr.borrow().is_none() {
            // Generally this won't fail because every server has already allocated this connection.
            let conn = crate::os::xous::services::log_server();

            // This is possibly fallible in the case that the connection table is full,
            // and we can't make the connection to the graphics server. Most servers do not already
            // have this connection.
            let gfx_conn = crate::os::xous::services::try_connect("panic-to-screen!");

            let pw = PanicWriter { conn, gfx_conn };

            // Send the "We're panicking" message (1000).
            crate::os::xous::ffi::scalar(
                conn,
                crate::os::xous::services::PanicToScreenScalar::BeginPanic.into(),
            )
            .ok();
            *pwr.borrow_mut() = Some(pw);
        }
        *pwr.borrow()
    })
}

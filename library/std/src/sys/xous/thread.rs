use crate::ffi::CStr;
use crate::io;
use crate::num::NonZeroUsize;
use crate::time::Duration;

pub struct Thread {
    tid: xous::TID,
}

pub const DEFAULT_MIN_STACK_SIZE: usize = 131072;
static mut TICKTIMER_CID: Option<xous::CID> = None;

impl Thread {
    // unsafe: see thread::Builder::spawn_unchecked for safety requirements
    pub unsafe fn new(stack: usize, p: Box<dyn FnOnce()>) -> io::Result<Thread> {
        let p = Box::into_raw(box p);
        let stack_size = crate::cmp::max(stack, 4096);
        let stack = xous::map_memory(None, None, stack_size, 0b111 /* R+W+X */)
            .map_err(|code| io::Error::from_raw_os_error(code as i32))?;

        let call = xous::SysCall::CreateThread(xous::ThreadInit {
            call: thread_start as *mut usize as usize,
            stack,
            arg1: p as usize,
            arg2: 0,
            arg3: 0,
            arg4: 0,
        });
        let result =
            xous::rsyscall(call).map_err(|code| io::Error::from_raw_os_error(code as i32))?;

        extern "C" fn thread_start(main: *mut usize) -> *mut usize {
            unsafe {
                // // Next, set up our stack overflow handler which may get triggered if we run
                // // out of stack.
                // let _handler = stack_overflow::Handler::new();
                // Finally, let's run some code.
                Box::from_raw(main as *mut Box<dyn FnOnce()>)();
            }
            crate::ptr::null_mut()
        }

        if let xous::Result::ThreadID(tid) = result {
            Ok(Thread { tid })
        } else {
            Err(io::Error::from_raw_os_error(1))
        }
    }

    pub fn yield_now() {
        xous::syscall::yield_slice();
    }

    pub fn set_name(_name: &CStr) {
        // nope
    }

    pub fn sleep(dur: Duration) {
        // Sleep is done by connecting to the ticktimer server and sending
        // a blocking message.
        if unsafe { TICKTIMER_CID.is_none() } {
            unsafe {
                TICKTIMER_CID = Some(
                    xous::connect(xous::SID::from_bytes(b"ticktimer-server").unwrap()).unwrap(),
                )
            };
        }
        let cid = unsafe { TICKTIMER_CID.unwrap() };

        // Because the sleep server works on units of `usized milliseconds`, split
        // the messages up into these chunks. This means we may run into issues
        // if you try to sleep a thread for more than 49 days on a 32-bit system.
        let mut millis = dur.as_millis();
        while millis > 0 {
            let sleep_duration =
                if millis > (usize::MAX as _) { usize::MAX } else { millis as usize };
            xous::send_message(cid, xous::Message::new_blocking_scalar(1, sleep_duration, 0, 0, 0))
                .expect("Ticktimer: failure to send message to Ticktimer");
            millis -= sleep_duration as u128;
        }
    }

    pub fn join(self) {
        xous::syscall::join_thread(self.tid).unwrap();
    }
}

pub fn available_concurrency() -> io::Result<NonZeroUsize> {
    // We're unicore right now.
    Ok(unsafe { NonZeroUsize::new_unchecked(1) })
}

pub mod guard {
    pub type Guard = !;
    pub unsafe fn current() -> Option<Guard> {
        None
    }
    pub unsafe fn init() -> Option<Guard> {
        None
    }
}

pub fn my_id() -> u32 {
    xous::current_tid().map(|tid| tid as u32).unwrap_or_default()
}

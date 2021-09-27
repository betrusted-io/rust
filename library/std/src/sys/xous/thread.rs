use crate::ffi::CStr;
use crate::io;
use crate::num::NonZeroUsize;
use crate::time::Duration;

pub struct Thread {
    tid: xous::TID,
}

pub const DEFAULT_MIN_STACK_SIZE: usize = 131072;
pub const GUARD_PAGE_SIZE: usize = 4096;
static mut TICKTIMER_CID: Option<xous::CID> = None;

impl Thread {
    // unsafe: see thread::Builder::spawn_unchecked for safety requirements
    pub unsafe fn new(stack: usize, p: Box<dyn FnOnce()>) -> io::Result<Thread> {
        let p = Box::into_raw(box p);
        let stack_size = crate::cmp::max(stack, 4096);

        // No access to this page. Note: Write-only pages are illegal, and will
        // cause an access violation.
        let guard_page_pre = xous::map_memory(None, None, GUARD_PAGE_SIZE, 0b100 /* W */)
            .map_err(|code| io::Error::from_raw_os_error(code as i32))?;

        // Stack sandwiched between guard pages
        let stack = xous::map_memory(None, None, stack_size, 0b111 /* R+W+X */)
            .map_err(|code| io::Error::from_raw_os_error(code as i32))?;

        // No access to this page. Note: Write-only pages are illegal, and will
        // cause an access violation.
        let guard_page_post = xous::map_memory(None, None, GUARD_PAGE_SIZE, 0b100 /* W */)
            .map_err(|code| io::Error::from_raw_os_error(code as i32))?;

        // Ensure that the pages are laid out like we expect them.
        let pre_addr = guard_page_pre.as_ptr() as usize;
        let stack_addr = stack.as_ptr() as usize;
        let post_addr = guard_page_post.as_ptr() as usize;

        assert_eq!(pre_addr + GUARD_PAGE_SIZE, stack_addr);
        assert_eq!(pre_addr + GUARD_PAGE_SIZE + stack_size, post_addr);

        let call = xous::SysCall::CreateThread(xous::ThreadInit {
            call: thread_start as *mut usize as usize,
            stack,
            arg1: p as usize,
            arg2: pre_addr,
            arg3: stack_size,
            arg4: 0,
        });
        let result =
            xous::rsyscall(call).map_err(|code| io::Error::from_raw_os_error(code as i32))?;

        extern "C" fn thread_start(main: *mut usize, guard_page_pre: usize, stack_size: usize) {
            unsafe {
                // // Next, set up our stack overflow handler which may get triggered if we run
                // // out of stack.
                // let _handler = stack_overflow::Handler::new();
                // Finally, let's run some code.
                Box::from_raw(main as *mut Box<dyn FnOnce()>)();
            }

            // Destroy TLS, which will free the TLS page
            unsafe {
                crate::sys::thread_local_key::destroy_tls();
            }

            // Deallocate the stack memory, along with the guard pages.
            let mapped_memory_base = guard_page_pre;
            let mapped_memory_length = GUARD_PAGE_SIZE + stack_size + GUARD_PAGE_SIZE;
            unsafe {
                asm!(
                    "ecall",
                    in("a0") xous::SysCallNumber::UnmapMemory as usize,
                    in("a1") mapped_memory_base,
                    in("a2") mapped_memory_length,
                    options(nomem, nostack)
                );
            }

            // Exit the thread by returning to the magic address 0xff80_3000u32
            unsafe {
                asm!("ret", in("a0") 0, in("ra") 0xff80_3000u32,
                    options(nomem, nostack, noreturn)
                );
            }
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

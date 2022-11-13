use crate::ffi::CStr;
use crate::io;
use crate::num::NonZeroUsize;
use crate::os::xous::ffi::MemoryFlags;
use crate::time::Duration;
use core::arch::asm;

pub struct Thread {
    tid: crate::os::xous::ffi::ThreadId,
}

pub const DEFAULT_MIN_STACK_SIZE: usize = 131072;
const MIN_STACK_SIZE: usize = 4096;
pub const GUARD_PAGE_SIZE: usize = 4096;

impl Thread {
    // unsafe: see thread::Builder::spawn_unchecked for safety requirements
    pub unsafe fn new(stack: usize, p: Box<dyn FnOnce()>) -> io::Result<Thread> {
        let p = Box::into_raw(box p);
        let stack_size = crate::cmp::max(stack, MIN_STACK_SIZE);

        // Allocate the whole thing, then divide it up after the fact. This ensures that
        // even if there's a context switch during this function, the whole stack plus
        // guard pages will remain contiguous.
        let stack_plus_guard_pages: core::ops::Range<*mut u8> = crate::os::xous::ffi::map_memory(
            None,
            None,
            stack_size + GUARD_PAGE_SIZE + GUARD_PAGE_SIZE,
            MemoryFlags::R | MemoryFlags::W | MemoryFlags::X,
        )
        .map_err(|code| io::Error::from_raw_os_error(code as i32))?;

        // No access to this page. Note: Write-only pages are illegal, and will
        // cause an access violation.
        let guard_page_pre = unsafe {
            stack_plus_guard_pages.start..stack_plus_guard_pages.start.add(GUARD_PAGE_SIZE)
        };
        crate::os::xous::ffi::update_memory_flags(&guard_page_pre, MemoryFlags::W)
            .map_err(|code| io::Error::from_raw_os_error(code as i32))?;

        // Stack sandwiched between guard pages
        let stack = unsafe { guard_page_pre.end..guard_page_pre.end.add(stack_size) };

        // No access to this page. Note: Write-only pages are illegal, and will
        // cause an access violation.
        let guard_page_post = unsafe { stack.end..stack.end.add(GUARD_PAGE_SIZE) };
        crate::os::xous::ffi::update_memory_flags(&guard_page_post, MemoryFlags::W)
            .map_err(|code| io::Error::from_raw_os_error(code as i32))?;

        // Ensure that the pages are laid out like we expect them.
        let pre_addr = guard_page_pre.start as usize;
        let stack_addr = stack.start as usize;
        let post_addr = guard_page_post.start as usize;

        assert_eq!(pre_addr + GUARD_PAGE_SIZE, stack_addr);
        assert_eq!(pre_addr + GUARD_PAGE_SIZE + stack_size, post_addr);

        let tid = crate::os::xous::ffi::create_thread(
            thread_start as *mut usize,
            stack,
            p as usize,
            pre_addr,
            stack_size,
            0,
        )
        .map_err(|code| io::Error::from_raw_os_error(code as i32))?;

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
                    in("a0") crate::os::xous::ffi::Syscall::UnmapMemory as usize,
                    in("a1") mapped_memory_base,
                    in("a2") mapped_memory_length,
                    options(nomem, nostack)
                );
            }

            // Exit the thread by returning to the magic address 0xff80_3000usize,
            // which tells the kernel to deallcate this thread.
            unsafe {
                asm!("ret", in("a0") 0, in("ra") 0xff80_3000usize,
                    options(nomem, nostack, noreturn)
                );
            }
        }

        Ok(Thread { tid })
    }

    pub fn yield_now() {
        crate::os::xous::ffi::do_yield();
    }

    pub fn set_name(_name: &CStr) {
        // nope
    }

    pub fn sleep(dur: Duration) {
        // Because the sleep server works on units of `usized milliseconds`, split
        // the messages up into these chunks. This means we may run into issues
        // if you try to sleep a thread for more than 49 days on a 32-bit system.
        let mut millis = dur.as_millis();
        while millis > 0 {
            let sleep_duration =
                if millis > (usize::MAX as _) { usize::MAX } else { millis as usize };
            crate::os::xous::ffi::blocking_scalar(
                crate::os::xous::services::ticktimer_server(),
                crate::os::xous::services::TicktimerScalar::SleepMs(sleep_duration).into(),
            )
            .expect("failed to send message to ticktimer server");
            millis -= sleep_duration as u128;
        }
    }

    pub fn join(self) {
        crate::os::xous::ffi::join_thread(self.tid).unwrap();
    }
}

pub fn available_parallelism() -> io::Result<NonZeroUsize> {
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

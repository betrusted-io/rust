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
        let mut stack_size = crate::cmp::max(stack, MIN_STACK_SIZE);

        if (stack_size & 4095) != 0 {
            stack_size = (stack_size + 4095) & !4095;
        }

        // Allocate the whole thing, then divide it up after the fact. This ensures that
        // even if there's a context switch during this function, the whole stack plus
        // guard pages will remain contiguous.
        let stack_plus_guard_pages: &mut [u8] = crate::os::xous::ffi::map_memory(
            None,
            None,
            stack_size + GUARD_PAGE_SIZE + GUARD_PAGE_SIZE,
            MemoryFlags::R | MemoryFlags::W | MemoryFlags::X,
        )
        .map_err(|code| io::Error::from_raw_os_error(code as i32))?;

        // No access to this page. Note: Write-only pages are illegal, and will
        // cause an access violation.
        crate::os::xous::ffi::update_memory_flags(
            &mut stack_plus_guard_pages[0..GUARD_PAGE_SIZE],
            MemoryFlags::W,
        )
        .map_err(|code| io::Error::from_raw_os_error(code as i32))?;

        // No access to this page. Note: Write-only pages are illegal, and will
        // cause an access violation.
        crate::os::xous::ffi::update_memory_flags(
            &mut stack_plus_guard_pages[(GUARD_PAGE_SIZE + stack_size)..],
            MemoryFlags::W,
        )
        .map_err(|code| io::Error::from_raw_os_error(code as i32))?;

        let tid = crate::os::xous::ffi::create_thread(
            thread_start as *mut usize,
            &stack_plus_guard_pages[GUARD_PAGE_SIZE..(stack_size - GUARD_PAGE_SIZE)],
            p as usize,
            stack_plus_guard_pages.as_ptr() as usize,
            stack_size,
            0,
        )
        .map_err(|code| io::Error::from_raw_os_error(code as i32))?;

        extern "C" fn thread_start(main: *mut usize, guard_page_pre: usize, stack_size: usize) {
            unsafe {
                // Finally, let's run some code.
                Box::from_raw(main as *mut Box<dyn FnOnce()>)();
            }

            // Destroy TLS, which will free the TLS page and call the destructor for
            // any thread local storage.
            unsafe {
                crate::sys::thread_local_key::destroy_tls();
            }

            // Deallocate the stack memory, along with the guard pages.
            let mapped_memory_base = guard_page_pre;
            let mapped_memory_length = GUARD_PAGE_SIZE + stack_size + GUARD_PAGE_SIZE;
            unsafe {
                asm!(
                    "ecall",
                    inlateout("a0") crate::os::xous::ffi::Syscall::UnmapMemory as usize => _,
                    inlateout("a1") mapped_memory_base => _,
                    inlateout("a2") mapped_memory_length => _,
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

use core::ptr;
use Allocator;

pub struct System {
    _priv: (),
}

impl System {
    pub const fn new() -> System {
        System { _priv: () }
    }
}

unsafe impl Allocator for System {
    /// Allocate an additional `size` bytes on the heap, and return a new
    /// chunk of memory, as well as the size of the allocation and some
    /// flags. Since flags are unused on this platform, they will always
    /// be `0`.
    fn alloc(&self, size: usize) -> (*mut u8, usize, u32) {
        let size = if size == 0 {
            4096
        } else if size & 4095 == 0 {
            size
        } else {
            size + (4096 - (size & 4095))
        };

        let syscall =
            xous::SysCall::IncreaseHeap(size, xous::MemoryFlags::R | xous::MemoryFlags::W);
        if let Ok(xous::Result::MemoryRange(mem)) = xous::rsyscall(syscall) {
            let start = mem.as_ptr() as usize - size + mem.len();
            (start as *mut u8, size, 0)
        } else {
            (ptr::null_mut(), 0, 0)
        }
    }

    fn remap(&self, _ptr: *mut u8, _oldsize: usize, _newsize: usize, _can_move: bool) -> *mut u8 {
        // TODO
        ptr::null_mut()
    }

    fn free_part(&self, _ptr: *mut u8, _oldsize: usize, _newsize: usize) -> bool {
        false
    }

    fn free(&self, _ptr: *mut u8, _size: usize) -> bool {
        false
    }

    fn can_release_part(&self, _flags: u32) -> bool {
        false
    }

    fn allocates_zeros(&self) -> bool {
        true
    }

    fn page_size(&self) -> usize {
        4 * 1024
    }
}

#[cfg(feature = "global")]
pub fn acquire_global_lock() {
    // global feature should not be enabled
    unimplemented!()
}

#[cfg(feature = "global")]
pub fn release_global_lock() {
    // global feature should not be enabled
    unimplemented!()
}

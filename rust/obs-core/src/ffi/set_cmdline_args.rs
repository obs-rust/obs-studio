//! `obs_set_cmdline_args()` over the C-owned command line: the stored
//! `struct obs_cmdline_args` behind `obs_core_cmdline_args_field()` (the
//! adapter `obs_get_cmdline_args()` reads too) and one `bmalloc` block. What
//! is read, allocated and copied is the safe core's
//! ([`crate::set_cmdline_args`]).
use core::ffi::{CStr, c_char, c_int, c_void};
use core::ptr;

use super::core_cmdline_getter::CommandLineArgs;

unsafe extern "C" {
    fn obs_core_cmdline_args_field() -> *const CommandLineArgs;
    fn bmalloc(size: usize) -> *mut c_void;
}

/// The stored command line and the caller's `argv`.
struct Host {
    args: *mut CommandLineArgs,
    argv: *const *const c_char,
}

impl crate::set_cmdline_args::CommandLine for Host {
    fn stored_argc(&mut self) -> i32 {
        // SAFETY: `args` is the live static the adapter returns.
        unsafe { (*self.args).argc }
    }
    fn store_argc(&mut self, argc: i32) {
        // SAFETY: as above.
        unsafe { (*self.args).argc = argc }
    }
    fn arg_len(&mut self, index: usize) -> usize {
        // SAFETY: the caller passes `argc` NUL-terminated strings, and the
        // safe core asks only for an index below `argc`.
        unsafe { CStr::from_ptr(*self.argv.add(index)).count_bytes() }
    }
    fn allocate(&mut self, size: usize) {
        // SAFETY: `bmalloc` returns `size` writable bytes (it aborts on
        // failure); the block is stored as the table, as the C does.
        unsafe { (*self.args).argv = bmalloc(size).cast() }
    }
    fn copy_arg(&mut self, index: usize, offset: usize, len: usize) {
        // SAFETY: the safe core sized the block as the table of `argc + 1`
        // pointers plus every string with its terminator, and places each
        // string after the table and the previous one, so the slot and the
        // `len` bytes at `offset` lie inside it; the source is the caller's
        // string of `len - 1` bytes and its NUL.
        unsafe {
            let table = (*self.args).argv;
            let data = table.cast::<u8>().add(offset);
            *table.add(index) = data.cast();
            ptr::copy_nonoverlapping((*self.argv.add(index)).cast::<u8>(), data, len);
        }
    }
    fn terminate(&mut self, index: usize) {
        // SAFETY: slot `argc` is the table's last one.
        unsafe { *(*self.args).argv.add(index) = ptr::null_mut() }
    }
}

/// Stores one copy of the command line, the first time a non-zero count
/// is given (a zero count stores an empty table and still counts as unset).
///
/// # Safety
/// As for the C function: `argv` holds `argc` NUL-terminated strings, and no
/// other thread sets, reads or frees the command line meanwhile.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_set_cmdline_args(argc: c_int, argv: *const *const c_char) {
    let mut host = Host {
        // SAFETY: the adapter returns the address of the live static.
        args: unsafe { obs_core_cmdline_args_field() }.cast_mut(),
        argv,
    };
    crate::set_cmdline_args::set_cmdline_args(&mut host, argc, size_of::<*mut c_char>());
}

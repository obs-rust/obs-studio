//! Public by-value snapshot, borrowing the C-owned argument table.
use core::ffi::{c_char, c_int};

/// The unchanged public struct obs_cmdline_args from obs.h.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CommandLineArgs {
    pub argc: c_int,
    pub argv: *mut *mut c_char,
}

unsafe extern "C" {
    fn obs_core_cmdline_args_field() -> *const CommandLineArgs;
}

/// Return the stored count and argument-table pointer without taking ownership.
///
/// # Safety
/// The setter and shutdown must not mutate the global snapshot concurrently.
/// Returned pointers retain the lifetime and read-only contract of obs.h.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_cmdline_args() -> CommandLineArgs {
    // SAFETY: the adapter always returns the live static struct's address,
    // including before initialization. The argument table is never dereferenced.
    let stored = unsafe { &*obs_core_cmdline_args_field() };
    let (argc, argv) = crate::core_cmdline_getter::snapshot(&stored.argc, &stored.argv);
    CommandLineArgs { argc, argv }
}

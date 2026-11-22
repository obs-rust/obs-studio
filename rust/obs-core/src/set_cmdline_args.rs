//! Safe core of `libobs/obs-set-cmdline-args.c.h`: `obs_set_cmdline_args()`,
//! which keeps one copy of the process command line for
//! `obs_get_cmdline_args()`.
//!
//! The stored `argc` is read once. Once it is non-zero
//! ([`cmdline_set`]) the call changes nothing. Otherwise the new count is
//! stored and the arguments are copied into one `bmalloc` block: a table of
//! `argc + 1` pointers ([`cmdline_table`]), then each string with its
//! terminator right after the previous one ([`cmdline_len`]), and the table
//! ends with NULL. A zero count stores an empty table, and since zero still
//! counts as unset, a later call copies again (the empty table is not freed:
//! characterized, not endorsed). `obs_shutdown()` frees the block with one
//! `bfree`.
//!
//! As in the C, the lengths are read in a first pass that sizes the block
//! and again while copying. A negative count is outside the contract of
//! `obs.h` (the original writes before its block); the shim stores the count
//! and allocates nothing.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

use crate::core_setup_rules::{cmdline_len, cmdline_set, cmdline_table};

/// The stored command line (`struct obs_cmdline_args`), the caller's
/// arguments and the block allocation.
pub trait CommandLine {
    /// One read of the stored `argc`.
    fn stored_argc(&mut self) -> i32;
    /// Store `argc`.
    fn store_argc(&mut self, argc: i32);
    /// `strlen(argv[index])` of the caller's argument.
    fn arg_len(&mut self, index: usize) -> usize;
    /// `args->argv = bmalloc(size)`.
    fn allocate(&mut self, size: usize);
    /// Point `args->argv[index]` at `offset` bytes into the block and copy
    /// the caller's argument there, `len` bytes with its terminator.
    fn copy_arg(&mut self, index: usize, offset: usize, len: usize);
    /// `args->argv[index] = NULL`.
    fn terminate(&mut self, index: usize);
}

/// `obs_set_cmdline_args(argc, argv)` with pointers of `ptr_size` bytes.
pub fn set_cmdline_args<C: CommandLine>(host: &mut C, argc: i32, ptr_size: usize) {
    if cmdline_set(host.stored_argc()) {
        return;
    }
    host.store_argc(argc);
    let Ok(count) = usize::try_from(argc) else {
        return;
    };

    let mut len = 0;
    for index in 0..count {
        len = cmdline_len(len, host.arg_len(index));
    }
    let table = cmdline_table(argc, ptr_size);
    host.allocate(table.wrapping_add(len));

    let mut offset = table;
    for index in 0..count {
        let len = cmdline_len(0, host.arg_len(index));
        host.copy_arg(index, offset, len);
        offset = offset.wrapping_add(len);
    }
    host.terminate(count);
}

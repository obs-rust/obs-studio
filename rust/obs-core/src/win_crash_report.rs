//! Safe core of `libobs/obs-win-crash-report.c.h`: the rules of
//! `libobs/obs-win-crash-handler.c.h`'s crash report.
//!
//! - [`contains`]: which module holds an address (`enum_all_modules()`
//!   names the faulting module with it, `enum_module()` each frame's);
//! - [`is_crashed`], [`in_pass`] and [`other_process`]:
//!   `write_thread_trace()`'s choice of the threads each pass of
//!   `write_thread_traces()` traces, the crashed one first;
//! - [`names_symbol`], [`mask_address`] and [`mask_param`]: `walk_stack()`'s
//!   frame line, and a 32-bit build's masks on the frame's values;
//! - [`name_prefix`]: `get_thread_name()`'s leading space;
//! - [`pass_on`]: when `exception_handler()` passes the exception on.
//!
//! The C keeps every DbgHelp, toolhelp and thread call, the conversions,
//! every `dstr` call and format string, `bcrash()` and both static flags,
//! each at its point, and passes their values in. These run inside the
//! unhandled exception filter, maybe with a damaged heap or stack: each is
//! a compare or mask on integers that allocates nothing, indexes nothing
//! and has no panic path, and none keeps state.
//!
//! The core is OS-free: it takes the C's integer values, and the C picks
//! which rules a build runs (the masks only on 32-bit Windows).
//!
//! Characterized, not endorsed (`test_win_crash_report_steps`), and kept:
//! a module whose end (`base + size`) wraps past 2^64, or lands on it,
//! holds nothing; the stack and PC masks keep 36 bits where the parameters
//! keep 32 (no effect on 32-bit addresses); a thread with the crashed id
//! under another process is left out of both passes.
//!
//! No intentional differences from the C.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

/// `dbghelp.h`'s `SYMFLAG_EXPORT`.
pub const SYMFLAG_EXPORT: u32 = 0x200;

/// A 32-bit build's mask on the stack and PC: `0xFFFFFFFFF`, nine Fs.
const ADDRESS_MASK: u64 = 0xF_FFFF_FFFF;

/// A 32-bit build's mask on the parameters: `0xFFFFFFFF`.
const PARAM_MASK: u64 = 0xFFFF_FFFF;

/// Whether a module of this base and size holds `addr`: `base <= addr <
/// base + size`, the end in `u64` as `DWORD64` computes it, so a module
/// whose end wraps past 2^64 holds nothing.
pub fn contains(addr: u64, base: u64, size: u32) -> bool {
    addr >= base && addr < base.wrapping_add(u64::from(size))
}

/// Whether a thread is the crashed one: its id is the current thread's.
pub fn is_crashed(thread_id: u32, current_thread_id: u32) -> bool {
    thread_id == current_thread_id
}

/// Whether a thread is traced in this pass: the crashed thread in the first
/// pass, every other thread in the second.
pub fn in_pass(first_pass: bool, crashed: bool) -> bool {
    first_pass == crashed
}

/// Whether a thread belongs to another process, and so is skipped.
pub fn other_process(owner_process_id: u32, current_process_id: u32) -> bool {
    owner_process_id != current_process_id
}

/// Whether a resolved symbol is written by name: it is not an export. The
/// caller checks the lookup's success first, and reads the flags only then.
pub fn names_symbol(flags: u32) -> bool {
    flags & SYMFLAG_EXPORT == 0
}

/// A 32-bit build's mask on a frame's stack and PC addresses: the low 36
/// bits.
pub fn mask_address(value: u64) -> u64 {
    value & ADDRESS_MASK
}

/// A 32-bit build's mask on each of a frame's four parameters: the low 32
/// bits.
pub fn mask_param(value: u64) -> u64 {
    value & PARAM_MASK
}

/// Whether a thread name of this length gets a leading space: it is not
/// empty.
pub fn name_prefix(len: usize) -> bool {
    len != 0
}

/// Whether the filter passes the exception on untouched: a debugger is
/// present, or the filter is already running. `inside` reads the re-entry
/// flag, and is called only when no debugger is present.
pub fn pass_on(debugger: bool, inside: impl FnOnce() -> bool) -> bool {
    debugger || inside()
}

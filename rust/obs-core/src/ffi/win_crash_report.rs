//! C ABI for `libobs/obs-win-crash-report.c.h`, the rules of
//! `obs-win-crash-handler.c.h`'s crash report, declared in
//! `libobs/obs-win-crash-report.h` without `EXPORT`.
//!
//! `obs-win-crash-handler.c.h` (still C) makes every DbgHelp, toolhelp and
//! thread call, passes the values in and acts on what comes back. These run
//! inside the unhandled exception filter: they take and return integers,
//! read nothing but `*inside` (and that only without a debugger), write
//! nothing, allocate nothing, keep no state, take no lock and call nothing
//! in libobs, and none can panic. All nine stay hidden (`rust-hidden.txt`).
//!
//! No intentional differences from C.

use crate::win_crash_report as report;

/// `bool obs_win_crash_report_contains(uint64_t addr, uint64_t base,
/// uint32_t size)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_win_crash_report_contains(addr: u64, base: u64, size: u32) -> bool {
    report::contains(addr, base, size)
}

/// `bool obs_win_crash_report_is_crashed(uint32_t thread_id, uint32_t
/// current_thread_id)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_win_crash_report_is_crashed(thread_id: u32, current_thread_id: u32) -> bool {
    report::is_crashed(thread_id, current_thread_id)
}

/// `bool obs_win_crash_report_in_pass(bool first_pass, bool crashed)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_win_crash_report_in_pass(first_pass: bool, crashed: bool) -> bool {
    report::in_pass(first_pass, crashed)
}

/// `bool obs_win_crash_report_other_process(uint32_t owner_process_id,
/// uint32_t current_process_id)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_win_crash_report_other_process(
    owner_process_id: u32,
    current_process_id: u32,
) -> bool {
    report::other_process(owner_process_id, current_process_id)
}

/// `bool obs_win_crash_report_names_symbol(uint32_t flags)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_win_crash_report_names_symbol(flags: u32) -> bool {
    report::names_symbol(flags)
}

/// `uint64_t obs_win_crash_report_mask_address(uint64_t value)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_win_crash_report_mask_address(value: u64) -> u64 {
    report::mask_address(value)
}

/// `uint64_t obs_win_crash_report_mask_param(uint64_t value)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_win_crash_report_mask_param(value: u64) -> u64 {
    report::mask_param(value)
}

/// `bool obs_win_crash_report_name_prefix(size_t len)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_win_crash_report_name_prefix(len: usize) -> bool {
    report::name_prefix(len)
}

/// `bool obs_win_crash_report_pass_on(bool debugger, const bool *inside)`.
///
/// # Safety
///
/// When `debugger` is false, `inside` must point to a readable `bool`. With
/// a debugger it is never read, and may be anything.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_win_crash_report_pass_on(debugger: bool, inside: *const bool) -> bool {
    // SAFETY: called only without a debugger, when the caller guarantees
    // `inside` points to a bool.
    report::pass_on(debugger, || unsafe { *inside })
}

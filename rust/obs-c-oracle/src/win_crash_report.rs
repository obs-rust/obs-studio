//! `oracle_*` build of `libobs/obs-win-crash-report.c.h`
//! (oracle/win_crash_report.c): the crash report rules of
//! `obs-win-crash-handler.c.h`, renamed. The `bool` results are declared as
//! `u8`, the byte the C returns, so a parity test compares it without
//! assuming it is 0 or 1.

unsafe extern "C" {
    /// `bool obs_win_crash_report_contains(uint64_t addr, uint64_t base,
    /// uint32_t size)`.
    pub fn oracle_obs_win_crash_report_contains(addr: u64, base: u64, size: u32) -> u8;
    /// `bool obs_win_crash_report_is_crashed(uint32_t thread_id, uint32_t
    /// current_thread_id)`.
    pub fn oracle_obs_win_crash_report_is_crashed(thread_id: u32, current_thread_id: u32) -> u8;
    /// `bool obs_win_crash_report_in_pass(bool first_pass, bool crashed)`.
    pub fn oracle_obs_win_crash_report_in_pass(first_pass: bool, crashed: bool) -> u8;
    /// `bool obs_win_crash_report_other_process(uint32_t owner_process_id,
    /// uint32_t current_process_id)`.
    pub fn oracle_obs_win_crash_report_other_process(
        owner_process_id: u32,
        current_process_id: u32,
    ) -> u8;
    /// `bool obs_win_crash_report_names_symbol(uint32_t flags)`.
    pub fn oracle_obs_win_crash_report_names_symbol(flags: u32) -> u8;
    /// `uint64_t obs_win_crash_report_mask_address(uint64_t value)`.
    pub fn oracle_obs_win_crash_report_mask_address(value: u64) -> u64;
    /// `uint64_t obs_win_crash_report_mask_param(uint64_t value)`.
    pub fn oracle_obs_win_crash_report_mask_param(value: u64) -> u64;
    /// `bool obs_win_crash_report_name_prefix(size_t len)`.
    pub fn oracle_obs_win_crash_report_name_prefix(len: usize) -> u8;
    /// `bool obs_win_crash_report_pass_on(bool debugger, const bool
    /// *inside)`.
    pub fn oracle_obs_win_crash_report_pass_on(debugger: bool, inside: *const bool) -> u8;
}

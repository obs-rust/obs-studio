//! `util/profiler-io.c`, compiled unmodified as `oracle_*` by
//! `oracle/profiler_io.c`. Its `<zlib.h>` is a pass-through stand-in, so
//! `oracle_profiler_snapshot_dump_csv_gz` writes the CSV uncompressed.

use core::ffi::c_char;

use crate::profiler_snapshot::OracleSnapshot;

unsafe extern "C" {
    pub fn oracle_profiler_print(snap: *mut OracleSnapshot);
    pub fn oracle_profiler_print_time_between_calls(snap: *mut OracleSnapshot);
    pub fn oracle_profiler_snapshot_dump_csv(
        snap: *const OracleSnapshot,
        filename: *const c_char,
    ) -> bool;
    pub fn oracle_profiler_snapshot_dump_csv_gz(
        snap: *const OracleSnapshot,
        filename: *const c_char,
    ) -> bool;
}

/// `struct oracle_profiler_io_log` in `oracle/profiler_io.c`.
#[repr(C)]
#[derive(Debug)]
pub struct OracleLog {
    pub data: *mut c_char,
    pub cap: usize,
    pub len: usize,
    pub levels_not_info: core::ffi::c_int,
    pub overflow: core::ffi::c_int,
}

unsafe extern "C" {
    /// A `log_handler_t` whose param is an [`OracleLog`]: appends each
    /// formatted line and a `'\n'`.
    pub fn oracle_profiler_io_log_capture();
    /// Makes the calling thread's lines the ones the capture keeps (`1`),
    /// or stops (`0`).
    pub fn oracle_profiler_io_log_own(own: core::ffi::c_int);
}

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

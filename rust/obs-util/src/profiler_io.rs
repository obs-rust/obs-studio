//! Safe core of `libobs/util/profiler-io.c`: the profiler's log report
//! (`profiler_print`, `profiler_print_time_between_calls`) and its CSV dump
//! (`profiler_snapshot_dump_csv`, `_csv_gz`).
//!
//! The walk runs over any [`Entry`] tree: the C ABI shim in
//! [`crate::ffi::profiler_io`] reads the snapshot's C structs in place, and
//! tests use [`OwnedEntry`]. Each log line is handed to a callback (the shim
//! sends it to `blog`), and the CSV goes to a byte sink.
//!
//! `%g` and `%p` are the C runtime's (`platform::snprintf_double` /
//! `snprintf_ptr`), so numbers, `nan`/`inf` and pointer columns are spelled
//! exactly as the C spells them on the same host. gzip is `flate2` over
//! miniz_oxide, not zlib: the compressed bytes differ, the CSV inside does
//! not.

#![allow(dead_code, unused_imports, unused_variables)] // RED stubs; the GREEN port uses them

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;

use crate::platform::{DoubleFormat, snprintf_double, snprintf_ptr};

/// `struct profiler_time_entry`: `count` calls that took `time_delta` µs.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TimeEntry {
    pub time_delta: u64,
    pub count: u64,
}

/// One snapshot entry (`struct profiler_snapshot_entry`).
pub trait Entry: Sized {
    /// The entry's address: the CSV `id` / `parent_id` columns.
    fn id(&self) -> usize;
    /// The name, up to its NUL.
    fn name(&self) -> &[u8];
    /// The name pointer: the CSV `name_id` / `parent_name_id` columns.
    fn name_id(&self) -> usize;
    fn times(&self) -> &[TimeEntry];
    fn min_time(&self) -> u64;
    fn max_time(&self) -> u64;
    fn overall_count(&self) -> u64;
    fn times_between_calls(&self) -> &[TimeEntry];
    fn expected_time_between_calls(&self) -> u64;
    fn min_time_between_calls(&self) -> u64;
    fn max_time_between_calls(&self) -> u64;
    fn overall_between_calls_count(&self) -> u64;
    fn children(&self) -> &[Self];
}

/// An [`Entry`] that owns its data; its `id` and `name_id` are the
/// addresses of the entry and of its name bytes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OwnedEntry {
    pub name: Vec<u8>,
    pub times: Vec<TimeEntry>,
    pub min_time: u64,
    pub max_time: u64,
    pub overall_count: u64,
    pub times_between_calls: Vec<TimeEntry>,
    pub expected_time_between_calls: u64,
    pub min_time_between_calls: u64,
    pub max_time_between_calls: u64,
    pub overall_between_calls_count: u64,
    pub children: Vec<OwnedEntry>,
}

impl Entry for OwnedEntry {
    fn id(&self) -> usize {
        core::ptr::from_ref(self).addr()
    }
    fn name(&self) -> &[u8] {
        &self.name
    }
    fn name_id(&self) -> usize {
        self.name.as_ptr().addr()
    }
    fn times(&self) -> &[TimeEntry] {
        &self.times
    }
    fn min_time(&self) -> u64 {
        self.min_time
    }
    fn max_time(&self) -> u64 {
        self.max_time
    }
    fn overall_count(&self) -> u64 {
        self.overall_count
    }
    fn times_between_calls(&self) -> &[TimeEntry] {
        &self.times_between_calls
    }
    fn expected_time_between_calls(&self) -> u64 {
        self.expected_time_between_calls
    }
    fn min_time_between_calls(&self) -> u64 {
        self.min_time_between_calls
    }
    fn max_time_between_calls(&self) -> u64 {
        self.max_time_between_calls
    }
    fn overall_between_calls_count(&self) -> u64 {
        self.overall_between_calls_count
    }
    fn children(&self) -> &[Self] {
        &self.children
    }
}

/// `profiler_print`'s first line.
pub const RESULTS_INTRO: &[u8] = b"== Profiler Results =============================";
/// `profiler_print_time_between_calls`' first line.
pub const TIME_BETWEEN_CALLS_INTRO: &[u8] = b"== Profiler Time Between Calls ==================";
/// The last line of both reports.
pub const FOOTER: &[u8] = b"=================================================";
/// The CSV's first line.
pub const CSV_HEADER: &[u8] =
    "id,parent_id,name_id,parent_name_id,name,time_between_calls,time_delta_\u{b5}s,count\n"
        .as_bytes();

const VPIPE_RIGHT: &[u8] = " \u{2523}".as_bytes();
const VPIPE: &[u8] = " \u{2503}".as_bytes();
const DOWN_RIGHT: &[u8] = " \u{2517}".as_bytes();
/// The `" ms"` after every `%g` time: a no-break space.
const MS: &[u8] = "\u{a0}ms".as_bytes();

/// Which report [`print`] writes (the C's `profile_entry_print_func`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Report {
    /// `profiler_print`: `profile_print_entry`.
    Results,
    /// `profiler_print_time_between_calls`: `profile_print_entry_expected`.
    TimeBetweenCalls,
}

impl Report {
    pub fn intro(self) -> &'static [u8] {
        match self {
            Self::Results => RESULTS_INTRO,
            Self::TimeBetweenCalls => TIME_BETWEEN_CALLS_INTRO,
        }
    }
}

/// `make_indent_string`: one fragment per level; bit `i` of `active` set
/// means level `i` still has siblings below.
pub fn indent_string(indent: u32, active: u64) -> Vec<u8> {
    todo!("profiler-io port")
}

/// What `gather_stats` reports.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stats {
    pub percentile99: u64,
    pub median: u64,
    pub percent_within_bounds: f64,
}

/// `gather_stats`: the 99th percentile and median of `entries` (sorted by
/// descending time), and the share of calls below `expected`.
///
/// As in C, the median is found only on an entry where the running count
/// first reaches half of `calls` and that is not also where it first reaches
/// 1%; one that reaches half only on a later boundary is left at 0.
pub fn gather_stats(expected: u64, entries: &[TimeEntry], calls: u64) -> Stats {
    todo!("profiler-io port")
}

/// What `gather_stats_between` reports: the median, and the percentages of
/// calls within, below and above the bounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BetweenStats {
    pub median: u64,
    pub percent: f64,
    pub lower: f64,
    pub higher: f64,
}

/// `gather_stats_between`, over `entries` sorted by descending time.
pub fn gather_stats_between(
    entries: &[TimeEntry],
    calls: u64,
    lower_bound: u64,
    upper_bound: u64,
    min: u64,
    max: u64,
) -> BetweenStats {
    todo!("profiler-io port")
}

/// `profile_print_entry`: one line for `entry`, then its children.
pub fn print_entry<E: Entry>(
    entry: &E,
    indent: u32,
    active: u64,
    parent_calls: u64,
    log: &mut impl FnMut(&[u8]),
) {
    todo!("profiler-io port")
}

/// `profile_print_entry_expected`: for an entry with an expected time
/// between calls, one line and then its children; nothing otherwise.
pub fn print_entry_expected<E: Entry>(
    entry: &E,
    indent: u32,
    active: u64,
    log: &mut impl FnMut(&[u8]),
) {
    todo!("profiler-io port")
}

/// `profile_print_func` with one of the two printers: the intro, every root,
/// then the footer, each line through `log`.
pub fn print<E: Entry>(roots: &[E], report: Report, log: &mut impl FnMut(&[u8])) {
    todo!("profiler-io port")
}

/// `profiler_snapshot_dump`: the CSV, one chunk per line, into `sink`.
pub fn dump_csv<E: Entry>(roots: &[E], sink: &mut impl FnMut(&[u8])) {
    todo!("profiler-io port")
}

/// `profiler_snapshot_dump_csv`: create or truncate `path` (`"wb+"`) and
/// write the CSV. `false` only when the file cannot be opened; write errors
/// are ignored, as the C ignores `fwrite`'s result.
pub fn dump_csv_file<E: Entry>(roots: &[E], path: &Path) -> bool {
    todo!("profiler-io port")
}

/// `profiler_snapshot_dump_csv_gz`: the CSV, gzip-compressed (level 6, as
/// zlib's `gzopen(path, "wb")`), into a created or truncated `path`.
pub fn dump_csv_gz_file<E: Entry>(roots: &[E], path: &Path) -> bool {
    todo!("profiler-io port")
}

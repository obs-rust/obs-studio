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

/// `%g`.
fn g(out: &mut Vec<u8>, value: f64) {
    out.extend_from_slice(&snprintf_double(DoubleFormat::G, value));
}

/// `%g` of a µs time in ms, then `" ms"`.
fn ms(out: &mut Vec<u8>, time: u64) {
    g(out, time as f64 / 1000.);
    out.extend_from_slice(MS);
}

/// `make_indent_string`: one fragment per level; bit `i` of `active` set
/// means level `i` still has siblings below.
pub fn indent_string(indent: u32, active: u64) -> Vec<u8> {
    let mut out = Vec::new();
    for i in 0..indent {
        let last = i + 1 == indent;
        let fragment: &[u8] = if active & 1u64.wrapping_shl(i) != 0 {
            if last { VPIPE_RIGHT } else { VPIPE }
        } else if last {
            DOWN_RIGHT
        } else {
            b"  "
        };
        out.extend_from_slice(fragment);
    }
    out
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
    let mut stats = Stats {
        percentile99: 0,
        median: 0,
        percent_within_bounds: 0.,
    };
    if entries.is_empty() {
        return stats;
    }

    let calls_f = calls as f64;
    let mut accu: u64 = 0;
    for entry in entries {
        let old_accu = accu;
        accu = accu.wrapping_add(entry.count);

        if (old_accu as f64) < calls_f * 0.01 && accu as f64 >= calls_f * 0.01 {
            stats.percentile99 = entry.time_delta;
        } else if (old_accu as f64) < calls_f * 0.5 && accu as f64 >= calls_f * 0.5 {
            stats.median = entry.time_delta;
            break;
        }
    }

    if expected == 0 {
        return stats;
    }

    accu = 0;
    for entry in entries {
        if entry.time_delta < expected {
            break;
        }
        accu = accu.wrapping_add(entry.count);
    }
    stats.percent_within_bounds = (1. - accu as f64 / calls_f) * 100.;
    stats
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
    let mut stats = BetweenStats {
        median: 0,
        percent: 0.,
        lower: 0.,
        higher: 0.,
    };
    if entries.is_empty() {
        return stats;
    }

    let calls_f = calls as f64;
    let mut accu: u64 = 0;
    for entry in entries {
        accu = accu.wrapping_add(entry.count);
        if (accu as f64) < calls_f * 0.5 {
            continue;
        }
        stats.median = entry.time_delta;
        break;
    }

    let mut found_upper_bound = max <= upper_bound;
    let mut found_lower_bound = false;

    if min >= upper_bound {
        stats.higher = 100.;
        return stats;
    }

    if found_upper_bound && min >= lower_bound {
        stats.percent = 100.;
        return stats;
    }

    accu = 0;
    for entry in entries {
        let delta = entry.time_delta;

        if !found_upper_bound && delta <= upper_bound {
            stats.higher = accu as f64 / calls_f * 100.;
            accu = 0;
            found_upper_bound = true;
        }

        if !found_lower_bound && delta < lower_bound {
            stats.percent = accu as f64 / calls_f * 100.;
            accu = 0;
            found_lower_bound = true;
        }

        accu = accu.wrapping_add(entry.count);
    }

    if !found_upper_bound {
        stats.higher = 100.;
    } else if !found_lower_bound {
        stats.percent = accu as f64 / calls_f * 100.;
    } else {
        stats.lower = accu as f64 / calls_f * 100.;
    }
    stats
}

/// The C's `active &= (1 << indent) - 1`: an `int` shift, sign-extended.
fn clear_from(active: u64, indent: u32) -> u64 {
    active & (1i32.wrapping_shl(indent).wrapping_sub(1) as i64 as u64)
}

/// `lround`, for the finite ratios it sees here.
fn lround(value: f64) -> i64 {
    value.round() as i64
}

/// `profile_print_entry`: one line for `entry`, then its children.
pub fn print_entry<E: Entry>(
    entry: &E,
    indent: u32,
    active: u64,
    parent_calls: u64,
    log: &mut impl FnMut(&[u8]),
) {
    let calls = entry.overall_count();
    let min = entry.min_time();
    let max = entry.max_time();
    let expected = entry.expected_time_between_calls();
    let stats = gather_stats(expected, entry.times(), calls);

    let mut line = indent_string(indent, active);
    line.extend_from_slice(entry.name());
    if min == max {
        line.extend_from_slice(b": ");
        ms(&mut line, min);
    } else {
        line.extend_from_slice(b": min=");
        ms(&mut line, min);
        line.extend_from_slice(b", median=");
        ms(&mut line, stats.median);
        line.extend_from_slice(b", max=");
        ms(&mut line, max);
        line.extend_from_slice(b", 99th percentile=");
        ms(&mut line, stats.percentile99);

        if expected != 0 {
            line.extend_from_slice(b", ");
            g(&mut line, stats.percent_within_bounds);
            line.extend_from_slice(b"% below ");
            ms(&mut line, expected);
        }
    }

    if parent_calls != 0 && calls != parent_calls {
        let calls_per_parent = calls as f64 / parent_calls as f64;
        if lround(calls_per_parent * 10.) != 10 {
            line.extend_from_slice(b", ");
            g(&mut line, calls_per_parent);
            line.extend_from_slice(b" calls per parent call");
        }
    }

    log(&line);

    let mut active = active | 1u64.wrapping_shl(indent);
    let children = entry.children();
    for (i, child) in children.iter().enumerate() {
        if i + 1 == children.len() {
            active = clear_from(active, indent);
        }
        print_entry(child, indent + 1, active, calls, log);
    }
}

/// `profile_print_entry_expected`: for an entry with an expected time
/// between calls, one line and then its children; nothing otherwise.
pub fn print_entry_expected<E: Entry>(
    entry: &E,
    indent: u32,
    active: u64,
    log: &mut impl FnMut(&[u8]),
) {
    let expected = entry.expected_time_between_calls();
    if expected == 0 {
        return;
    }

    let min = entry.min_time_between_calls();
    let max = entry.max_time_between_calls();
    let stats = gather_stats_between(
        entry.times_between_calls(),
        entry.overall_between_calls_count(),
        (expected as f64 * 0.98) as u64,
        (expected as f64 * 1.02 + 0.5) as u64,
        min,
        max,
    );

    let mut line = indent_string(indent, active);
    line.extend_from_slice(entry.name());
    line.extend_from_slice(b": min=");
    ms(&mut line, min);
    line.extend_from_slice(b", median=");
    ms(&mut line, stats.median);
    line.extend_from_slice(b", max=");
    ms(&mut line, max);
    line.extend_from_slice(b", ");
    g(&mut line, stats.percent);
    line.extend_from_slice("% within \u{b1}2% of ".as_bytes());
    ms(&mut line, expected);
    line.extend_from_slice(b" (");
    g(&mut line, stats.lower);
    line.extend_from_slice(b"% lower, ");
    g(&mut line, stats.higher);
    line.extend_from_slice(b"% higher)");

    log(&line);

    let mut active = active | 1u64.wrapping_shl(indent);
    let children = entry.children();
    for (i, child) in children.iter().enumerate() {
        if i + 1 == children.len() {
            active = clear_from(active, indent);
        }
        print_entry_expected(child, indent + 1, active, log);
    }
}

/// `profile_print_func` with one of the two printers: the intro, every root,
/// then the footer, each line through `log`.
pub fn print<E: Entry>(roots: &[E], report: Report, log: &mut impl FnMut(&[u8])) {
    log(report.intro());
    for root in roots {
        match report {
            Report::Results => print_entry(root, 0, 0, 0, log),
            Report::TimeBetweenCalls => print_entry_expected(root, 0, 0, log),
        }
    }
    log(FOOTER);
}

fn csv_row<E: Entry>(
    out: &mut Vec<u8>,
    entry: &E,
    parent: Option<&E>,
    between: u64,
    time: &TimeEntry,
) {
    out.clear();
    out.extend_from_slice(&snprintf_ptr(entry.id()));
    out.push(b',');
    out.extend_from_slice(&snprintf_ptr(parent.map_or(0, Entry::id)));
    out.push(b',');
    out.extend_from_slice(&snprintf_ptr(entry.name_id()));
    out.push(b',');
    out.extend_from_slice(&snprintf_ptr(parent.map_or(0, Entry::name_id)));
    out.push(b',');
    out.extend_from_slice(entry.name());
    out.extend_from_slice(format!(",{between},{},{}\n", time.time_delta, time.count).as_bytes());
}

/// `entry_dump_csv`: the entry's time rows, its time-between-calls rows,
/// then its children's.
fn entry_dump_csv<E: Entry>(
    buffer: &mut Vec<u8>,
    parent: Option<&E>,
    entry: &E,
    sink: &mut impl FnMut(&[u8]),
) {
    for time in entry.times() {
        csv_row(buffer, entry, parent, 0, time);
        sink(buffer);
    }
    let expected = entry.expected_time_between_calls();
    for time in entry.times_between_calls() {
        csv_row(buffer, entry, parent, expected, time);
        sink(buffer);
    }
    for child in entry.children() {
        entry_dump_csv(buffer, Some(entry), child, sink);
    }
}

/// `profiler_snapshot_dump`: the CSV, one chunk per line, into `sink`.
pub fn dump_csv<E: Entry>(roots: &[E], sink: &mut impl FnMut(&[u8])) {
    sink(CSV_HEADER);
    let mut buffer = Vec::new();
    for root in roots {
        entry_dump_csv(&mut buffer, None, root, sink);
    }
}

/// `profiler_snapshot_dump_csv`: create or truncate `path` (`"wb+"`) and
/// write the CSV. `false` only when the file cannot be opened; write errors
/// are ignored, as the C ignores `fwrite`'s result.
pub fn dump_csv_file<E: Entry>(roots: &[E], path: &Path) -> bool {
    let Ok(file) = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
    else {
        return false;
    };
    let mut out = BufWriter::new(file);
    dump_csv(roots, &mut |chunk| {
        let _ = out.write_all(chunk);
    });
    let _ = out.flush();
    true
}

/// `profiler_snapshot_dump_csv_gz`: the CSV, gzip-compressed (level 6, as
/// zlib's `gzopen(path, "wb")`), into a created or truncated `path`.
pub fn dump_csv_gz_file<E: Entry>(roots: &[E], path: &Path) -> bool {
    let Ok(file) = File::create(path) else {
        return false;
    };
    let mut gz = flate2::write::GzEncoder::new(BufWriter::new(file), flate2::Compression::new(6));
    dump_csv(roots, &mut |chunk| {
        let _ = gz.write_all(chunk);
    });
    if let Ok(mut out) = gz.finish() {
        let _ = out.flush();
    }
    true
}

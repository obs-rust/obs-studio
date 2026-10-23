//! Tier 1: `obs-util::profiler_io`, the port of `libobs/util/profiler-io.c`.
//!
//! A 1:1 port of `test/cmocka/test_profiler_io.c` over [`OwnedEntry`] trees
//! (the same snapshot, the same expected lines and bytes), plus the stats
//! helpers' edge cases. The C ABI shim is compared with the C in
//! `profiler_io_parity.rs`.
//!
//! Intentional differences from the C (none reachable from libobs callers):
//! - the `.csv.gz` stream is miniz_oxide's deflate, not zlib's: the
//!   compressed bytes and the gzip header's OS byte differ, the CSV inside
//!   is identical;
//! - a NULL snapshot to the CSV dumps writes the header alone, a NULL file
//!   name returns false, and an entry with a NULL name prints an empty name
//!   (the C dereferences, or passes NULL to `%s`).

use std::io::Read;
use std::path::PathBuf;

use obs_util::platform::snprintf_ptr;
use obs_util::profiler_io::{
    self as io, BetweenStats, CSV_HEADER, Entry, OwnedEntry, Report, Stats, TimeEntry,
};

const MS: &str = "\u{a0}ms";

fn t(v: &[(u64, u64)]) -> Vec<TimeEntry> {
    v.iter()
        .map(|&(time_delta, count)| TimeEntry { time_delta, count })
        .collect()
}

fn entry(name: &str, min: u64, max: u64, calls: u64, times: &[(u64, u64)]) -> OwnedEntry {
    OwnedEntry {
        name: name.as_bytes().to_vec(),
        min_time: min,
        max_time: max,
        overall_count: calls,
        times: t(times),
        ..OwnedEntry::default()
    }
}

/// The snapshot `test_profiler_io.c` builds.
fn snapshot() -> Vec<OwnedEntry> {
    let mut root = entry("root", 1000, 5000, 10, &[(5000, 1), (3000, 4), (1000, 5)]);
    root.expected_time_between_calls = 2000;
    root.min_time_between_calls = 1500;
    root.max_time_between_calls = 2100;
    root.overall_between_calls_count = 9;
    root.times_between_calls = t(&[(2100, 2), (2000, 5), (1900, 1), (1500, 1)]);

    let mut child1 = entry("child1", 500, 1500, 20, &[(1500, 10), (500, 10)]);
    child1.children = vec![entry("leaf", 250, 250, 19, &[(250, 19)])];

    let mut child2 = entry("child2", 100, 100, 15, &[(100, 15)]);
    child2.expected_time_between_calls = 1000;
    child2.min_time_between_calls = 1000;
    child2.max_time_between_calls = 1000;
    child2.overall_between_calls_count = 14;
    child2.times_between_calls = t(&[(1000, 14)]);

    root.children = vec![child1, child2];

    let mut other = entry("other", 7, 7, 1, &[(7, 1)]);
    other.expected_time_between_calls = 5;
    other.min_time_between_calls = 9;
    other.max_time_between_calls = 9;
    other.overall_between_calls_count = 3;
    other.times_between_calls = t(&[(9, 3)]);

    vec![root, other]
}

fn lines<E: Entry>(roots: &[E], report: Report) -> Vec<String> {
    let mut out = Vec::new();
    io::print(roots, report, &mut |l| {
        out.push(String::from_utf8(l.to_vec()).expect("utf-8 line"))
    });
    out
}

#[test]
fn test_profiler_print() {
    let expected = [
        "== Profiler Results =============================".to_owned(),
        format!("root: min=1{MS}, median=3{MS}, max=5{MS}, 99th percentile=5{MS}, 50% below 2{MS}"),
        format!(
            " \u{2523}child1: min=0.5{MS}, median=0{MS}, max=1.5{MS}, 99th percentile=1.5{MS}, 2 calls per parent call"
        ),
        format!(" \u{2503} \u{2517}leaf: 0.25{MS}"),
        format!(" \u{2517}child2: 0.1{MS}, 1.5 calls per parent call"),
        format!("other: 0.007{MS}"),
        "=================================================".to_owned(),
    ];
    assert_eq!(lines(&snapshot(), Report::Results), expected);
}

#[test]
fn test_profiler_print_time_between_calls() {
    let expected = [
        "== Profiler Time Between Calls ==================".to_owned(),
        format!(
            "root: min=1.5{MS}, median=2{MS}, max=2.1{MS}, 55.5556% within \u{b1}2% of 2{MS} (22.2222% lower, 22.2222% higher)"
        ),
        format!(
            " \u{2517}child2: min=1{MS}, median=1{MS}, max=1{MS}, 100% within \u{b1}2% of 1{MS} (0% lower, 0% higher)"
        ),
        format!(
            "other: min=0.009{MS}, median=0.009{MS}, max=0.009{MS}, 0% within \u{b1}2% of 0.005{MS} (0% lower, 100% higher)"
        ),
        "=================================================".to_owned(),
    ];
    assert_eq!(lines(&snapshot(), Report::TimeBetweenCalls), expected);
}

#[test]
fn test_profiler_print_empty_snapshot() {
    let none: [OwnedEntry; 0] = [];
    assert_eq!(
        lines(&none, Report::Results),
        [
            "== Profiler Results =============================",
            "================================================="
        ]
    );
    assert_eq!(
        lines(&none, Report::TimeBetweenCalls),
        [
            "== Profiler Time Between Calls ==================",
            "================================================="
        ]
    );
}

fn ptr(p: usize) -> String {
    String::from_utf8(snprintf_ptr(p)).expect("ascii pointer")
}

fn expected_csv(roots: &[OwnedEntry]) -> Vec<u8> {
    fn rows(out: &mut String, e: &OwnedEntry, parent: Option<&OwnedEntry>) {
        let name = String::from_utf8_lossy(&e.name);
        let head = format!(
            "{},{},{},{},{name}",
            ptr(e.id()),
            ptr(parent.map_or(0, Entry::id)),
            ptr(e.name_id()),
            ptr(parent.map_or(0, Entry::name_id)),
        );
        for x in &e.times {
            out.push_str(&format!("{head},0,{},{}\n", x.time_delta, x.count));
        }
        for x in &e.times_between_calls {
            out.push_str(&format!(
                "{head},{},{},{}\n",
                e.expected_time_between_calls, x.time_delta, x.count
            ));
        }
        for c in &e.children {
            rows(out, c, Some(e));
        }
    }
    let mut out = String::from_utf8(CSV_HEADER.to_vec()).unwrap();
    for r in roots {
        rows(&mut out, r, None);
    }
    out.into_bytes()
}

fn csv_bytes(roots: &[OwnedEntry]) -> Vec<u8> {
    let mut out = Vec::new();
    io::dump_csv(roots, &mut |c| out.extend_from_slice(c));
    out
}

/// A scratch directory under the test's working directory, removed on drop.
struct TmpDir(PathBuf);

impl TmpDir {
    fn new(tag: &str) -> Self {
        let dir = PathBuf::from(format!("profiler_io_{tag}_{}.tmp", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create tmp dir");
        Self(dir)
    }
}

impl Drop for TmpDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn test_profiler_dump_csv() {
    let snap = snapshot();
    let expected = expected_csv(&snap);
    assert_eq!(csv_bytes(&snap), expected);

    // the root rows have a NULL parent and parent name
    let text = String::from_utf8(expected.clone()).unwrap();
    let first_row = text.lines().nth(1).unwrap();
    assert!(first_row.ends_with(",root,0,5000,1"), "{first_row}");
    assert_eq!(text.lines().count(), 1 + 3 + 4 + 2 + 1 + 1 + 1 + 1 + 1);

    let tmp = TmpDir::new("csv");
    let csv = tmp.0.join("out.csv");

    // "wb+": an existing file is truncated
    std::fs::write(&csv, "stale stale stale\n".repeat(64)).unwrap();
    assert!(io::dump_csv_file(&snap, &csv));
    assert_eq!(std::fs::read(&csv).unwrap(), expected);

    // an empty snapshot writes the header alone
    let none: [OwnedEntry; 0] = [];
    assert!(io::dump_csv_file(&none, &csv));
    assert_eq!(std::fs::read(&csv).unwrap(), CSV_HEADER);

    // gz: a gzip stream holding the same CSV
    let gz = tmp.0.join("out.csv.gz");
    std::fs::write(&gz, "stale".repeat(1000)).unwrap();
    assert!(io::dump_csv_gz_file(&snap, &gz));
    let raw = std::fs::read(&gz).unwrap();
    assert_eq!(&raw[..3], &[0x1f, 0x8b, 8]);
    let mut plain = Vec::new();
    flate2::read::GzDecoder::new(&raw[..])
        .read_to_end(&mut plain)
        .unwrap();
    assert_eq!(plain, expected);

    // a missing directory fails
    assert!(!io::dump_csv_file(
        &snap,
        &tmp.0.join("no_such_dir/out.csv")
    ));
    assert!(!io::dump_csv_gz_file(
        &snap,
        &tmp.0.join("no_such_dir/out.csv.gz")
    ));
}

#[test]
fn indent_string_fragments() {
    assert_eq!(io::indent_string(0, 0), b"");
    assert_eq!(io::indent_string(0, u64::MAX), b"");
    assert_eq!(io::indent_string(1, 1), " \u{2523}".as_bytes());
    assert_eq!(io::indent_string(1, 0), " \u{2517}".as_bytes());
    assert_eq!(
        io::indent_string(3, 0b011),
        " \u{2503} \u{2503} \u{2517}".as_bytes()
    );
    assert_eq!(io::indent_string(3, 0b100), "     \u{2523}".as_bytes());
}

#[test]
fn gather_stats_edges() {
    let none = Stats {
        percentile99: 0,
        median: 0,
        percent_within_bounds: 0.,
    };
    assert_eq!(io::gather_stats(10, &[], 5), none);

    // the first entry reaches both 1% and half: only the percentile is set
    let s = io::gather_stats(0, &t(&[(9, 10)]), 10);
    assert_eq!((s.percentile99, s.median), (9, 0));

    // half is reached on a later entry
    let s = io::gather_stats(0, &t(&[(9, 1), (8, 1), (7, 8)]), 10);
    assert_eq!((s.percentile99, s.median), (9, 7));

    // percent below the expected time: entries at or above it are late
    let s = io::gather_stats(8, &t(&[(9, 1), (8, 4), (7, 5)]), 10);
    assert_eq!((s.percentile99, s.median), (9, 8));
    assert_eq!(s.percent_within_bounds, 50.);
    let s = io::gather_stats(100, &t(&[(9, 1), (8, 1), (7, 8)]), 10);
    assert_eq!(s.percent_within_bounds, 100.);

    // zero calls with entries: the C divides by zero
    let s = io::gather_stats(1, &t(&[(9, 1)]), 0);
    assert!(s.percent_within_bounds.is_infinite() && s.percent_within_bounds < 0.);
}

#[test]
fn gather_stats_between_edges() {
    let zero = BetweenStats {
        median: 0,
        percent: 0.,
        lower: 0.,
        higher: 0.,
    };
    assert_eq!(io::gather_stats_between(&[], 4, 98, 102, 1, 200), zero);

    // all above the upper bound
    let s = io::gather_stats_between(&t(&[(200, 4)]), 4, 98, 102, 200, 200);
    assert_eq!((s.median, s.higher, s.percent), (200, 100., 0.));

    // all within
    let s = io::gather_stats_between(&t(&[(101, 2), (99, 2)]), 4, 98, 102, 99, 101);
    assert_eq!((s.median, s.percent), (101, 100.));

    // higher, within and lower
    let s = io::gather_stats_between(&t(&[(150, 1), (100, 2), (50, 1)]), 4, 98, 102, 50, 150);
    assert_eq!((s.higher, s.percent, s.lower), (25., 50., 25.));

    // nothing within the upper bound after the shortcut checks
    let s = io::gather_stats_between(&t(&[(150, 1), (120, 3)]), 4, 98, 102, 50, 150);
    assert_eq!((s.higher, s.percent, s.lower), (100., 0., 0.));

    // within and higher, nothing lower
    let s = io::gather_stats_between(&t(&[(150, 1), (100, 3)]), 4, 98, 102, 100, 150);
    assert_eq!((s.higher, s.percent, s.lower), (25., 75., 0.));
}

#[test]
fn calls_per_parent_rounding() {
    let mut parent = entry("p", 1, 1, 20, &[(1, 20)]);
    // 0.95 rounds to 1.0: hidden; 1.05 rounds to 1.1: shown
    parent.children = vec![
        entry("a", 1, 1, 19, &[(1, 19)]),
        entry("b", 1, 1, 21, &[(1, 21)]),
    ];
    let out = lines(&[parent], Report::Results);
    assert_eq!(out[2], format!(" \u{2523}a: 0.001{MS}"));
    assert_eq!(
        out[3],
        format!(" \u{2517}b: 0.001{MS}, 1.05 calls per parent call")
    );
}

#[test]
fn deep_tree_pipes() {
    // a chain three deep under a root with a later sibling keeps the pipe
    let mut c = entry("c", 1, 1, 1, &[]);
    c.children = vec![entry("d", 1, 1, 1, &[])];
    let mut b = entry("b", 1, 1, 1, &[]);
    b.children = vec![c];
    let mut a = entry("a", 1, 1, 1, &[]);
    a.children = vec![b, entry("e", 1, 1, 1, &[])];
    let out = lines(&[a], Report::Results);
    assert_eq!(out[1], format!("a: 0.001{MS}"));
    assert_eq!(out[2], format!(" \u{2523}b: 0.001{MS}"));
    assert_eq!(out[3], format!(" \u{2503} \u{2517}c: 0.001{MS}"));
    assert_eq!(out[4], format!(" \u{2503}   \u{2517}d: 0.001{MS}"));
    assert_eq!(out[5], format!(" \u{2517}e: 0.001{MS}"));
}

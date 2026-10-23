//! Tier 3: the Rust C ABI shims behave exactly like the original
//! `libobs/util/profiler-io.c`, compiled as an oracle.
//!
//! Each case builds one random snapshot twice, once in each side's layout,
//! and compares every line `profiler_print` and
//! `profiler_print_time_between_calls` log, and the bytes
//! `profiler_snapshot_dump_csv` and `_csv_gz` write. Times and counts mix
//! small values with any `u64` (so zero calls, `nan` and `inf` occur), and
//! names repeat.
//!
//! The CSV's `id` and `parent_id` columns are entry addresses, which differ
//! between the two copies; they are mapped to the entry's walk order before
//! comparing. Name pointers are shared, so `name_id` columns compare as is.
//!
//! The oracle's `<zlib.h>` is a pass-through (`oracle/zlib-shim`), so its
//! `.csv.gz` holds the plain CSV; the Rust file is decompressed first.
//! Intentional differences: the compressed bytes (miniz_oxide vs zlib);
//! NULL snapshot / file name / entry name, which the C dereferences and
//! these cases never pass; and an expected time above [`MAX_EXPECTED`],
//! whose bounds conversion is undefined in C and saturates in Rust.
//!
//! Mutation check (done once for the port): changing `0.98` to `0.97` in
//! `print_entry_expected` makes `print_parity` fail on its first case.

use core::ffi::{CStr, c_char, c_uint, c_void};
use core::ptr;
use std::collections::HashMap;
use std::ffi::CString;
use std::io::Read;

use obs_c_oracle::profiler_io::{self as c, OracleLog};
use obs_c_oracle::profiler_snapshot::{
    OracleDarray, OracleSnapshot, OracleSnapshotEntry, OracleTimeEntry,
    oracle_profile_snapshot_free,
};
use obs_util::ffi::base::base_set_log_handler;
use obs_util::ffi::darray::bmalloc;
use obs_util::ffi::dstr::dstr;
use obs_util::ffi::profiler_io as rs;
use obs_util::ffi::profiler_snapshot::{
    Darray, profile_snapshot_free, profiler_snapshot, profiler_snapshot_entry, profiler_time_entry,
};
use obs_util::platform::snprintf_ptr;
use proptest::prelude::*;

use crate::common::{LOG_HANDLER_LOCK, ScratchDir};

/// The largest expected time the C converts in range: above it,
/// `(uint64_t)(expected * 1.02 + 0.5)` is out of range, undefined in C
/// (x86 gives 2^63 or 0), where Rust's `as` saturates. Excluded on purpose.
const MAX_EXPECTED: u64 = u64::MAX / 103 * 100;

static NAMES: [&CStr; 4] = [c"alpha", c"render", c"tick", c"video_thread"];

#[derive(Debug, Clone)]
struct Spec {
    name: usize,
    times: Vec<(u64, u64)>,
    min_time: u64,
    max_time: u64,
    overall_count: u64,
    between: Vec<(u64, u64)>,
    expected: u64,
    min_between: u64,
    max_between: u64,
    overall_between: u64,
    children: Vec<Spec>,
}

fn value() -> impl Strategy<Value = u64> {
    prop_oneof![4 => 0u64..40_000, 1 => Just(0u64), 1 => any::<u64>()]
}

fn times() -> impl Strategy<Value = Vec<(u64, u64)>> {
    prop::collection::vec((value(), prop_oneof![0u64..50, any::<u64>()]), 0..5).prop_map(|mut v| {
        // the profiler keeps them by descending time
        v.sort_by_key(|t| std::cmp::Reverse(t.0));
        v
    })
}

fn spec(depth: u32) -> BoxedStrategy<Spec> {
    let children = if depth == 0 {
        Just(Vec::new()).boxed()
    } else {
        prop::collection::vec(spec(depth - 1), 0..4).boxed()
    };
    (
        (
            0..NAMES.len(),
            times(),
            value(),
            value(),
            prop_oneof![0u64..100, any::<u64>()],
        ),
        (
            times(),
            prop_oneof![Just(0u64), 1u64..40_000, 0..=MAX_EXPECTED],
            value(),
            value(),
            prop_oneof![0u64..100, any::<u64>()],
        ),
        children,
        any::<bool>(),
    )
        .prop_map(
            |(
                (name, times, a, b, overall_count),
                (between, expected, c, d, overall_between),
                children,
                same,
            )| {
                // min == max takes the short form; make it common
                let (min_time, max_time) = if same { (a, a) } else { (a.min(b), a.max(b)) };
                Spec {
                    name,
                    times,
                    min_time,
                    max_time,
                    overall_count,
                    between,
                    expected,
                    min_between: c.min(d),
                    max_between: c.max(d),
                    overall_between,
                    children,
                }
            },
        )
        .boxed()
}

fn alloc<T>(items: Vec<T>) -> (*mut T, usize) {
    let num = items.len();
    if num == 0 {
        return (ptr::null_mut(), 0);
    }
    // SAFETY: bmalloc returns room for `num` items, all written here.
    unsafe {
        let p = bmalloc(num * size_of::<T>()).cast::<T>();
        for (i, item) in items.into_iter().enumerate() {
            p.add(i).write(item);
        }
        (p, num)
    }
}

fn rs_darray<T>(items: Vec<T>) -> Darray<T> {
    let (array, num) = alloc(items);
    Darray {
        array,
        num,
        capacity: num,
    }
}

fn c_darray<T>(items: Vec<T>) -> OracleDarray<T> {
    let (array, num) = alloc(items);
    OracleDarray {
        array,
        num,
        capacity: num,
    }
}

fn rs_entry(s: &Spec) -> profiler_snapshot_entry {
    let t = |v: &[(u64, u64)]| {
        v.iter()
            .map(|&(time_delta, count)| profiler_time_entry { time_delta, count })
            .collect()
    };
    profiler_snapshot_entry {
        name: NAMES[s.name].as_ptr(),
        times: rs_darray(t(&s.times)),
        min_time: s.min_time,
        max_time: s.max_time,
        overall_count: s.overall_count,
        times_between_calls: rs_darray(t(&s.between)),
        expected_time_between_calls: s.expected,
        min_time_between_calls: s.min_between,
        max_time_between_calls: s.max_between,
        overall_between_calls_count: s.overall_between,
        children: rs_darray(s.children.iter().map(rs_entry).collect()),
    }
}

fn c_entry(s: &Spec) -> OracleSnapshotEntry {
    let t = |v: &[(u64, u64)]| {
        v.iter()
            .map(|&(time_delta, count)| OracleTimeEntry { time_delta, count })
            .collect()
    };
    OracleSnapshotEntry {
        name: NAMES[s.name].as_ptr(),
        times: c_darray(t(&s.times)),
        min_time: s.min_time,
        max_time: s.max_time,
        overall_count: s.overall_count,
        times_between_calls: c_darray(t(&s.between)),
        expected_time_between_calls: s.expected,
        min_time_between_calls: s.min_between,
        max_time_between_calls: s.max_between,
        overall_between_calls_count: s.overall_between,
        children: c_darray(s.children.iter().map(c_entry).collect()),
    }
}

/// Both copies of the snapshot, freed on drop.
struct Snapshots {
    rs: *mut profiler_snapshot,
    c: *mut OracleSnapshot,
}

impl Snapshots {
    fn new(roots: &[Spec]) -> Self {
        // SAFETY: bmalloc returns room for one snapshot each.
        unsafe {
            let r = bmalloc(size_of::<profiler_snapshot>()).cast::<profiler_snapshot>();
            r.write(profiler_snapshot {
                roots: rs_darray(roots.iter().map(rs_entry).collect()),
            });
            let o = bmalloc(size_of::<OracleSnapshot>()).cast::<OracleSnapshot>();
            o.write(OracleSnapshot {
                roots: c_darray(roots.iter().map(c_entry).collect()),
            });
            Self { rs: r, c: o }
        }
    }

    /// Every entry address of one side, in walk order, as `%p` spells it.
    fn ids(&self, rust: bool) -> HashMap<Vec<u8>, usize> {
        fn walk<T>(
            out: &mut HashMap<Vec<u8>, usize>,
            arr: *mut T,
            num: usize,
            kids: &impl Fn(&T) -> (*mut T, usize),
        ) {
            for i in 0..num {
                // SAFETY: `arr` holds `num` entries built by `Snapshots::new`.
                let e = unsafe { &*arr.add(i) };
                let next = out.len();
                out.insert(snprintf_ptr(ptr::from_ref(e).addr()), next);
                let (a, n) = kids(e);
                walk(out, a, n, kids);
            }
        }
        let mut out = HashMap::new();
        out.insert(snprintf_ptr(0), usize::MAX);
        // SAFETY: both snapshots are valid until drop.
        unsafe {
            if rust {
                let roots = &(*self.rs).roots;
                walk(
                    &mut out,
                    roots.array,
                    roots.num,
                    &|e: &profiler_snapshot_entry| (e.children.array, e.children.num),
                );
            } else {
                let roots = &(*self.c).roots;
                walk(
                    &mut out,
                    roots.array,
                    roots.num,
                    &|e: &OracleSnapshotEntry| (e.children.array, e.children.num),
                );
            }
        }
        out
    }
}

impl Drop for Snapshots {
    fn drop(&mut self) {
        // SAFETY: each snapshot is freed once, by its own side's free.
        unsafe {
            profile_snapshot_free(self.rs);
            oracle_profile_snapshot_free(self.c);
        }
    }
}

/// Runs `f` with the log handler capturing into a buffer; the lines it got.
fn capture(f: impl FnOnce()) -> Vec<u8> {
    let mut data = vec![0u8; 1 << 20];
    let mut log = OracleLog {
        data: data.as_mut_ptr().cast::<c_char>(),
        cap: data.len(),
        len: 0,
        levels_not_info: 0,
        overflow: 0,
    };
    // SAFETY: the handler matches `log_handler_t` and writes only into
    // `log`, which outlives the capture; the default is restored after.
    unsafe {
        c::oracle_profiler_io_log_own(1);
        base_set_log_handler(
            c::oracle_profiler_io_log_capture as *mut c_void,
            (&raw mut log).cast(),
        );
    }
    f();
    // SAFETY: NULL restores the default handler.
    unsafe {
        base_set_log_handler(ptr::null_mut(), ptr::null_mut());
        c::oracle_profiler_io_log_own(0);
    }
    assert_eq!(log.overflow, 0);
    assert_eq!(log.levels_not_info, 0);
    data.truncate(log.len);
    data
}

/// The CSV with each id / parent_id column replaced by its walk index.
fn normalize(csv: &[u8], ids: &HashMap<Vec<u8>, usize>) -> Vec<Vec<u8>> {
    let mut lines = csv.split(|&b| b == b'\n');
    let mut out = vec![lines.next().unwrap().to_vec()];
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let mut fields = line.splitn(3, |&b| b == b',');
        let id = ids[fields.next().unwrap()];
        let parent = ids[fields.next().unwrap()];
        let mut row = format!("{id},{parent},").into_bytes();
        row.extend_from_slice(fields.next().unwrap());
        out.push(row);
    }
    out
}

fn roots_strategy() -> impl Strategy<Value = Vec<Spec>> {
    prop::collection::vec(spec(3), 0..4)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn print_parity(roots in roots_strategy()) {
        let _guard = LOG_HANDLER_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let snaps = Snapshots::new(&roots);
        // SAFETY: both snapshots are valid for the calls.
        unsafe {
            let r = capture(|| rs::profiler_print(snaps.rs));
            let o = capture(|| c::oracle_profiler_print(snaps.c));
            prop_assert_eq!(String::from_utf8_lossy(&r), String::from_utf8_lossy(&o));
            let r = capture(|| rs::profiler_print_time_between_calls(snaps.rs));
            let o = capture(|| c::oracle_profiler_print_time_between_calls(snaps.c));
            prop_assert_eq!(String::from_utf8_lossy(&r), String::from_utf8_lossy(&o));
        }
    }

    #[test]
    fn csv_parity(roots in roots_strategy()) {
        let snaps = Snapshots::new(&roots);
        let dir = ScratchDir::new("profiler-io-parity");
        let file = |name: &str| CString::new(dir.path().join(name).to_str().unwrap()).unwrap();
        let (rs_csv, c_csv, rs_gz, c_gz) =
            (file("rs.csv"), file("c.csv"), file("rs.csv.gz"), file("c.csv.gz"));
        // SAFETY: both snapshots are valid; the names are C strings.
        unsafe {
            prop_assert!(rs::profiler_snapshot_dump_csv(snaps.rs, rs_csv.as_ptr()));
            prop_assert!(c::oracle_profiler_snapshot_dump_csv(snaps.c, c_csv.as_ptr()));
            prop_assert!(rs::profiler_snapshot_dump_csv_gz(snaps.rs, rs_gz.as_ptr()));
            prop_assert!(c::oracle_profiler_snapshot_dump_csv_gz(snaps.c, c_gz.as_ptr()));
        }
        let read = |p: &CString| std::fs::read(p.to_str().unwrap()).unwrap();
        let (rs_ids, c_ids) = (snaps.ids(true), snaps.ids(false));

        let rust = normalize(&read(&rs_csv), &rs_ids);
        prop_assert_eq!(&rust, &normalize(&read(&c_csv), &c_ids));

        let mut plain = Vec::new();
        flate2::read::GzDecoder::new(&read(&rs_gz)[..]).read_to_end(&mut plain).unwrap();
        prop_assert_eq!(&normalize(&plain, &rs_ids), &normalize(&read(&c_gz), &c_ids));
        prop_assert_eq!(normalize(&plain, &rs_ids), rust);
    }
}

#[test]
fn csv_failure_parity() {
    let snaps = Snapshots::new(&[]);
    let dir = ScratchDir::new("profiler-io-fail");
    let missing = CString::new(dir.path().join("no_such_dir/x.csv").to_str().unwrap()).unwrap();
    // SAFETY: valid snapshots and C strings.
    unsafe {
        assert_eq!(
            rs::profiler_snapshot_dump_csv(snaps.rs, missing.as_ptr()),
            c::oracle_profiler_snapshot_dump_csv(snaps.c, missing.as_ptr())
        );
        assert_eq!(
            rs::profiler_snapshot_dump_csv_gz(snaps.rs, missing.as_ptr()),
            c::oracle_profiler_snapshot_dump_csv_gz(snaps.c, missing.as_ptr())
        );
        assert!(!rs::profiler_snapshot_dump_csv(snaps.rs, missing.as_ptr()));
    }
}

/// What `record_print` saw: entry, indent, active, parent calls, buffers.
type PrintCall = (usize, c_uint, u64, u64, bool);
static SEEN: std::sync::Mutex<Vec<PrintCall>> = std::sync::Mutex::new(Vec::new());

unsafe extern "C" fn record_print(
    entry: *mut profiler_snapshot_entry,
    indent_buffer: *mut dstr,
    output_buffer: *mut dstr,
    indent: c_uint,
    active: u64,
    parent_calls: u64,
) {
    let buffers_ok = !indent_buffer.is_null() && !output_buffer.is_null();
    SEEN.lock()
        .unwrap()
        .push((entry.addr(), indent, active, parent_calls, buffers_ok));
}

/// `profile_print_func` (hidden, no C caller): the intro, the printer once
/// per root with zero indent, active and parent calls, then the footer.
#[test]
fn profile_print_func_calls_printer_per_root() {
    let _guard = LOG_HANDLER_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let spec = Spec {
        name: 0,
        times: vec![],
        min_time: 1,
        max_time: 1,
        overall_count: 1,
        between: vec![],
        expected: 0,
        min_between: 0,
        max_between: 0,
        overall_between: 0,
        children: vec![],
    };
    let snaps = Snapshots::new(&[spec.clone(), spec]);
    SEEN.lock().unwrap().clear();
    // SAFETY: a valid snapshot, C string intro and printer.
    let lines = capture(|| unsafe {
        rs::profile_print_func(c"== intro ==".as_ptr(), Some(record_print), snaps.rs)
    });
    assert_eq!(
        String::from_utf8(lines).unwrap(),
        "== intro ==\n=================================================\n"
    );
    // SAFETY: the snapshot has two roots.
    let roots = unsafe { (*snaps.rs).roots.array };
    let seen = SEEN.lock().unwrap().clone();
    // SAFETY: as above.
    let expected = unsafe { [roots.addr(), roots.add(1).addr()] };
    assert_eq!(seen, expected.map(|a| (a, 0, 0, 0, true)).to_vec(),);
}

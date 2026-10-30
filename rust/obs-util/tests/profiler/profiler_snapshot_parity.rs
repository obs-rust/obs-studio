//! Tier 3: the Rust C ABI shims behave exactly like the original
//! `libobs/util/profiler-snapshot.c`, compiled as an oracle.
//!
//! Each case builds the same random snapshot twice in `bmalloc` memory, the
//! way `profiler.c` does, once for each side. It then compares enumeration
//! (with early stops), every getter, `profiler_snapshot_filter_roots` driven
//! by a scripted series of callback answers, and finally frees both.
//!
//! Intentional differences (not exercised here, C would crash): a NULL
//! `snap` to `profiler_snapshot_filter_roots`, and a NULL callback with
//! entries to visit, do nothing in Rust.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use core::ffi::{CStr, c_char, c_void};
use core::mem::size_of;
use core::ptr;

use obs_c_oracle::profiler_snapshot::{
    self as c, OracleDarray, OracleSnapshot, OracleSnapshotEntry, OracleTimeEntry,
};
use obs_util::ffi::darray::bmalloc;
use obs_util::ffi::profiler_snapshot::{
    self as rs, Darray, profiler_snapshot, profiler_snapshot_entry, profiler_time_entry,
};
use proptest::prelude::*;

/// Names with stable addresses; entries store these pointers, as
/// `profiler.c` stores the caller's name pointers.
static NAMES: [&CStr; 6] = [c"alpha", c"beta", c"gamma", c"delta", c"render", c"tick"];

#[derive(Debug, Clone)]
struct Spec {
    name: usize,
    times: Vec<(u64, u64)>,
    times_extra_cap: usize,
    min_time: u64,
    max_time: u64,
    overall_count: u64,
    between: Vec<(u64, u64)>,
    expected: u64,
    min_between: u64,
    max_between: u64,
    overall_between: u64,
    children: Vec<Spec>,
    children_extra_cap: usize,
}

fn times() -> impl Strategy<Value = Vec<(u64, u64)>> {
    prop::collection::vec((any::<u64>(), any::<u64>()), 0..4)
}

fn spec(depth: u32) -> BoxedStrategy<Spec> {
    let children = if depth == 0 {
        Just(Vec::new()).boxed()
    } else {
        prop::collection::vec(spec(depth - 1), 0..4).boxed()
    };
    (
        (0..NAMES.len(), times(), 0usize..3, any::<[u64; 3]>()),
        (times(), any::<[u64; 4]>()),
        (children, 0usize..3),
    )
        .prop_map(
            |((name, times, times_extra_cap, a), (between, b), (children, children_extra_cap))| {
                Spec {
                    name,
                    times,
                    times_extra_cap,
                    min_time: a[0],
                    max_time: a[1],
                    overall_count: a[2],
                    between,
                    expected: b[0],
                    min_between: b[1],
                    max_between: b[2],
                    overall_between: b[3],
                    children,
                    children_extra_cap,
                }
            },
        )
        .boxed()
}

/// A `bmalloc`ed copy of `items` with `extra` spare capacity, laid out as
/// `DARRAY(T)` expects (`array`, `num`, `capacity`). Empty with no spare
/// capacity is the zeroed darray, as `da_init` leaves it.
fn alloc_array<T>(items: Vec<T>, extra: usize) -> (*mut T, usize, usize) {
    let num = items.len();
    let capacity = num + extra;
    if capacity == 0 {
        return (ptr::null_mut(), 0, 0);
    }
    // SAFETY: bmalloc returns `capacity` items' worth of writable memory,
    // and only the first `num` are written and later read.
    unsafe {
        let p = bmalloc(capacity * size_of::<T>()).cast::<T>();
        for (i, item) in items.into_iter().enumerate() {
            p.add(i).write(item);
        }
        (p, num, capacity)
    }
}

fn rs_darray<T>(items: Vec<T>, extra: usize) -> Darray<T> {
    let (array, num, capacity) = alloc_array(items, extra);
    Darray {
        array,
        num,
        capacity,
    }
}

fn c_darray<T>(items: Vec<T>, extra: usize) -> OracleDarray<T> {
    let (array, num, capacity) = alloc_array(items, extra);
    OracleDarray {
        array,
        num,
        capacity,
    }
}

fn rs_entry(s: &Spec) -> profiler_snapshot_entry {
    let t = |v: &[(u64, u64)]| {
        v.iter()
            .map(|&(time_delta, count)| profiler_time_entry { time_delta, count })
            .collect::<Vec<_>>()
    };
    profiler_snapshot_entry {
        name: NAMES[s.name].as_ptr(),
        times: rs_darray(t(&s.times), s.times_extra_cap),
        min_time: s.min_time,
        max_time: s.max_time,
        overall_count: s.overall_count,
        times_between_calls: rs_darray(t(&s.between), 0),
        expected_time_between_calls: s.expected,
        min_time_between_calls: s.min_between,
        max_time_between_calls: s.max_between,
        overall_between_calls_count: s.overall_between,
        children: rs_darray(
            s.children.iter().map(rs_entry).collect(),
            s.children_extra_cap,
        ),
    }
}

fn c_entry(s: &Spec) -> OracleSnapshotEntry {
    let t = |v: &[(u64, u64)]| {
        v.iter()
            .map(|&(time_delta, count)| OracleTimeEntry { time_delta, count })
            .collect::<Vec<_>>()
    };
    OracleSnapshotEntry {
        name: NAMES[s.name].as_ptr(),
        times: c_darray(t(&s.times), s.times_extra_cap),
        min_time: s.min_time,
        max_time: s.max_time,
        overall_count: s.overall_count,
        times_between_calls: c_darray(t(&s.between), 0),
        expected_time_between_calls: s.expected,
        min_time_between_calls: s.min_between,
        max_time_between_calls: s.max_between,
        overall_between_calls_count: s.overall_between,
        children: c_darray(
            s.children.iter().map(c_entry).collect(),
            s.children_extra_cap,
        ),
    }
}

/// `profile_snapshot_create`'s result: a `bmalloc`ed snapshot.
fn snapshots(roots: &[Spec], extra: usize) -> (*mut profiler_snapshot, *mut OracleSnapshot) {
    // SAFETY: bmalloc returns writable memory for one snapshot each.
    unsafe {
        let r = bmalloc(size_of::<profiler_snapshot>()).cast::<profiler_snapshot>();
        r.write(profiler_snapshot {
            roots: rs_darray(roots.iter().map(rs_entry).collect(), extra),
        });
        let o = bmalloc(size_of::<OracleSnapshot>()).cast::<OracleSnapshot>();
        o.write(OracleSnapshot {
            roots: c_darray(roots.iter().map(c_entry).collect(), extra),
        });
        (r, o)
    }
}

/// Records each entry an enumeration callback sees, and answers false after
/// `stop_after` calls.
struct Visits {
    stop_after: usize,
    seen: Vec<usize>,
    entries: Vec<*mut c_void>,
}

fn name_index(name: *const c_char) -> usize {
    NAMES
        .iter()
        .position(|n| ptr::eq(n.as_ptr(), name))
        .expect("entry name is one of NAMES")
}

unsafe extern "C" fn visit_rs(ctx: *mut c_void, e: *mut profiler_snapshot_entry) -> bool {
    // SAFETY: the test passes a live `Visits` and the shim a live entry.
    let (v, e) = unsafe { (&mut *ctx.cast::<Visits>(), &*e) };
    v.seen.push(name_index(e.name));
    v.entries.push(ptr::from_ref(e).cast_mut().cast());
    v.seen.len() < v.stop_after
}

unsafe extern "C" fn visit_c(ctx: *mut c_void, e: *mut OracleSnapshotEntry) -> bool {
    // SAFETY: the test passes a live `Visits` and the oracle a live entry.
    let (v, e) = unsafe { (&mut *ctx.cast::<Visits>(), &*e) };
    v.seen.push(name_index(e.name));
    v.entries.push(ptr::from_ref(e).cast_mut().cast());
    v.seen.len() < v.stop_after
}

/// Answers each filter call from a script: (keep going, remove). A `None`
/// remove leaves `*remove` untouched, as a callback may, so the default the
/// caller initialized it to decides.
struct Script {
    answers: Vec<(bool, Option<bool>)>,
    seen: Vec<usize>,
}

unsafe extern "C" fn filter_cb(data: *mut c_void, name: *const c_char, remove: *mut bool) -> bool {
    // SAFETY: the test passes a live `Script`; `remove` is the callee's local.
    let s = unsafe { &mut *data.cast::<Script>() };
    let (keep_going, rm) = s.answers.get(s.seen.len()).copied().unwrap_or((true, None));
    s.seen.push(name_index(name));
    if let Some(rm) = rm {
        // SAFETY: `remove` points at a live bool.
        unsafe { *remove = rm };
    }
    keep_going
}

fn times_of<T: Copy>(array: *const T, num: usize) -> Vec<T> {
    (0..num)
        // SAFETY: the darray holds `num` initialized items.
        .map(|i| unsafe { *array.add(i) })
        .collect()
}

/// Every getter and the children of `r` (shim side) and `o` (oracle side),
/// recursively, with an early stop at `stop_after` children.
fn compare_entry(
    r: *mut profiler_snapshot_entry,
    o: *mut OracleSnapshotEntry,
    stop_after: usize,
) -> Result<(), TestCaseError> {
    // SAFETY: both pointers are live entries of the two snapshots.
    unsafe {
        prop_assert_eq!(
            name_index(rs::profiler_snapshot_entry_name(r)),
            name_index(c::oracle_profiler_snapshot_entry_name(o))
        );
        let pairs = [
            (
                rs::profiler_snapshot_entry_overall_count(r),
                c::oracle_profiler_snapshot_entry_overall_count(o),
            ),
            (
                rs::profiler_snapshot_entry_min_time(r),
                c::oracle_profiler_snapshot_entry_min_time(o),
            ),
            (
                rs::profiler_snapshot_entry_max_time(r),
                c::oracle_profiler_snapshot_entry_max_time(o),
            ),
            (
                rs::profiler_snapshot_entry_expected_time_between_calls(r),
                c::oracle_profiler_snapshot_entry_expected_time_between_calls(o),
            ),
            (
                rs::profiler_snapshot_entry_min_time_between_calls(r),
                c::oracle_profiler_snapshot_entry_min_time_between_calls(o),
            ),
            (
                rs::profiler_snapshot_entry_max_time_between_calls(r),
                c::oracle_profiler_snapshot_entry_max_time_between_calls(o),
            ),
            (
                rs::profiler_snapshot_entry_overall_between_calls_count(r),
                c::oracle_profiler_snapshot_entry_overall_between_calls_count(o),
            ),
        ];
        for (a, b) in pairs {
            prop_assert_eq!(a, b);
        }

        // The times getters return the entry's own darray.
        let rt = rs::profiler_snapshot_entry_times(r);
        let ct = c::oracle_profiler_snapshot_entry_times(o);
        prop_assert!(ptr::eq(rt, &(*r).times));
        prop_assert!(ptr::eq(ct, &(*o).times));
        prop_assert_eq!(
            times_of((*rt).array, (*rt).num)
                .iter()
                .map(|t| (t.time_delta, t.count))
                .collect::<Vec<_>>(),
            times_of((*ct).array, (*ct).num)
                .iter()
                .map(|t| (t.time_delta, t.count))
                .collect::<Vec<_>>()
        );
        let rb = rs::profiler_snapshot_entry_times_between_calls(r);
        let cb = c::oracle_profiler_snapshot_entry_times_between_calls(o);
        prop_assert!(ptr::eq(rb, &(*r).times_between_calls));
        prop_assert!(ptr::eq(cb, &(*o).times_between_calls));
        prop_assert_eq!((*rb).num, (*cb).num);

        prop_assert_eq!(
            rs::profiler_snapshot_num_children(r),
            c::oracle_profiler_snapshot_num_children(o)
        );

        let mut rv = Visits {
            stop_after,
            seen: Vec::new(),
            entries: Vec::new(),
        };
        let mut cv = Visits {
            stop_after,
            seen: Vec::new(),
            entries: Vec::new(),
        };
        rs::profiler_snapshot_enumerate_children(r, Some(visit_rs), ptr::from_mut(&mut rv).cast());
        c::oracle_profiler_snapshot_enumerate_children(
            o,
            Some(visit_c),
            ptr::from_mut(&mut cv).cast(),
        );
        prop_assert_eq!(&rv.seen, &cv.seen);
        for (re, ce) in rv.entries.iter().zip(&cv.entries) {
            compare_entry(re.cast(), ce.cast(), stop_after)?;
        }
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(512)))]

    #[test]
    fn snapshot_access_matches_c_oracle(
        roots in prop::collection::vec(spec(2), 0..6),
        extra in 0usize..3,
        stop_after in 1usize..5,
        answers in prop::collection::vec((any::<bool>(), prop::option::of(any::<bool>())), 0..8),
    ) {
        let (r, o) = snapshots(&roots, extra);
        // SAFETY: `r` and `o` are live snapshots built above; callbacks get
        // live contexts; each snapshot is freed exactly once at the end.
        unsafe {
            prop_assert_eq!(
                rs::profiler_snapshot_num_roots(r),
                c::oracle_profiler_snapshot_num_roots(o)
            );

            let mut rv = Visits { stop_after, seen: Vec::new(), entries: Vec::new() };
            let mut cv = Visits { stop_after, seen: Vec::new(), entries: Vec::new() };
            rs::profiler_snapshot_enumerate_roots(r, Some(visit_rs), ptr::from_mut(&mut rv).cast());
            c::oracle_profiler_snapshot_enumerate_roots(o, Some(visit_c), ptr::from_mut(&mut cv).cast());
            prop_assert_eq!(&rv.seen, &cv.seen);
            for (re, ce) in rv.entries.iter().zip(&cv.entries) {
                compare_entry(re.cast(), ce.cast(), stop_after)?;
            }

            let mut rs_script = Script { answers: answers.clone(), seen: Vec::new() };
            let mut c_script = Script { answers, seen: Vec::new() };
            rs::profiler_snapshot_filter_roots(r, Some(filter_cb), ptr::from_mut(&mut rs_script).cast());
            c::oracle_profiler_snapshot_filter_roots(o, Some(filter_cb), ptr::from_mut(&mut c_script).cast());
            prop_assert_eq!(&rs_script.seen, &c_script.seen);

            // What is left: same roots, same order; da_erase keeps capacity.
            prop_assert_eq!((*r).roots.num, (*o).roots.num);
            prop_assert_eq!((*r).roots.capacity, (*o).roots.capacity);
            let mut rv = Visits { stop_after: usize::MAX, seen: Vec::new(), entries: Vec::new() };
            let mut cv = Visits { stop_after: usize::MAX, seen: Vec::new(), entries: Vec::new() };
            rs::profiler_snapshot_enumerate_roots(r, Some(visit_rs), ptr::from_mut(&mut rv).cast());
            c::oracle_profiler_snapshot_enumerate_roots(o, Some(visit_c), ptr::from_mut(&mut cv).cast());
            prop_assert_eq!(&rv.seen, &cv.seen);
            for (re, ce) in rv.entries.iter().zip(&cv.entries) {
                compare_entry(re.cast(), ce.cast(), usize::MAX)?;
            }

            rs::profile_snapshot_free(r);
            c::oracle_profile_snapshot_free(o);
        }
    }
}

/// NULL snapshots and entries read as empty on both sides.
#[test]
fn null_inputs_match_c_oracle() {
    let mut v = Visits {
        stop_after: usize::MAX,
        seen: Vec::new(),
        entries: Vec::new(),
    };
    let ctx = ptr::from_mut(&mut v).cast();
    // SAFETY: every accessor accepts NULL; the callbacks are never called.
    unsafe {
        assert_eq!(rs::profiler_snapshot_num_roots(ptr::null_mut()), 0);
        assert_eq!(c::oracle_profiler_snapshot_num_roots(ptr::null_mut()), 0);
        assert_eq!(rs::profiler_snapshot_num_children(ptr::null_mut()), 0);
        assert_eq!(c::oracle_profiler_snapshot_num_children(ptr::null_mut()), 0);
        assert!(rs::profiler_snapshot_entry_name(ptr::null_mut()).is_null());
        assert!(c::oracle_profiler_snapshot_entry_name(ptr::null_mut()).is_null());
        assert!(rs::profiler_snapshot_entry_times(ptr::null_mut()).is_null());
        assert!(c::oracle_profiler_snapshot_entry_times(ptr::null_mut()).is_null());
        assert!(rs::profiler_snapshot_entry_times_between_calls(ptr::null_mut()).is_null());
        assert!(c::oracle_profiler_snapshot_entry_times_between_calls(ptr::null_mut()).is_null());
        assert_eq!(
            rs::profiler_snapshot_entry_overall_count(ptr::null_mut()),
            0
        );
        assert_eq!(rs::profiler_snapshot_entry_max_time(ptr::null_mut()), 0);
        rs::profiler_snapshot_enumerate_roots(ptr::null_mut(), Some(visit_rs), ctx);
        c::oracle_profiler_snapshot_enumerate_roots(ptr::null_mut(), Some(visit_c), ctx);
        rs::profiler_snapshot_enumerate_children(ptr::null_mut(), Some(visit_rs), ctx);
        c::oracle_profiler_snapshot_enumerate_children(ptr::null_mut(), Some(visit_c), ctx);
        rs::profile_snapshot_free(ptr::null_mut());
        c::oracle_profile_snapshot_free(ptr::null_mut());
    }
    assert!(v.seen.is_empty());
}

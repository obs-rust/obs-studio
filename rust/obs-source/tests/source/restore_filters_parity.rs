//! Tier 3: the original C `obs_source_restore_filters()` against the Rust
//! export on noise-filled real-header sources, with a stand-in array and
//! stand-in references, loads and releases: the new list, every filter's
//! target and parent, each item read, name read and item release, each
//! reference, load and release in order with the lock state and the list
//! and links at that moment, the NULL log lines, every other byte, the lock
//! afterwards and the item holds.
use core::ffi::{CStr, c_char, c_long, c_void};
use core::ptr::{NonNull, null_mut};
use obs_source::ffi::source_restore_filters::obs_source_restore_filters;
use obs_source::restore_filters::{RestoreList, restore};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Restore = unsafe extern "C" fn(*mut c_void, *mut c_void);
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);

unsafe extern "C" {
    fn oracle_srf_run(
        restore: Option<Restore>,
        get: GetLog,
        set: SetLog,
        present: bool,
        with_array: bool,
        noise: u8,
        count: usize,
        item_count: usize,
        items: u32,
        dying: c_long,
        out: *mut i64,
        message: *mut c_char,
        message_size: usize,
    ) -> bool;
    fn oracle_srf_layout(which: usize) -> usize;
    fn obs_source_restore_filters_lock(source: *mut c_void);
    fn obs_source_restore_filters_count(source: *const c_void) -> usize;
    fn obs_source_restore_filters_at(source: *const c_void, index: usize) -> *mut c_void;
    fn obs_source_restore_filters_set_parent(filter: *mut c_void, parent: *mut c_void);
    fn obs_source_restore_filters_set_target(filter: *mut c_void, target: *mut c_void);
    fn obs_source_restore_filters_free_list(source: *mut c_void);
    fn obs_source_restore_filters_unlock(source: *mut c_void);
    fn obs_source_restore_filters_item_count(array: *mut c_void) -> usize;
    fn obs_source_restore_filters_item(array: *mut c_void, index: usize) -> *mut c_void;
    fn obs_source_restore_filters_item_name(item: *mut c_void) -> *const c_char;
    fn obs_source_restore_filters_name(filter: *const c_void) -> *const c_char;
    fn obs_source_restore_filters_get_ref(filter: *mut c_void) -> *mut c_void;
    fn obs_source_restore_filters_load(item: *mut c_void) -> *mut c_void;
    fn obs_source_restore_filters_load_private(item: *mut c_void) -> *mut c_void;
    fn obs_source_restore_filters_store(
        source: *mut c_void,
        filters: *const *mut c_void,
        count: usize,
    );
    fn obs_source_restore_filters_release_item(item: *mut c_void);
    fn obs_source_restore_filters_release(filter: *mut c_void);
}

/// The current filter slots (`SRF_SLOTS`) and item slots (`SRF_ITEMS`).
const SLOTS: usize = 6;
const ITEMS: usize = 6;
const LIST: usize = SLOTS + ITEMS;
/// The event width (`SRF_EVENT_WIDTH`) and the out array's layout.
const EVENT_WIDTH: usize = 5;
const EVENTS: usize = 96;
const COUNT_AFTER: usize = 0;
const ORDER: usize = 1;
const TARGETS: usize = ORDER + LIST;
const PARENTS: usize = TARGETS + LIST;
const UNCHANGED: usize = PARENTS + LIST;
const UNLOCKED: usize = UNCHANGED + 1;
const LOGS: usize = UNCHANGED + 2;
const HELD: usize = UNCHANGED + 3;
const EVENT_COUNT: usize = UNCHANGED + 4;
const OVERFLOW: usize = UNCHANGED + 5;
const HEAD: usize = UNCHANGED + 6;
const OUT: usize = HEAD + EVENTS * EVENT_WIDTH;
/// Event kinds (`SRF_EV_*`).
const EV_REF: i64 = 5;
const EV_LOAD: i64 = 6;
const EV_RELEASE: i64 = 7;
const EV_LOG: i64 = 8;
/// Source ids (`SRF_NULL`, `SRF_PARENT`, `SRF_UNTOUCHED`, `SRF_LOADED`).
const NULL: c_long = -1;
/// `SRF_NULL` as a recorded id (`c_long` is 32 bits on Windows).
const NULL_ID: i64 = -1;
const PARENT: i64 = -3;
const UNTOUCHED: i64 = -4;
const LOADED: i64 = 100;
/// `LOG_DEBUG`.
const LOG_DEBUG: i64 = 400;
/// The current filters' names (`srf_names`, slot 2 unnamed) and the item
/// names by selector (`srf_item_names`).
const NAMES: [Option<&str>; SLOTS] = [Some("a"), Some("bb"), None, Some("e"), Some("a"), Some("g")];
const ITEM_NAMES: [&str; 7] = ["a", "bb", "e", "g", "zz", "", "x"];

/// What one call left: the out array and the last log line.
type Seen = (Vec<i64>, String);

#[derive(Clone, Copy, Debug)]
struct Case {
    present: bool,
    with_array: bool,
    noise: u8,
    count: usize,
    item_count: usize,
    items: u32,
    dying: c_long,
}

fn run(restore: Option<Restore>, case: Case) -> Seen {
    let mut out: [i64; OUT] = [0; OUT];
    let mut buffer = [0 as c_char; 128];
    // SAFETY: the isolated child owns the log handler. C builds a live
    // fixture, runs the call, records and frees it, and restores logging;
    // both buffers are as long as passed.
    let built = unsafe {
        oracle_srf_run(
            restore,
            base_get_log_handler,
            base_set_log_handler,
            case.present,
            case.with_array,
            case.noise,
            case.count,
            case.item_count,
            case.items,
            case.dying,
            out.as_mut_ptr(),
            buffer.as_mut_ptr(),
            buffer.len(),
        )
    };
    assert!(built, "{case:?}: fixture");
    // SAFETY: the oracle NUL-terminated the buffer.
    let message = unsafe { CStr::from_ptr(buffer.as_ptr()) }
        .to_str()
        .unwrap()
        .to_owned();
    (out.to_vec(), message)
}

/// Both sides alike on `case`; the original's observation.
fn compare_with(restore: Restore, case: Case) -> Option<Seen> {
    let c = run(None, case);
    let rust = run(Some(restore), case);
    (c == rust).then_some(c)
}

/// Each recorded event as (kind, a, b, locked), in order.
fn events(out: &[i64]) -> Vec<(i64, i64, i64, i64)> {
    let count = usize::try_from(out[EVENT_COUNT]).unwrap();
    (0..count)
        .map(|i| {
            let e = &out[HEAD + i * EVENT_WIDTH..HEAD + (i + 1) * EVENT_WIDTH];
            (e[0], e[1], e[2], e[4])
        })
        .collect()
}

/// The item names of a case, in order.
fn item_names(case: Case) -> Vec<&'static str> {
    (0..case.item_count)
        .map(|k| {
            ITEM_NAMES[usize::try_from((case.items >> (3 * k)) & 7).unwrap() % ITEM_NAMES.len()]
        })
        .collect()
}

/// The list the C builds: per item, the first current filter of its name
/// unless it can't be referenced, else the item's loaded filter.
fn expected_list(case: Case) -> Vec<i64> {
    item_names(case)
        .into_iter()
        .enumerate()
        .map(|(k, name)| {
            let k = i64::try_from(k).unwrap();
            match NAMES[..case.count].iter().position(|n| *n == Some(name)) {
                Some(j) if c_long::try_from(j).unwrap() != case.dying => i64::try_from(j).unwrap(),
                _ => LOADED + k,
            }
        })
        .collect()
}

fn compare(case: Case) -> Seen {
    let seen = compare_with(obs_source_restore_filters, case).unwrap_or_else(|| {
        panic!(
            "{case:?}: C {:?}, Rust {:?}",
            run(None, case),
            run(Some(obs_source_restore_filters), case)
        )
    });
    let out = &seen.0;
    // Nothing else written, the lock free, no event lost, every item
    // released.
    assert_eq!(
        (out[UNCHANGED], out[UNLOCKED], out[OVERFLOW], out[HELD]),
        (1, 1, 0, 0),
        "{case:?}"
    );
    seen
}

/// One case with a source and an array, compared and checked against the
/// contract.
fn restore_case(case: Case) {
    let (out, _) = compare(case);
    let seen = events(&out);
    let list = expected_list(case);
    let count = i64::try_from(case.count).unwrap();
    // The new list, chained to the source, every entry parented.
    assert_eq!(
        out[COUNT_AFTER],
        i64::try_from(list.len()).unwrap(),
        "{case:?}"
    );
    assert_eq!(&out[ORDER..ORDER + list.len()], list.as_slice(), "{case:?}");
    let slot = |id: i64| {
        let id = if id >= LOADED {
            id - LOADED + i64::try_from(SLOTS).unwrap()
        } else {
            id
        };
        usize::try_from(id).unwrap()
    };
    // A link left as it was reads UNTOUCHED: before the call, a listed
    // filter targets the next (the last the parent) with the parent as its
    // parent, and every other filter has neither.
    let links = |id: i64| {
        let before = if id < count {
            (if id + 1 < count { id + 1 } else { PARENT }, PARENT)
        } else {
            (NULL_ID, NULL_ID)
        };
        let read = |value: i64, before: i64| if value == UNTOUCHED { before } else { value };
        (
            read(out[TARGETS + slot(id)], before.0),
            read(out[PARENTS + slot(id)], before.1),
        )
    };
    for (i, &filter) in list.iter().enumerate() {
        let (target, parent) = links(filter);
        if !list[i + 1..].contains(&filter) {
            let next = list.get(i + 1).copied().unwrap_or(PARENT);
            assert_eq!(target, next, "{case:?}");
        }
        assert_eq!(parent, PARENT, "{case:?}");
    }
    // An old filter not listed again is unlinked.
    for old in (0..count).filter(|f| !list.contains(f)) {
        assert_eq!(links(old), (NULL_ID, NULL_ID), "{case:?}");
    }
    // No log line; every load private (the chain rule) and every call
    // after the first lock with the lock free; the old filters released
    // last, once each, in old order.
    assert!(seen.iter().all(|e| e.0 != EV_LOG), "{case:?}");
    assert!(seen.iter().all(|e| e.3 == 0), "{case:?}");
    assert!(seen.iter().filter(|e| e.0 == EV_LOAD).all(|e| e.2 == 0));
    let releases: Vec<_> = seen
        .iter()
        .filter(|e| e.0 == EV_RELEASE)
        .map(|e| e.1)
        .collect();
    assert_eq!(releases, (0..count).collect::<Vec<_>>(), "{case:?}");
    assert!(
        seen[seen.len() - releases.len()..]
            .iter()
            .all(|e| e.0 == EV_RELEASE)
    );
    let refs = seen.iter().filter(|e| e.0 == EV_REF).count();
    assert!(refs <= case.item_count);
}

/// Mutants of the export: the real core over a list that breaks one call.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Break {
    NoUnlink,
    SourceTarget,
    LoadPublic,
    ReleaseLocked,
    Unnamed,
    NoRelease,
    ParentWithTarget,
}

struct Mutant {
    source: NonNull<c_void>,
    array: *mut c_void,
    broken: Break,
}

/// The bytes of a C string, or `None` for NULL.
///
/// # Safety
/// As the export's.
unsafe fn bytes<'a>(text: *const c_char) -> Option<&'a [u8]> {
    // SAFETY: the caller's contract.
    (!text.is_null()).then(|| unsafe { CStr::from_ptr(text) }.to_bytes())
}

impl RestoreList for Mutant {
    type Source = *mut c_void;
    type Item = *mut c_void;
    fn this(&self) -> *mut c_void {
        self.source.as_ptr()
    }
    fn lock(&self) {
        // SAFETY: a live fixture source.
        unsafe { obs_source_restore_filters_lock(self.source.as_ptr()) }
    }
    fn count(&self) -> usize {
        // SAFETY: as the export's.
        unsafe { obs_source_restore_filters_count(self.source.as_ptr()) }
    }
    fn filter(&self, index: usize) -> *mut c_void {
        // SAFETY: as the export's.
        unsafe { obs_source_restore_filters_at(self.source.as_ptr(), index) }
    }
    fn unlink(&self, filter: *mut c_void) {
        if self.broken != Break::NoUnlink {
            // SAFETY: as the export's.
            unsafe {
                obs_source_restore_filters_set_parent(filter, null_mut());
                obs_source_restore_filters_set_target(filter, null_mut());
            }
        }
    }
    fn free_list(&self) {
        // SAFETY: as the export's.
        unsafe { obs_source_restore_filters_free_list(self.source.as_ptr()) }
    }
    fn unlock(&self) {
        // SAFETY: as the export's.
        unsafe { obs_source_restore_filters_unlock(self.source.as_ptr()) }
    }
    fn item_count(&self) -> usize {
        // SAFETY: as the export's.
        unsafe { obs_source_restore_filters_item_count(self.array) }
    }
    fn item(&self, index: usize) -> *mut c_void {
        // SAFETY: as the export's.
        unsafe { obs_source_restore_filters_item(self.array, index) }
    }
    fn item_name(&self, item: *mut c_void) -> Option<&[u8]> {
        // SAFETY: as the export's.
        unsafe { bytes(obs_source_restore_filters_item_name(item)) }
    }
    fn name(&self, filter: *mut c_void) -> Option<&[u8]> {
        // SAFETY: as the export's.
        let name = unsafe { bytes(obs_source_restore_filters_name(filter)) };
        name.filter(|_| self.broken != Break::Unnamed)
    }
    fn get_ref(&self, filter: *mut c_void) -> Option<*mut c_void> {
        // SAFETY: as the export's.
        let referenced = unsafe { obs_source_restore_filters_get_ref(filter) };
        (!referenced.is_null()).then_some(referenced)
    }
    fn load(&self, item: *mut c_void) -> *mut c_void {
        // SAFETY: as the export's.
        unsafe { obs_source_restore_filters_load(item) }
    }
    fn load_private(&self, item: *mut c_void) -> *mut c_void {
        if self.broken == Break::LoadPublic {
            return self.load(item);
        }
        // SAFETY: as the export's.
        unsafe { obs_source_restore_filters_load_private(item) }
    }
    fn set_target(&self, filter: *mut c_void, target: *mut c_void) {
        let target = if self.broken == Break::SourceTarget {
            self.source.as_ptr()
        } else {
            target
        };
        // SAFETY: as the export's.
        unsafe { obs_source_restore_filters_set_target(filter, target) }
        if self.broken == Break::ParentWithTarget {
            self.set_parent(filter);
        }
    }
    fn set_parent(&self, filter: *mut c_void) {
        // SAFETY: as the export's.
        unsafe { obs_source_restore_filters_set_parent(filter, self.source.as_ptr()) }
    }
    fn store(&self, filters: Vec<*mut c_void>) {
        // SAFETY: as the export's.
        unsafe {
            obs_source_restore_filters_store(self.source.as_ptr(), filters.as_ptr(), filters.len());
        }
    }
    fn release_item(&self, item: *mut c_void) {
        // SAFETY: as the export's.
        unsafe { obs_source_restore_filters_release_item(item) }
    }
    fn release(&self, filter: *mut c_void) {
        match self.broken {
            Break::NoRelease => {}
            // SAFETY: as the export's, but under the lock.
            Break::ReleaseLocked => unsafe {
                obs_source_restore_filters_lock(self.source.as_ptr());
                obs_source_restore_filters_release(filter);
                obs_source_restore_filters_unlock(self.source.as_ptr());
            },
            // SAFETY: as the export's.
            _ => unsafe { obs_source_restore_filters_release(filter) },
        }
    }
}

unsafe extern "C" fn mutant_restore<const B: u8>(source: *mut c_void, array: *mut c_void) {
    let (Some(source), false) = (NonNull::new(source), array.is_null()) else {
        // SAFETY: as the export's.
        return unsafe { obs_source_restore_filters(source, array) };
    };
    let broken = match B {
        0 => Break::NoUnlink,
        1 => Break::SourceTarget,
        2 => Break::LoadPublic,
        3 => Break::ReleaseLocked,
        4 => Break::Unnamed,
        5 => Break::NoRelease,
        _ => Break::ParentWithTarget,
    };
    let mutant = Mutant {
        source,
        array,
        broken,
    };
    restore(Some(&mutant), true, |_, _| panic!("logged"));
}

/// Selectors packed three bits each, item 0 lowest.
fn pack(selectors: &[u32]) -> u32 {
    selectors
        .iter()
        .rev()
        .fold(0, |packed, &s| (packed << 3) | s)
}

fn case(count: usize, selectors: &[u32], dying: c_long) -> Case {
    Case {
        present: true,
        with_array: true,
        noise: 0x5a,
        count,
        item_count: selectors.len(),
        items: pack(selectors),
        dying,
    }
}

/// Each mutant disagrees with the original C on a chain of 4 restored as
/// "zz", "a", "x", "bb" (a load, a reuse, a load, a reuse; "e" dropped).
fn rejects_mutants() {
    let mutants: [Restore; 7] = [
        mutant_restore::<0>,
        mutant_restore::<1>,
        mutant_restore::<2>,
        mutant_restore::<3>,
        mutant_restore::<4>,
        mutant_restore::<5>,
        mutant_restore::<6>,
    ];
    let probe = case(4, &[4, 0, 6, 1], NULL);
    restore_case(probe);
    for (i, mutant) in mutants.into_iter().enumerate() {
        assert!(
            compare_with(mutant, probe).is_none(),
            "restore_filters mutant {i} agreed with the original C"
        );
    }
}

const SOURCE_LINE: &str = "obs_source_restore_filters: Null 'source' parameter";
const ARRAY_LINE: &str = "obs_source_restore_filters: Null 'array' parameter";

/// NULL source or array: one line, nothing else.
fn null_arguments() {
    for (present, with_array, line) in [
        (false, true, SOURCE_LINE),
        (false, false, SOURCE_LINE),
        (true, false, ARRAY_LINE),
    ] {
        let (out, message) = compare(Case {
            present,
            with_array,
            ..case(3, &[0, 1], NULL)
        });
        assert_eq!(message, line);
        assert_eq!(events(&out), [(EV_LOG, LOG_DEBUG, 1, 0)]);
        assert_eq!(out[LOGS], 1);
        assert_eq!(out[COUNT_AFTER], 3);
        assert_eq!(&out[ORDER..ORDER + 3], [0, 1, 2]);
        assert!(out[TARGETS..UNCHANGED].iter().all(|&l| l == UNTOUCHED));
    }
}

/// Every chain of 0 to 6 current filters (an unnamed one, a repeated name)
/// with every array of up to 3 items over the 7 item names (reusing,
/// loading, duplicating and dropping filters), with no filter or each
/// listed one unreferenceable (sampled past 4 filters), and arrays of 4 to
/// 6 items over a few patterns.
fn every_chain() {
    let mut arrays: Vec<Vec<u32>> = vec![Vec::new()];
    let mut last = vec![Vec::new()];
    for _ in 0..3 {
        let next: Vec<Vec<u32>> = last
            .iter()
            .flat_map(|a: &Vec<u32>| {
                (0..7).map(move |s| {
                    let mut longer = a.clone();
                    longer.push(s);
                    longer
                })
            })
            .collect();
        arrays.extend(next.iter().cloned());
        last = next;
    }
    arrays.extend([
        vec![0, 0, 0, 0],
        vec![3, 2, 1, 0, 4],
        vec![4, 5, 6, 4, 5, 6],
        vec![0, 1, 2, 3, 0, 1],
    ]);
    let mut grid = 0u64;
    for count in 0..=SLOTS {
        for array in &arrays {
            for dying in
                std::iter::once(NULL).chain((0..count).map(|d| c_long::try_from(d).unwrap()))
            {
                grid += 1;
                if count >= 5 && !obs_c_oracle::parity::keep(grid) {
                    continue;
                }
                let noise = u8::try_from(grid % 256).unwrap().wrapping_mul(37);
                restore_case(Case {
                    noise,
                    ..case(count, array, dying)
                });
            }
        }
    }
}

#[test]
fn restore_filters_matches_c() {
    if !crate::output::isolated(concat!(module_path!(), "::restore_filters_matches_c")) {
        return;
    }
    // SAFETY: plain reads of the oracle's constants.
    assert_eq!(
        unsafe {
            [
                oracle_srf_layout(0),
                oracle_srf_layout(1),
                oracle_srf_layout(2),
                oracle_srf_layout(3),
                oracle_srf_layout(4),
            ]
        },
        [OUT, SLOTS, ITEMS, EVENT_WIDTH, ITEM_NAMES.len()]
    );
    null_arguments();
    every_chain();
    rejects_mutants();
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    });
    runner
        .run(
            &(
                any::<(bool, bool, u8, u32)>(),
                0..=SLOTS,
                0..=ITEMS,
                -1..=c_long::try_from(SLOTS).unwrap(),
            ),
            |((present, with_array, noise, items), count, item_count, dying)| {
                let case = Case {
                    present,
                    with_array,
                    noise,
                    count,
                    item_count,
                    items,
                    dying,
                };
                if present && with_array {
                    restore_case(case);
                } else {
                    compare(case);
                }
                Ok(())
            },
        )
        .unwrap();
}

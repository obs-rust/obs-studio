//! Tier 3: the original extracted `obs_save_sources_filtered()` and
//! `obs_save_sources()` versus the shim on one recording fixture core
//! (oracle/save_sources.c), through the real-named adapters. The fixture
//! lists real `struct obs_source` entries from the core's real sources head
//! through their real `hh_uuid` next links, each with the case's filter type
//! and `removed`, `temp_removed` and `private` fields, and records every lock
//! and unlock, the array made, every callback (with its source and a check
//! of its parameter), every save, push and release (with its source, the
//! array pushed into and the mutex held), and whether the array returned is
//! the one made. The callback keeps its source by call number across runs,
//! so one source's answer can change between calls, and on a chosen call
//! edits the fixture: a later source's fields or the current source's next
//! link, which pins where each read is made. Cases: the C test's tables,
//! every exclusion alone and combined (a source both removed and private),
//! every edit position, one and two runs of both entry points, and proptest
//! tables.
//!
//! Excluded: an uninitialized core and another thread's table change,
//! outside the original's contract. One intentional difference, not probed
//! here (the original calls NULL): a NULL callback keeps no source. Mutants
//! show the probe catches each broken step.
use core::ffi::{CStr, c_char, c_void};
use core::ptr;
use obs_core::ffi::save_sources::{Filter, save_sources_bridge, save_sources_filtered_bridge};
use proptest::prelude::*;

const POOL: usize = 8;
const NONE: u8 = 0xff;

const FILTER: u8 = 1;
const REMOVED: u8 = 2;
const TEMP_REMOVED: u8 = 4;
const PRIVATE: u8 = 8;

type Filtered = unsafe extern "C" fn(cb: Option<Filter>, param: *mut c_void) -> *mut c_void;
type All = unsafe extern "C" fn() -> *mut c_void;

#[repr(C)]
#[derive(Clone, Copy)]
struct Case {
    order: *const u8,
    count: usize,
    flags: [u8; POOL],
    runs: u8,
    filtered: u8,
    keep: u32,
    edit_call: u8,
    edit_kind: u8,
    edit_target: u8,
    edit_flags: u8,
}

unsafe extern "C" {
    fn oracle_save_sources_probe(filtered: Filtered, all: All, case: *const Case) -> u32;
    fn oracle_save_sources_trace(
        filtered: Option<Filtered>,
        all: Option<All>,
        case: *const Case,
        trace: *mut c_char,
        size: usize,
    ) -> bool;
    fn oracle_save_sources_adapters() -> bool;
}

unsafe extern "C" fn shim_filtered(cb: Option<Filter>, param: *mut c_void) -> *mut c_void {
    // SAFETY: on the fixture core, with the fixture's callback.
    unsafe { save_sources_filtered_bridge(cb, param) }
}

unsafe extern "C" fn shim_all() -> *mut c_void {
    // SAFETY: on the fixture core.
    unsafe { save_sources_bridge() }
}

/// A case over `order` with `flags` per pool entry, no edit.
fn case(order: &[u8], flags: [u8; POOL], runs: u8, filtered: bool, keep: u32) -> Case {
    Case {
        order: order.as_ptr(),
        count: order.len(),
        flags,
        runs,
        filtered: u8::from(filtered),
        keep,
        edit_call: NONE,
        edit_kind: 0,
        edit_target: 0,
        edit_flags: 0,
    }
}

fn probe_with(filtered: Filtered, all: All, case: &Case) -> u32 {
    // SAFETY: the oracle owns the fixture core every adapter reaches; the
    // order outlives the call.
    unsafe { oracle_save_sources_probe(filtered, all, case) }
}

fn probe(case: &Case) -> u32 {
    probe_with(shim_filtered, shim_all, case)
}

fn trace_of(filtered: Option<Filtered>, all: Option<All>, case: &Case) -> String {
    let mut trace = [0 as c_char; 2048];
    // SAFETY: the buffer is writable for its size; the oracle writes
    // NUL-terminated text.
    let released =
        unsafe { oracle_save_sources_trace(filtered, all, case, trace.as_mut_ptr(), trace.len()) };
    assert!(released, "the run left the mutex held");
    // SAFETY: NUL-terminated by the oracle.
    unsafe { CStr::from_ptr(trace.as_ptr()) }
        .to_str()
        .unwrap()
        .to_owned()
}

/// Runs `body` in a child test process: a `todo!()` or panic inside the
/// extern "C" bridge aborts the process, which would hide every other case.
fn isolated(test: &str, body: impl FnOnce()) {
    const CHILD: &str = "OBS_SAVE_SOURCES_PARITY_CHILD";
    if std::env::var_os(CHILD).is_some_and(|v| v == test) {
        body();
        return;
    }
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg(format!(
            "{}::{test}",
            module_path!().split_once("::").unwrap().1
        ))
        .env(CHILD, test)
        .status()
        .unwrap();
    assert!(status.success(), "{test} failed in its child: {status}");
}

/// The original's trace.
fn original(case: &Case) -> String {
    trace_of(None, None, case)
}

/// The C test's leaves-out case: keep first, an attached filter, a loose
/// filter, a private input, a private scene, a hidden input, a removed
/// input, keep middle, keep last (the released input is gone).
const MIXED_ORDER: [u8; 8] = [0, 1, 2, 3, 4, 5, 6, 7];
const MIXED_FLAGS: [u8; POOL] = [
    0,
    FILTER | PRIVATE,
    FILTER | PRIVATE,
    PRIVATE,
    PRIVATE | REMOVED,
    TEMP_REMOVED,
    REMOVED,
    0,
];

#[test]
fn real_adapters_read_and_write_the_real_fields() {
    // SAFETY: the oracle arms and frees its own fixture.
    assert!(unsafe { oracle_save_sources_adapters() });
}

#[test]
fn original_c_is_the_characterized_walk() {
    // The array before the lock; each candidate saved, pushed and released
    // in table order; the array returned.
    assert_eq!(
        original(&case(&[2, 0, 1], [0; POOL], 1, false, 0)),
        "A0LscpcrcsaparasbpbrbU=y"
    );
    // The callback first, then the save, only for what it keeps.
    assert_eq!(
        original(&case(&[2, 0, 1], [0; POOL], 1, true, 0b101)),
        "A0LccscpcrccacbsbpbrbU=y"
    );
    // An empty table: a new array, the lock and nothing else.
    assert_eq!(original(&case(&[], [0; POOL], 2, true, 0)), "A0LU=y/A1LU=y");
    // Every exclusion: no callback, no save.
    assert_eq!(
        original(&case(&MIXED_ORDER, MIXED_FLAGS, 1, true, u32::MAX)),
        "A0LcasaparachshphrhU=y"
    );
}

/// Every flag combination on its own, at the head, middle and tail.
fn flag_tables() -> Vec<([u8; 3], [u8; POOL])> {
    let mut tables = Vec::new();
    for flags in 0..16_u8 {
        for slot in 0..3 {
            let mut f = [0; POOL];
            f[[4, 1, 6][slot]] = flags;
            tables.push(([4, 1, 6], f));
        }
    }
    tables
}

#[test]
fn characterized_edges() {
    isolated("characterized_edges", || {
        for (order, flags) in flag_tables() {
            for filtered in [false, true] {
                for keep in [0, 0b010, 0b101, u32::MAX] {
                    for runs in [1, 2] {
                        let c = case(&order, flags, runs, filtered, keep);
                        assert_eq!(
                            probe(&c),
                            0,
                            "{flags:?} filtered {filtered} keep {keep:b} x{runs}"
                        );
                    }
                }
            }
        }
        for count in 0..=POOL {
            let order: Vec<u8> = (0..count).map(|i| i as u8).collect();
            let reversed: Vec<u8> = order.iter().rev().copied().collect();
            for table in [&order, &reversed] {
                for filtered in [false, true] {
                    let c = case(table, MIXED_FLAGS, 2, filtered, 0x5a5a);
                    assert_eq!(probe(&c), 0, "{table:?} filtered {filtered}");
                }
            }
        }
    });
}

#[test]
fn a_callback_that_changes_its_answer_matches() {
    isolated("a_callback_that_changes_its_answer_matches", || {
        // Two runs: calls 0..3 keep a and c, calls 3..6 keep b only.
        let c = case(&[0, 1, 2], [0; POOL], 2, true, 0b010_101);
        assert_eq!(original(&c), "A0LcasaparacbccscpcrcU=y/A1LcacbsbpbrbccU=y");
        assert_eq!(probe(&c), 0);
    });
}

#[test]
fn edits_from_the_callback_pin_each_read() {
    isolated("edits_from_the_callback_pin_each_read", || {
        // On each callback call, mark a source (a later one, or one already past
        // its checks) with every flag combination, or relink the current source's
        // next link to a later source, an unlisted one or NULL (the fixture
        // leaves a link to an earlier one alone, so the walk ends).
        let order = [3_u8, 0, 5, 1, 7];
        for edit_call in 0..5_u8 {
            for edit_target in order {
                for edit_flags in 0..16_u8 {
                    let mut c = case(&order, [0; POOL], 2, true, 0b1011_0110_1101);
                    c.edit_call = edit_call;
                    c.edit_kind = 0;
                    c.edit_target = edit_target;
                    c.edit_flags = edit_flags;
                    assert_eq!(probe(&c), 0, "flags {edit_call} {edit_target} {edit_flags}");
                }
            }
            for edit_target in [3_u8, 0, 5, 1, 7, 2, NONE] {
                let mut c = case(&order, [0; POOL], 1, true, u32::MAX);
                c.edit_call = edit_call;
                c.edit_kind = 1;
                c.edit_target = edit_target;
                assert_eq!(probe(&c), 0, "relink {edit_call} {edit_target}");
            }
        }
        // The original reads the next link after the callback and the save.
        let mut c = case(&[0, 1, 2], [0; POOL], 1, true, u32::MAX);
        c.edit_call = 0;
        c.edit_kind = 1;
        c.edit_target = 2;
        assert_eq!(original(&c), "A0LcasaparaccscpcrcU=y");
        // A flag set on a later source from the callback is seen.
        let mut c = case(&[0, 1, 2], [0; POOL], 1, true, u32::MAX);
        c.edit_call = 0;
        c.edit_kind = 0;
        c.edit_target = 1;
        c.edit_flags = REMOVED;
        assert_eq!(original(&c), "A0LcasaparaccscpcrcU=y");
    });
}

// Mutants: a walk that breaks one step must be caught.

unsafe extern "C" {
    fn obs_save_sources_lock();
    fn obs_save_sources_unlock();
    fn obs_save_sources_first() -> *mut c_void;
    fn obs_save_sources_next(source: *const c_void) -> *mut c_void;
    fn obs_save_sources_is_filter(source: *const c_void) -> bool;
    fn obs_save_sources_removed(source: *const c_void) -> bool;
    fn obs_save_sources_temp_removed(source: *const c_void) -> bool;
    fn obs_save_sources_private(source: *const c_void) -> bool;
    fn obs_save_sources_array_create() -> *mut c_void;
    fn obs_save_sources_save(source: *mut c_void) -> *mut c_void;
    fn obs_save_sources_push(array: *mut c_void, data: *mut c_void);
    fn obs_save_sources_release(data: *mut c_void);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mutant {
    /// The next link read before the callback.
    NextFirst,
    /// The temp-removed (hidden) check left out.
    KeepsHidden,
    /// The private check made after the callback.
    PrivateAfterCallback,
    /// The array made under the lock.
    ArrayUnderLock,
    /// The data released before its push.
    ReleaseBeforePush,
    /// No lock at all.
    Unlocked,
}

/// The walk, broken as `mutant` says.
///
/// # Safety
/// The fixture's adapters, on this thread's fixture.
unsafe fn walk(mutant: Mutant, cb: Option<Filter>, param: *mut c_void) -> *mut c_void {
    // SAFETY: as the shim, on the fixture core.
    unsafe {
        let keep = |source: *mut c_void| cb.is_some_and(|cb| cb(param, source));
        let early = mutant == Mutant::ArrayUnderLock;
        let mut array = ptr::null_mut();
        if !early {
            array = obs_save_sources_array_create();
        }
        if mutant != Mutant::Unlocked {
            obs_save_sources_lock();
        }
        if early {
            array = obs_save_sources_array_create();
        }
        let mut source = obs_save_sources_first();
        while !source.is_null() {
            let next = (mutant == Mutant::NextFirst).then(|| obs_save_sources_next(source));
            let candidate = !obs_save_sources_is_filter(source)
                && !obs_save_sources_removed(source)
                && (mutant == Mutant::KeepsHidden || !obs_save_sources_temp_removed(source))
                && (mutant == Mutant::PrivateAfterCallback || !obs_save_sources_private(source))
                && keep(source)
                && (mutant != Mutant::PrivateAfterCallback || !obs_save_sources_private(source));
            if candidate {
                let data = obs_save_sources_save(source);
                if mutant == Mutant::ReleaseBeforePush {
                    obs_save_sources_release(data);
                    obs_save_sources_push(array, data);
                } else {
                    obs_save_sources_push(array, data);
                    obs_save_sources_release(data);
                }
            }
            source = next.unwrap_or_else(|| obs_save_sources_next(source));
        }
        if mutant != Mutant::Unlocked {
            obs_save_sources_unlock();
        }
        array
    }
}

macro_rules! mutant_fns {
    ($($name:ident => $mutant:expr),* $(,)?) => {
        $(
            unsafe extern "C" fn $name(cb: Option<Filter>, param: *mut c_void) -> *mut c_void {
                // SAFETY: on the fixture.
                unsafe { walk($mutant, cb, param) }
            }
        )*
    };
}

mutant_fns! {
    next_first => Mutant::NextFirst,
    keeps_hidden => Mutant::KeepsHidden,
    private_after_callback => Mutant::PrivateAfterCallback,
    array_under_lock => Mutant::ArrayUnderLock,
    release_before_push => Mutant::ReleaseBeforePush,
    unlocked => Mutant::Unlocked,
}

#[test]
fn mutants_are_caught() {
    isolated("mutants_are_caught", || {
        // The mixed table, filtered, once as it is and once with the first
        // callback ending the walk early (its next link relinked to NULL): every
        // mutant breaks one step of one of them.
        let plain = case(&MIXED_ORDER, MIXED_FLAGS, 1, true, u32::MAX);
        let mut relinked = plain;
        relinked.edit_call = 0;
        relinked.edit_kind = 1;
        relinked.edit_target = NONE;
        for mutant in [
            next_first,
            keeps_hidden,
            private_after_callback,
            array_under_lock,
            release_before_push,
            unlocked,
        ] {
            assert!(
                probe_with(mutant, shim_all, &plain) != 0
                    || probe_with(mutant, shim_all, &relinked) != 0
            );
        }
        // The shim itself matches on both.
        assert_eq!(probe(&plain), 0);
        assert_eq!(probe(&relinked), 0);
    });
}

#[test]
fn any_table_matches_original_c() {
    isolated("any_table_matches_original_c", || {
        let config = ProptestConfig {
            cases: obs_c_oracle::parity::cases(256),
            failure_persistence: None,
            ..ProptestConfig::default()
        };
        let strategy = (
            Just((0..POOL as u8).collect::<Vec<u8>>()).prop_shuffle(),
            0..=POOL,
            proptest::array::uniform8(0..16_u8),
            1..=2_u8,
            any::<bool>(),
            any::<u32>(),
            prop_oneof![Just(NONE), 0..16_u8],
            0..=1_u8,
            prop_oneof![Just(NONE), 0..POOL as u8],
            0..16_u8,
        );
        proptest::test_runner::TestRunner::new(config)
            .run(
                &strategy,
                |(
                    order,
                    count,
                    flags,
                    runs,
                    filtered,
                    keep,
                    edit_call,
                    edit_kind,
                    edit_target,
                    edit_flags,
                )| {
                    let mut c = case(&order[..count], flags, runs, filtered, keep);
                    c.edit_call = edit_call;
                    c.edit_kind = edit_kind;
                    c.edit_target = edit_target;
                    c.edit_flags = edit_flags;
                    prop_assert_eq!(probe(&c), 0);
                    Ok(())
                },
            )
            .unwrap();
    });
}

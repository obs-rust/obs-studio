//! Tier 3: the original extracted `obs_load_sources()` versus the shim on one
//! recording fixture (oracle/load_sources.c), through the real-named
//! adapters. The fixture loads real `struct obs_source` entries from a tagged
//! data array, each typed a transition or an input in its real `info.type`,
//! and records the count read, every item take and data release, every load,
//! transition load, second-pass load, callback (with a check of its private
//! data) and source release (NULL included), with a mark for a call on an
//! item not held and for a reference left at the end of a run. On a chosen
//! second-pass load the fixture flips a source's type, which pins where the
//! type is read. Cases: the C test's arrays, an empty and a NULL array, NULL
//! slots anywhere, a NULL callback, one source in two slots, every edit
//! position, one and two runs, and proptest arrays.
//!
//! Excluded: an uninitialized core and another thread's change of the array,
//! outside the original's contract. No intentional differences. Mutants show
//! the probe catches each broken step.
use core::ffi::{CStr, c_char, c_void};
use core::ptr;
use obs_core::ffi::load_sources::{LoadCallback, load_sources_bridge};
use proptest::prelude::*;

const POOL: usize = 8;
const NONE: u8 = 0xff;

type Candidate =
    unsafe extern "C" fn(array: *mut c_void, cb: Option<LoadCallback>, private_data: *mut c_void);

#[repr(C)]
#[derive(Clone, Copy)]
struct Case {
    count: u8,
    null_array: u8,
    slots: [u8; POOL],
    transition: u8,
    with_cb: u8,
    runs: u8,
    edit_call: u8,
    edit_target: u8,
}

unsafe extern "C" {
    fn oracle_load_sources_probe(candidate: Candidate, case: *const Case) -> u32;
    fn oracle_load_sources_trace(
        candidate: Option<Candidate>,
        case: *const Case,
        trace: *mut c_char,
        size: usize,
    ) -> bool;
    fn oracle_load_sources_adapters() -> bool;
}

unsafe extern "C" fn shim(array: *mut c_void, cb: Option<LoadCallback>, private_data: *mut c_void) {
    // SAFETY: on the fixture, with the fixture's array and callback.
    unsafe { load_sources_bridge(array, cb, private_data) }
}

/// A case over `slots` (one per entry), `transition` the pool entries that
/// are transitions, no edit.
fn case(slots: &[u8], transition: u8, with_cb: bool, runs: u8) -> Case {
    let mut all = [NONE; POOL];
    all[..slots.len()].copy_from_slice(slots);
    Case {
        count: slots.len() as u8,
        null_array: 0,
        slots: all,
        transition,
        with_cb: u8::from(with_cb),
        runs,
        edit_call: NONE,
        edit_target: NONE,
    }
}

fn probe_with(candidate: Candidate, case: &Case) -> u32 {
    // SAFETY: the oracle owns the fixture every adapter reaches.
    unsafe { oracle_load_sources_probe(candidate, case) }
}

fn probe(case: &Case) -> u32 {
    probe_with(shim, case)
}

/// The original's trace.
fn original(case: &Case) -> String {
    let mut trace = [0 as c_char; 1024];
    // SAFETY: the buffer is writable for its size; the oracle writes
    // NUL-terminated text.
    let fits = unsafe { oracle_load_sources_trace(None, case, trace.as_mut_ptr(), trace.len()) };
    assert!(fits, "the case does not fit the fixture");
    // SAFETY: NUL-terminated by the oracle.
    unsafe { CStr::from_ptr(trace.as_ptr()) }
        .to_str()
        .unwrap()
        .to_owned()
}

/// Runs `body` in a child test process: a `todo!()` or panic inside the
/// extern "C" bridge aborts the process, which would hide every other case.
fn isolated(test: &str, body: impl FnOnce()) {
    const CHILD: &str = "OBS_LOAD_SOURCES_PARITY_CHILD";
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

#[test]
fn real_adapters_reach_the_real_calls_and_field() {
    // SAFETY: the oracle arms and frees its own fixture.
    assert!(unsafe { oracle_load_sources_adapters() });
}

#[test]
fn original_c_is_the_characterized_walk() {
    // Pass 1 takes, loads and releases each item; pass 2 takes it again,
    // transition-loads a transition, load2s and calls back each source and
    // releases the item (a NULL slot only takes and releases); pass 3
    // releases every slot, NULL included.
    assert_eq!(
        original(&case(&[2, NONE, 4], 1 << 4, true, 1)),
        "Ni0l0r0i1l1r1i2l2r2i0Dcccr0i1r1i2Te2Decer2xcx-xe="
    );
    // A NULL callback: the loads still run.
    assert_eq!(original(&case(&[0], 0, false, 1)), "Ni0l0r0i0Dar0xa=");
    // An empty array and a NULL one: the count, nothing else.
    assert_eq!(original(&case(&[], 0, true, 2)), "N=/N=");
    let mut null = case(&[0, 1], 0, true, 2);
    null.null_array = 1;
    assert_eq!(original(&null), "N=/N=");
}

#[test]
fn characterized_edges() {
    isolated("characterized_edges", || {
        // Every array of up to four entries over a NULL slot and two pool
        // entries (one of them possibly in two slots), every transition mix,
        // with and without the callback, once and twice.
        let choices = [NONE, 1, 6];
        for count in 0..=4_u32 {
            for pattern in 0..3_u32.pow(count) {
                let slots: Vec<u8> = (0..count)
                    .map(|i| choices[(pattern / 3_u32.pow(i) % 3) as usize])
                    .collect();
                for transition in [0, 1 << 1, 1 << 6, (1 << 1) | (1 << 6)] {
                    for with_cb in [false, true] {
                        for runs in [1, 2] {
                            let c = case(&slots, transition, with_cb, runs);
                            assert_eq!(
                                probe(&c),
                                0,
                                "{slots:?} transitions {transition:b} cb {with_cb} x{runs}"
                            );
                        }
                    }
                }
            }
        }
        // A NULL array.
        for with_cb in [false, true] {
            let mut c = case(&[0, 1, 2], 0b111, with_cb, 2);
            c.null_array = 1;
            assert_eq!(probe(&c), 0);
        }
        // The C test's array: three inputs, a missing-plugin entry, the
        // transition's source A and the transition itself, every entry
        // reversed too.
        let order = [0_u8, 1, 2, 3, 4, 5];
        let reversed = [5_u8, 4, 3, 2, 1, 0];
        for slots in [&order, &reversed] {
            for with_cb in [false, true] {
                assert_eq!(probe(&case(slots, 1 << 5, with_cb, 2)), 0);
            }
        }
        // A full array.
        let full: Vec<u8> = (0..POOL as u8).collect();
        assert_eq!(probe(&case(&full, 0b1010_0110, true, 2)), 0);
    });
}

#[test]
fn type_flips_from_second_pass_loads_match() {
    isolated("type_flips_from_second_pass_loads_match", || {
        for transition in 0..8_u8 {
            for edit_call in 0..4_u8 {
                for edit_target in [0_u8, 1, 2, NONE] {
                    for slots in [[0_u8, 1, 2], [0, NONE, 2]] {
                        let mut c = case(&slots, transition, true, 2);
                        c.edit_call = edit_call;
                        c.edit_target = edit_target;
                        assert_eq!(
                            probe(&c),
                            0,
                            "{slots:?} {transition:b} edit {edit_call} {edit_target}"
                        );
                    }
                }
            }
        }
        // The original reads a source's type in pass 2, after the earlier
        // sources' load2: source b made a transition by a's load2 is
        // transition-loaded.
        let mut c = case(&[0, 1], 0, true, 1);
        c.edit_call = 0;
        c.edit_target = 1;
        assert_eq!(original(&c), "Ni0l0r0i1l1r1i0Dacar0i1Tb1Dbcbr1xaxb=");
        assert_eq!(probe(&c), 0);
    });
}

// Mutants: a walk that breaks one step must be caught.

unsafe extern "C" {
    fn obs_load_sources_count(array: *mut c_void) -> usize;
    fn obs_load_sources_item(array: *mut c_void, index: usize) -> *mut c_void;
    fn obs_load_sources_release_data(data: *mut c_void);
    fn obs_load_sources_load(data: *mut c_void) -> *mut c_void;
    fn obs_load_sources_is_transition(source: *const c_void) -> bool;
    fn obs_load_sources_transition_load(source: *mut c_void, data: *mut c_void);
    fn obs_load_sources_load2(source: *mut c_void);
    fn obs_load_sources_release_source(source: *mut c_void);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mutant {
    /// The item released before its source is loaded.
    ReleaseBeforeLoad,
    /// The type read in pass 1, right after the load.
    TypeInPass1,
    /// The callback before load2.
    CallbackFirst,
    /// No item taken in pass 2 for a NULL slot.
    SkipNullItem,
    /// A NULL slot not released in pass 3.
    SkipNullRelease,
    /// Each source released right after its callback, not in pass 3.
    ReleaseInPass2,
    /// The count read again for pass 2.
    CountTwice,
}

/// The walk, broken as `mutant` says.
///
/// # Safety
/// The fixture's adapters, on this thread's fixture.
unsafe fn walk(
    mutant: Mutant,
    array: *mut c_void,
    cb: Option<LoadCallback>,
    private_data: *mut c_void,
) {
    // SAFETY: as the shim, on the fixture.
    unsafe {
        let count = obs_load_sources_count(array);
        let mut sources = Vec::new();
        let mut types = Vec::new();
        for i in 0..count {
            let data = obs_load_sources_item(array, i);
            if mutant == Mutant::ReleaseBeforeLoad {
                obs_load_sources_release_data(data);
            }
            let source = obs_load_sources_load(data);
            types.push(
                mutant == Mutant::TypeInPass1
                    && !source.is_null()
                    && obs_load_sources_is_transition(source),
            );
            sources.push(source);
            if mutant != Mutant::ReleaseBeforeLoad {
                obs_load_sources_release_data(data);
            }
        }
        let pass2 = if mutant == Mutant::CountTwice {
            obs_load_sources_count(array)
        } else {
            sources.len()
        };
        for i in 0..pass2.min(sources.len()) {
            let source = sources[i];
            if source.is_null() && mutant == Mutant::SkipNullItem {
                continue;
            }
            let data = obs_load_sources_item(array, i);
            if !source.is_null() {
                let transition = if mutant == Mutant::TypeInPass1 {
                    types[i]
                } else {
                    obs_load_sources_is_transition(source)
                };
                if transition {
                    obs_load_sources_transition_load(source, data);
                }
                if mutant == Mutant::CallbackFirst
                    && let Some(cb) = cb
                {
                    cb(private_data, source);
                }
                obs_load_sources_load2(source);
                if mutant != Mutant::CallbackFirst
                    && let Some(cb) = cb
                {
                    cb(private_data, source);
                }
                if mutant == Mutant::ReleaseInPass2 {
                    obs_load_sources_release_source(source);
                }
            }
            obs_load_sources_release_data(data);
        }
        for source in sources {
            if mutant == Mutant::ReleaseInPass2 && !source.is_null() {
                continue;
            }
            if source.is_null() && mutant == Mutant::SkipNullRelease {
                continue;
            }
            obs_load_sources_release_source(if source.is_null() {
                ptr::null_mut()
            } else {
                source
            });
        }
    }
}

macro_rules! mutant_fns {
    ($($name:ident => $mutant:expr),* $(,)?) => {
        $(
            unsafe extern "C" fn $name(
                array: *mut c_void,
                cb: Option<LoadCallback>,
                private_data: *mut c_void,
            ) {
                // SAFETY: on the fixture.
                unsafe { walk($mutant, array, cb, private_data) }
            }
        )*
    };
}

mutant_fns! {
    release_before_load => Mutant::ReleaseBeforeLoad,
    type_in_pass1 => Mutant::TypeInPass1,
    callback_first => Mutant::CallbackFirst,
    skip_null_item => Mutant::SkipNullItem,
    skip_null_release => Mutant::SkipNullRelease,
    release_in_pass2 => Mutant::ReleaseInPass2,
    count_twice => Mutant::CountTwice,
}

#[test]
fn mutants_are_caught() {
    isolated("mutants_are_caught", || {
        // Two inputs around a NULL slot and a transition, with the first
        // load2 making the third entry's source a transition.
        let mut c = case(&[0, NONE, 1, 2], 1 << 1, true, 1);
        c.edit_call = 0;
        c.edit_target = 2;
        for mutant in [
            release_before_load,
            type_in_pass1,
            callback_first,
            skip_null_item,
            skip_null_release,
            release_in_pass2,
            count_twice,
        ] {
            assert_ne!(probe_with(mutant, &c), 0);
        }
        // The shim itself matches.
        assert_eq!(probe(&c), 0);
    });
}

#[test]
fn any_array_matches_original_c() {
    isolated("any_array_matches_original_c", || {
        let config = ProptestConfig {
            cases: obs_c_oracle::parity::cases(256),
            failure_persistence: None,
            ..ProptestConfig::default()
        };
        let strategy = (
            proptest::collection::vec(prop_oneof![Just(NONE), 0..POOL as u8], 0..=POOL),
            any::<bool>(),
            any::<u8>(),
            any::<bool>(),
            1..=2_u8,
            prop_oneof![Just(NONE), 0..16_u8],
            prop_oneof![Just(NONE), 0..POOL as u8],
        );
        proptest::test_runner::TestRunner::new(config)
            .run(
                &strategy,
                |(slots, null_array, transition, with_cb, runs, edit_call, edit_target)| {
                    let mut c = case(&slots, transition, with_cb, runs);
                    c.null_array = u8::from(null_array);
                    c.edit_call = edit_call;
                    c.edit_target = edit_target;
                    prop_assert_eq!(probe(&c), 0);
                    Ok(())
                },
            )
            .unwrap();
    });
}

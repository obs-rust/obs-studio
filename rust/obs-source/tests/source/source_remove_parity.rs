//! Tier 3: the original C `obs_source_remove()` against the Rust export on
//! noise-filled real-header sources: the source passed in and a twin the
//! stand-in `obs_source_get_ref()` may return instead, so each read and step
//! shows which pointer it uses. It compares every call (the reference, the
//! global and source signals under `obs_source_dosignal()`'s private rule,
//! the canvas removal and the release, each with the source it names and
//! that source's removed flag then), the log lines in order with the rest,
//! both removed flags afterwards, every other byte and the last log line.
use core::cell::Cell;
use core::ffi::{CStr, c_char, c_int, c_long, c_uint, c_void};
use core::ptr::NonNull;
use obs_source::ffi::source_remove::obs_source_remove;
use obs_source::source_remove::{Source, remove};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Remove = unsafe extern "C" fn(*mut c_void);
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);

unsafe extern "C" {
    fn oracle_sr_run(
        remove: Option<Remove>,
        get: GetLog,
        set: SetLog,
        present: bool,
        noise: u8,
        flags: c_uint,
        reference: c_int,
        out: *mut c_long,
        message: *mut c_char,
        message_size: usize,
    ) -> bool;
    fn oracle_sr_layout(which: usize) -> usize;
    fn obs_source_remove_removed(source: *const c_void) -> bool;
    fn obs_source_remove_get_ref(source: *mut c_void) -> *mut c_void;
    fn obs_source_remove_set_removed(source: *mut c_void);
    fn obs_source_remove_signal(source: *mut c_void);
    fn obs_source_remove_has_canvas(source: *const c_void) -> bool;
    fn obs_source_remove_private(source: *const c_void) -> bool;
    fn obs_source_remove_from_canvas(source: *mut c_void);
    fn obs_source_remove_release(source: *mut c_void);
}

/// The out array's layout (`SR_OUT_*`) and the event width
/// (`SR_EVENT_WIDTH`).
const EVENT_WIDTH: usize = 4;
const EVENTS: usize = 8;
const REMOVED: usize = 0;
const TWIN_REMOVED: usize = 1;
const UNCHANGED: usize = 2;
const LOGS: usize = 3;
const EVENT_COUNT: usize = 4;
const OVERFLOW: usize = 5;
const HEAD: usize = 6;
const OUT: usize = HEAD + EVENTS * EVENT_WIDTH;
/// Event kinds (`SR_EV_*`).
const EV_GET_REF: c_long = 1;
const EV_GLOBAL_SIGNAL: c_long = 2;
const EV_SOURCE_SIGNAL: c_long = 3;
const EV_CANVAS_REMOVE: c_long = 4;
const EV_RELEASE: c_long = 5;
const EV_LOG: c_long = 6;
/// Source ids.
const NULL_ID: c_long = -1;
const SOURCE: c_long = 0;
const TWIN: c_long = 1;
/// What the stand-in reference is (`SR_REF_*`).
const REF_NULL: c_int = 0;
const REF_SELF: c_int = 1;
const REF_TWIN: c_int = 2;
/// Fixture flag bits.
const F_REMOVED: c_uint = 1;
const F_CANVAS: c_uint = 2;
const F_PRIVATE: c_uint = 4;
const F_TWIN_CANVAS: c_uint = 8;
const F_TWIN_PRIVATE: c_uint = 16;

/// What one call left: the out array and the last log line.
type Seen = (Vec<c_long>, String);

#[derive(Clone, Copy, Debug)]
struct Case {
    present: bool,
    noise: u8,
    flags: c_uint,
    reference: c_int,
}

fn run(remove: Option<Remove>, case: Case) -> Seen {
    let mut out: [c_long; OUT] = [0; OUT];
    let mut buffer = [0 as c_char; 128];
    // SAFETY: the isolated child owns the log handler. C builds the fixture,
    // runs the call, records and frees it, and restores logging; both
    // buffers are as long as passed.
    let built = unsafe {
        oracle_sr_run(
            remove,
            base_get_log_handler,
            base_set_log_handler,
            case.present,
            case.noise,
            case.flags,
            case.reference,
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
fn compare_with(remove: Remove, case: Case) -> Option<Seen> {
    let c = run(None, case);
    let rust = run(Some(remove), case);
    (c == rust).then_some(c)
}

/// The recorded events: kind, source id, its removed flag, name check.
fn events(out: &[c_long]) -> Vec<[c_long; EVENT_WIDTH]> {
    let count = usize::try_from(out[EVENT_COUNT]).unwrap();
    (0..count)
        .map(|i| {
            let at = HEAD + i * EVENT_WIDTH;
            [out[at], out[at + 1], out[at + 2], out[at + 3]]
        })
        .collect()
}

fn compare(case: Case) -> Seen {
    let seen = compare_with(obs_source_remove, case).unwrap_or_else(|| {
        panic!(
            "{case:?}: C {:?}, Rust {:?}",
            run(None, case),
            run(Some(obs_source_remove), case)
        )
    });
    // Nothing but the removed flags written, and no event lost.
    assert_eq!((seen.0[UNCHANGED], seen.0[OVERFLOW]), (1, 0), "{case:?}");
    seen
}

/// The contract, from the original's observation of `case`.
fn check(case: Case, (out, message): &Seen) {
    let events = events(out);
    if !case.present {
        assert_eq!(message, "obs_source_remove: Null 'source' parameter");
        assert_eq!(events, [[EV_LOG, NULL_ID, -1, 400]], "{case:?}");
        assert_eq!(out[LOGS], 1);
        return;
    }
    assert_eq!(out[LOGS], 0, "{case:?}");
    let removed = case.flags & F_REMOVED != 0;
    if removed {
        assert!(events.is_empty(), "{case:?}");
        return;
    }
    let mut expected = vec![[EV_GET_REF, SOURCE, 0, 0]];
    let s = match case.reference {
        REF_SELF => SOURCE,
        REF_TWIN => TWIN,
        _ => {
            assert_eq!(events, expected, "{case:?}");
            assert_eq!((out[REMOVED], out[TWIN_REMOVED]), (0, 0));
            return;
        }
    };
    // The signal's private rule reads the reference; the canvas check reads
    // the source passed in.
    let s_private = if s == SOURCE {
        case.flags & F_PRIVATE != 0
    } else {
        case.flags & F_TWIN_PRIVATE != 0
    };
    if !s_private {
        expected.push([EV_GLOBAL_SIGNAL, s, 1, 1]);
    }
    expected.push([EV_SOURCE_SIGNAL, s, 1, 1]);
    if case.flags & F_CANVAS != 0 && case.flags & F_PRIVATE == 0 {
        expected.push([EV_CANVAS_REMOVE, s, 1, 0]);
    }
    expected.push([EV_RELEASE, s, 1, 0]);
    assert_eq!(events, expected, "{case:?}");
    assert_eq!(
        (out[REMOVED], out[TWIN_REMOVED]),
        (c_long::from(s == SOURCE), c_long::from(s == TWIN)),
        "{case:?}"
    );
}

/// Mutants of the export: the real core over a source that breaks one call.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Break {
    NoSetRemoved,
    SetAfterSignal,
    IgnorePrivate,
    CanvasFromReference,
    NoRelease,
}

struct Mutant {
    source: NonNull<c_void>,
    broken: Break,
    /// The reference get_ref() returned.
    reference: Cell<*mut c_void>,
}

impl Source for Mutant {
    type Ref = NonNull<c_void>;
    fn removed(&self) -> bool {
        // SAFETY: a live fixture source.
        unsafe { obs_source_remove_removed(self.source.as_ptr()) }
    }
    fn get_ref(&self) -> Option<NonNull<c_void>> {
        // SAFETY: as the export's.
        let s = unsafe { obs_source_remove_get_ref(self.source.as_ptr()) };
        self.reference.set(s);
        NonNull::new(s)
    }
    fn set_removed(&self, s: NonNull<c_void>) {
        if self.broken == Break::NoSetRemoved || self.broken == Break::SetAfterSignal {
            return;
        }
        // SAFETY: as the export's.
        unsafe { obs_source_remove_set_removed(s.as_ptr()) }
    }
    fn signal(&self, s: NonNull<c_void>) {
        // SAFETY: as the export's.
        unsafe { obs_source_remove_signal(s.as_ptr()) }
        if self.broken == Break::SetAfterSignal {
            // SAFETY: as the export's.
            unsafe { obs_source_remove_set_removed(s.as_ptr()) }
        }
    }
    fn has_canvas(&self) -> bool {
        if self.broken == Break::CanvasFromReference {
            // SAFETY: the reference get_ref() returned, a fixture source.
            return unsafe { obs_source_remove_has_canvas(self.reference.get()) };
        }
        // SAFETY: as the export's.
        unsafe { obs_source_remove_has_canvas(self.source.as_ptr()) }
    }
    fn is_private(&self) -> bool {
        // SAFETY: as the export's.
        self.broken != Break::IgnorePrivate
            && unsafe { obs_source_remove_private(self.source.as_ptr()) }
    }
    fn remove_from_canvas(&self, s: NonNull<c_void>) {
        // SAFETY: as the export's.
        unsafe { obs_source_remove_from_canvas(s.as_ptr()) }
    }
    fn release(&self, s: NonNull<c_void>) {
        if self.broken != Break::NoRelease {
            // SAFETY: as the export's.
            unsafe { obs_source_remove_release(s.as_ptr()) }
        }
    }
}

fn broken(b: u8) -> Break {
    match b {
        0 => Break::NoSetRemoved,
        1 => Break::SetAfterSignal,
        2 => Break::IgnorePrivate,
        3 => Break::CanvasFromReference,
        _ => Break::NoRelease,
    }
}

unsafe extern "C" fn mutant_remove<const B: u8>(source: *mut c_void) {
    let Some(source) = NonNull::new(source) else {
        // SAFETY: as the export's.
        return unsafe { obs_source_remove(source) };
    };
    let mutant = Mutant {
        source,
        broken: broken(B),
        reference: Cell::new(core::ptr::null_mut()),
    };
    remove(Some(&mutant), |_, _| panic!("logged"));
}

/// Each mutant disagrees with the original C somewhere on the grid.
fn rejects_mutants() {
    let mutants: [Remove; 5] = [
        mutant_remove::<0>,
        mutant_remove::<1>,
        mutant_remove::<2>,
        mutant_remove::<3>,
        mutant_remove::<4>,
    ];
    for mutant in mutants {
        let caught = (0..32).any(|flags| {
            [REF_SELF, REF_TWIN].into_iter().any(|reference| {
                compare_with(
                    mutant,
                    Case {
                        present: true,
                        noise: 0x5a,
                        flags,
                        reference,
                    },
                )
                .is_none()
            })
        });
        assert!(
            caught,
            "an obs_source_remove mutant agreed with the original C"
        );
    }
}

/// The canvas and private checks read the source passed in, not the
/// reference: a twin with a canvas is not taken off it, and the source's own
/// canvas decides even when the reference has none.
fn canvas_read_from_the_argument() {
    let removes = |flags| {
        let case = Case {
            present: true,
            noise: 0x5a,
            flags,
            reference: REF_TWIN,
        };
        let seen = compare(case);
        events(&seen.0)
            .iter()
            .any(|event| event[0] == EV_CANVAS_REMOVE)
    };
    assert!(!removes(F_TWIN_CANVAS));
    assert!(removes(F_CANVAS | F_TWIN_PRIVATE));
    assert!(!removes(F_CANVAS | F_PRIVATE | F_TWIN_CANVAS));
}

/// NULL, then every removed, canvas and private pattern of the source and
/// the twin, for each reference, at several noise bytes.
fn every_branch() {
    for noise in [0x00, 0x5a, 0xa5, 0xff] {
        let case = Case {
            present: false,
            noise,
            flags: 0,
            reference: REF_SELF,
        };
        check(case, &compare(case));
        for flags in 0..32 {
            for reference in [REF_NULL, REF_SELF, REF_TWIN] {
                let case = Case {
                    present: true,
                    noise,
                    flags,
                    reference,
                };
                check(case, &compare(case));
            }
        }
    }
}

#[test]
fn source_remove_matches_c() {
    if !crate::output::isolated(concat!(module_path!(), "::source_remove_matches_c")) {
        return;
    }
    // SAFETY: plain reads of the oracle's constants.
    assert_eq!(
        unsafe { [oracle_sr_layout(0), oracle_sr_layout(1)] },
        [OUT, EVENT_WIDTH]
    );
    every_branch();
    canvas_read_from_the_argument();
    rejects_mutants();
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    });
    runner
        .run(
            &(any::<(bool, u8)>(), 0..32u32, REF_NULL..=REF_TWIN),
            |((present, noise), flags, reference)| {
                let case = Case {
                    present,
                    noise,
                    flags,
                    reference,
                };
                check(case, &compare(case));
                Ok(())
            },
        )
        .unwrap();
}

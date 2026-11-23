//! Tier 3: the original extracted `obs_set_audio_monitoring_device()` and
//! `obs_reset_audio_monitoring()` versus the shims on one recording fixture
//! core (oracle/audio_monitoring_device.c), through the real-named adapters
//! of `obs-audio-monitoring-device-fields.c.h`. Compared: the result, the
//! trace of every core read, availability check, lock, compare, free, copy
//! (each step on the fields checked to run under the lock), monitor reset,
//! enumeration (checked to run after the unlock) and each source's settings
//! read, notice (with its own `device_id`) and release, and the stored name
//! and id. Inputs: NULL, empty and short names and ids, the same id as
//! stored or not, monitoring available or not, up to eight monitors and up
//! to eight sources with and without `OBS_SOURCE_DO_NOT_SELF_MONITOR`.
//!
//! Excluded: an uninitialized core (both dereference it once the name and
//! id pass) and a NULL stored id (both pass it to `strcmp`). No intentional
//! differences. Mutants show the probe catches each broken step.
use core::cell::Cell;
use core::ffi::{CStr, c_char, c_int, c_void};
use core::ptr::null;
use std::ffi::CString;

use obs_core::core_setup_rules::DO_NOT_SELF_MONITOR;
use obs_core::ffi::audio_monitoring_device::{
    Visit, obs_reset_audio_monitoring, obs_set_audio_monitoring_device,
};
use proptest::prelude::*;

type SetFn = unsafe extern "C" fn(*const c_char, *const c_char) -> bool;
type ResetFn = unsafe extern "C" fn();

unsafe extern "C" {
    fn oracle_audio_monitoring_device_set_probe(
        candidate: SetFn,
        name: *const c_char,
        id: *const c_char,
        available: bool,
        stored_id: *const c_char,
        monitors: usize,
        flags: *const u32,
        device_ids: *const *const c_char,
        sources: usize,
    ) -> u32;
    fn oracle_audio_monitoring_device_reset_probe(
        candidate: ResetFn,
        available: bool,
        monitors: usize,
    ) -> u32;
    fn oracle_audio_monitoring_device_set_original(
        name: *const c_char,
        id: *const c_char,
        available: bool,
        stored_id: *const c_char,
        monitors: usize,
        flags: *const u32,
        device_ids: *const *const c_char,
        sources: usize,
        trace: *mut c_char,
        trace_size: usize,
        stored: *mut c_char,
        stored_size: usize,
    ) -> bool;
    fn obs_audio_monitoring_device_available() -> bool;
    fn obs_audio_monitoring_device_lock();
    fn obs_audio_monitoring_device_unlock();
    fn obs_audio_monitoring_device_compare(id: *const c_char) -> c_int;
    fn obs_audio_monitoring_device_store(name: *const c_char, id: *const c_char);
    fn obs_audio_monitoring_device_monitor_count() -> usize;
    fn obs_audio_monitoring_device_reset_monitor(index: usize);
    fn obs_audio_monitoring_device_enum_sources(visit: Visit, param: *mut c_void);
    fn obs_audio_monitoring_device_output_flags(source: *const c_void) -> u32;
    fn obs_audio_monitoring_device_notify(source: *mut c_void);
}

const AUDIO: u32 = 1 << 1;
const AOC: u32 = AUDIO | DO_NOT_SELF_MONITOR;

/// One fixture: the arguments, availability, the stored id, the monitor
/// count and the sources (flags and `device_id`).
#[derive(Clone, Debug)]
struct Case {
    name: Option<String>,
    id: Option<String>,
    available: bool,
    stored_id: String,
    monitors: usize,
    sources: Vec<(u32, String)>,
}

impl Case {
    fn new(name: Option<&str>, id: Option<&str>) -> Self {
        Self {
            name: name.map(str::to_owned),
            id: id.map(str::to_owned),
            available: true,
            stored_id: "old".into(),
            monitors: 2,
            sources: vec![
                (AOC, "old.monitor".into()),
                (AUDIO, "new.monitor".into()),
                (AOC, "new.monitor".into()),
            ],
        }
    }

    /// Runs `body` with the C strings and arrays the oracle takes.
    fn with<R>(
        &self,
        body: impl FnOnce(*const c_char, *const c_char, *const c_char, &[u32], &[*const c_char]) -> R,
    ) -> R {
        let c = |text: &str| CString::new(text).unwrap();
        let name = self.name.as_deref().map(c);
        let id = self.id.as_deref().map(c);
        let stored = c(&self.stored_id);
        let flags: Vec<u32> = self.sources.iter().map(|(flags, _)| *flags).collect();
        let ids: Vec<CString> = self.sources.iter().map(|(_, id)| c(id)).collect();
        let id_ptrs: Vec<*const c_char> = ids.iter().map(|id| id.as_ptr()).collect();
        body(
            name.as_ref().map_or(null(), |name| name.as_ptr()),
            id.as_ref().map_or(null(), |id| id.as_ptr()),
            stored.as_ptr(),
            &flags,
            &id_ptrs,
        )
    }

    fn probe(&self, candidate: SetFn) -> u32 {
        self.with(|name, id, stored, flags, ids| {
            // SAFETY: every string is NUL-terminated or NULL and outlives
            // the call; `flags` and `ids` hold `sources` entries; the oracle
            // owns the fixture core every adapter reaches.
            unsafe {
                oracle_audio_monitoring_device_set_probe(
                    candidate,
                    name,
                    id,
                    self.available,
                    stored,
                    self.monitors,
                    flags.as_ptr(),
                    ids.as_ptr(),
                    flags.len(),
                )
            }
        })
    }

    /// The original's trace without the core reads, its result and the
    /// stored `name/id`.
    fn original(&self) -> (String, bool, String) {
        let mut trace = [0 as c_char; 512];
        let mut stored = [0 as c_char; 128];
        let result = self.with(|name, id, stored_id, flags, ids| {
            // SAFETY: as in `probe`; both buffers are writable for their
            // sizes and the oracle writes NUL-terminated text.
            unsafe {
                oracle_audio_monitoring_device_set_original(
                    name,
                    id,
                    self.available,
                    stored_id,
                    self.monitors,
                    flags.as_ptr(),
                    ids.as_ptr(),
                    flags.len(),
                    trace.as_mut_ptr(),
                    trace.len(),
                    stored.as_mut_ptr(),
                    stored.len(),
                )
            }
        });
        // SAFETY: NUL-terminated by the oracle.
        let text = |buffer: &[c_char]| {
            unsafe { CStr::from_ptr(buffer.as_ptr()) }
                .to_str()
                .unwrap()
                .to_owned()
        };
        (text(&trace).replace('o', ""), result, text(&stored))
    }
}

fn reset_probe(candidate: ResetFn, available: bool, monitors: usize) -> u32 {
    // SAFETY: the oracle owns the fixture core every adapter reaches.
    unsafe { oracle_audio_monitoring_device_reset_probe(candidate, available, monitors) }
}

#[test]
fn original_matches_the_characterization() {
    // NULL or empty name or id: nothing else is asked.
    for (name, id) in [
        (None, Some("new")),
        (Some("New"), None),
        (Some(""), Some("new")),
        (Some("New"), Some("")),
    ] {
        assert_eq!(
            Case::new(name, id).original(),
            (String::new(), false, "Old name/old".into())
        );
    }
    // Unavailable: one check.
    let mut case = Case::new(Some("New"), Some("new"));
    case.available = false;
    assert_eq!(case.original(), ("A".into(), false, "Old name/old".into()));
    // The same id: lock, compare, unlock.
    assert_eq!(
        Case::new(Some("New"), Some("old")).original(),
        ("ALCU".into(), true, "Old name/old".into())
    );
    // A new id: copies under the lock, both monitors reset under their own
    // lock, then the two capture sources told, the plain one only asked.
    assert_eq!(
        Case::new(Some("New"), Some("new")).original(),
        (
            "ALCFnFiSSUALM0M1UEg0s0N0r0g2s2N2r2".into(),
            true,
            "New/new".into()
        )
    );
}

fn names() -> impl Strategy<Value = Option<String>> {
    prop_oneof![
        1 => Just(None),
        1 => Just(Some(String::new())),
        8 => "[a-c]{1,6}".prop_map(Some),
    ]
}

fn cases() -> impl Strategy<Value = Case> {
    (
        names(),
        names(),
        any::<bool>(),
        "[a-c]{1,6}",
        0..=8usize,
        prop::collection::vec(
            (
                prop_oneof![Just(0), Just(AUDIO), Just(AOC), Just(DO_NOT_SELF_MONITOR)],
                "[a-c]{0,6}",
            ),
            0..=8,
        ),
    )
        .prop_map(|(name, id, available, stored_id, monitors, sources)| Case {
            name,
            id,
            available,
            stored_id,
            monitors,
            sources,
        })
}

#[test]
fn characterized_edges() {
    for name in [None, Some(""), Some("New")] {
        for id in [None, Some(""), Some("old"), Some("new")] {
            for available in [false, true] {
                for monitors in [0, 1, 8] {
                    let mut case = Case::new(name, id);
                    case.available = available;
                    case.monitors = monitors;
                    assert_eq!(case.probe(obs_set_audio_monitoring_device), 0, "{case:?}");
                }
            }
        }
    }
    for available in [false, true] {
        for monitors in 0..=8 {
            assert_eq!(
                reset_probe(obs_reset_audio_monitoring, available, monitors),
                0,
                "{available} {monitors}"
            );
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(512), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn any_device_matches_c(case in cases()) {
        prop_assert_eq!(case.probe(obs_set_audio_monitoring_device), 0, "{:?}", case);
    }

    #[test]
    fn any_reset_matches_c(available in any::<bool>(), monitors in 0..=8usize) {
        prop_assert_eq!(reset_probe(obs_reset_audio_monitoring, available, monitors), 0);
    }
}

// ---------------------------------------------------------------------------
// mutants, over the same adapters

/// The original's steps, with switches for each broken contract.
#[derive(Clone, Copy, Default)]
struct Breaks {
    available_first: bool,
    no_same_shortcut: bool,
    same_is_false: bool,
    reset_before_unlock: bool,
    no_reset: bool,
    notify_all: bool,
    stop_after_first: bool,
    reset_without_check: bool,
    count_once_less: bool,
}

thread_local! {
    static BREAKS: Cell<Breaks> = Cell::new(Breaks::default());
}

fn breaks() -> Breaks {
    BREAKS.with(Cell::get)
}

unsafe extern "C" fn mutant_reset() {
    let b = breaks();
    // SAFETY: the probe's fixture core; each adapter is used as the shim
    // does.
    unsafe {
        if !b.reset_without_check && !obs_audio_monitoring_device_available() {
            return;
        }
        obs_audio_monitoring_device_lock();
        let mut index = 0;
        while index
            < obs_audio_monitoring_device_monitor_count()
                .saturating_sub(usize::from(b.count_once_less))
        {
            obs_audio_monitoring_device_reset_monitor(index);
            index += 1;
        }
        obs_audio_monitoring_device_unlock();
    }
}

unsafe extern "C" fn mutant_visit(_param: *mut c_void, source: *mut c_void) -> bool {
    let b = breaks();
    // SAFETY: a fixture source handed over by the enumeration.
    unsafe {
        if b.notify_all
            || obs_audio_monitoring_device_output_flags(source) & DO_NOT_SELF_MONITOR != 0
        {
            obs_audio_monitoring_device_notify(source);
        }
    }
    !b.stop_after_first
}

unsafe extern "C" fn mutant_set(name: *const c_char, id: *const c_char) -> bool {
    let b = breaks();
    // SAFETY: the probe's strings and fixture core; each adapter is used as
    // the shim does.
    unsafe {
        if b.available_first && !obs_audio_monitoring_device_available() {
            return false;
        }
        if name.is_null() || id.is_null() || *name == 0 || *id == 0 {
            return false;
        }
        if !b.available_first && !obs_audio_monitoring_device_available() {
            return false;
        }
        obs_audio_monitoring_device_lock();
        if !b.no_same_shortcut && obs_audio_monitoring_device_compare(id) == 0 {
            obs_audio_monitoring_device_unlock();
            return !b.same_is_false;
        }
        obs_audio_monitoring_device_store(name, id);
        if b.reset_before_unlock {
            mutant_reset();
            obs_audio_monitoring_device_unlock();
        } else {
            obs_audio_monitoring_device_unlock();
            if !b.no_reset {
                mutant_reset();
            }
        }
        obs_audio_monitoring_device_enum_sources(mutant_visit, core::ptr::null_mut());
        true
    }
}

#[test]
fn unbroken_mutant_matches() {
    BREAKS.with(|cell| cell.set(Breaks::default()));
    for name in [None, Some(""), Some("New")] {
        for id in [None, Some(""), Some("old"), Some("new")] {
            assert_eq!(Case::new(name, id).probe(mutant_set), 0);
        }
    }
    assert_eq!(reset_probe(mutant_reset, true, 3), 0);
}

#[test]
fn mutants_are_caught() {
    let set_mutants: [(&str, Breaks); 8] = [
        (
            "availability before the name and id",
            Breaks {
                available_first: true,
                ..Breaks::default()
            },
        ),
        (
            "no same-id shortcut",
            Breaks {
                no_same_shortcut: true,
                ..Breaks::default()
            },
        ),
        (
            "same id answers false",
            Breaks {
                same_is_false: true,
                ..Breaks::default()
            },
        ),
        (
            "resets before the unlock",
            Breaks {
                reset_before_unlock: true,
                ..Breaks::default()
            },
        ),
        (
            "no resets",
            Breaks {
                no_reset: true,
                ..Breaks::default()
            },
        ),
        (
            "every source told",
            Breaks {
                notify_all: true,
                ..Breaks::default()
            },
        ),
        (
            "enumeration stopped after one source",
            Breaks {
                stop_after_first: true,
                ..Breaks::default()
            },
        ),
        (
            "one monitor fewer",
            Breaks {
                count_once_less: true,
                ..Breaks::default()
            },
        ),
    ];
    let fixtures = [
        Case::new(None, Some("new")),
        Case::new(Some(""), Some("new")),
        Case::new(Some("New"), Some("old")),
        Case::new(Some("New"), Some("new")),
    ];
    for (label, b) in set_mutants {
        BREAKS.with(|cell| cell.set(b));
        let caught = fixtures.iter().any(|case| case.probe(mutant_set) != 0);
        assert!(caught, "mutant not caught: {label}");
    }
    let reset_mutants: [(&str, Breaks); 2] = [
        (
            "reset without the availability check",
            Breaks {
                reset_without_check: true,
                ..Breaks::default()
            },
        ),
        (
            "reset one monitor fewer",
            Breaks {
                count_once_less: true,
                ..Breaks::default()
            },
        ),
    ];
    for (label, b) in reset_mutants {
        BREAKS.with(|cell| cell.set(b));
        let caught = [false, true]
            .iter()
            .any(|&available| reset_probe(mutant_reset, available, 2) != 0);
        assert!(caught, "mutant not caught: {label}");
    }
    BREAKS.with(|cell| cell.set(Breaks::default()));
}

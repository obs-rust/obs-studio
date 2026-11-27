//! Tier 3: the original C `obs_source_audio_output_capture_device_changed()`
//! and its activate handler, and the Rust exports, on noise-filled
//! real-header sources and a stand-in core (oracle/
//! source_output_capture_device.c). Both reach the same recording stand-ins
//! for every call (`devices_match()`, the host default device, `bfree()`, the
//! signals, the `activate` connection, the calldata and the settings); the
//! Rust side reaches them through the production adapters. Compares every
//! call in order with its arguments, the log lines and every byte of the
//! sources, over a grid of every input, plus a proptest; and rejects
//! mutants. No intentional differences. Excluded: a NULL source with a
//! monitoring device set, which both dereference.
use core::ffi::{CStr, c_char, c_void};
use core::ptr::NonNull;
use obs_source::ffi::source_output_capture_device::{
    Id, Libobs, Ptr, obs_source_audio_output_capture_device_activated,
    obs_source_audio_output_capture_device_changed,
};
use obs_source::output_capture_device::{DO_NOT_SELF_MONITOR, Device};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

const NODES: usize = 3;
const STRINGS: i8 = 4;
const EVENTS: usize = 12;
const NULL: i64 = 0xfe;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct Case {
    which: u8,
    source: i8,
    noise: u8,
    monitoring_set: bool,
    device_id: i8,
    monitoring_id: i8,
    duplicating: i8,
    host: bool,
    host_def: i8,
    matches: bool,
    settings_id: i8,
    flags: [u32; NODES],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Outcome {
    events: u32,
    trace: [[i64; 4]; EVENTS],
    log_count: u32,
    log_level: i32,
    log: [c_char; 256],
    untouched: bool,
}

impl Default for Outcome {
    fn default() -> Self {
        Self {
            events: 0,
            trace: [[0; 4]; EVENTS],
            log_count: 0,
            log_level: 0,
            log: [0; 256],
            untouched: false,
        }
    }
}

impl Outcome {
    fn log(&self) -> &str {
        // SAFETY: the oracle wrote a NUL-terminated string (or left zeros).
        unsafe { CStr::from_ptr(self.log.as_ptr()) }
            .to_str()
            .unwrap()
    }

    /// The calls' kinds, in order.
    fn kinds(&self) -> String {
        self.trace
            .iter()
            .take(usize::try_from(self.events).unwrap())
            .map(|e| char::from(u8::try_from(e[0]).unwrap()))
            .collect()
    }
}

type Changed = unsafe extern "C" fn(*mut c_void, *const c_char);
type Activated = unsafe extern "C" fn(*mut c_void, *mut c_void);
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);

unsafe extern "C" {
    fn oracle_source_output_capture_device_originals(
        changed: *mut Option<Changed>,
        activated: *mut Option<Activated>,
    );
    fn oracle_source_output_capture_device_run(
        changed: Changed,
        activated: Activated,
        get: GetLog,
        set: SetLog,
        case: *const Case,
        out: *mut Outcome,
    ) -> bool;
}

fn originals() -> (Changed, Activated) {
    let mut changed = None;
    let mut activated = None;
    // SAFETY: two writable slots for function pointers.
    unsafe { oracle_source_output_capture_device_originals(&raw mut changed, &raw mut activated) };
    (changed.unwrap(), activated.unwrap())
}

/// Runs one side; panics when the oracle could not build the fixture or an
/// adapter does not reach its field.
fn run(changed: Changed, activated: Activated, case: &Case) -> Outcome {
    let mut out = Outcome::default();
    // SAFETY: the isolated child owns the log handler; the case and the
    // outcome are valid for the call.
    let valid = unsafe {
        oracle_source_output_capture_device_run(
            changed,
            activated,
            base_get_log_handler,
            base_set_log_handler,
            case,
            &mut out,
        )
    };
    assert!(valid, "the oracle fixture or an adapter failed: {case:?}");
    out
}

/// The original's outcome, when the given calls agree with it.
fn check_with(changed: Changed, activated: Activated, case: &Case) -> Option<Outcome> {
    let (original_changed, original_activated) = originals();
    let original = run(original_changed, original_activated, case);
    let ported = run(changed, activated, case);
    (ported == original).then_some(original)
}

fn check(case: &Case) -> Outcome {
    check_with(
        obs_source_audio_output_capture_device_changed,
        obs_source_audio_output_capture_device_activated,
        case,
    )
    .unwrap_or_else(|| panic!("the Rust exports disagree with the original C: {case:?}"))
}

/// Node 0 captures an output device, "sink_a" (string 0) is monitored, the
/// host declines, nobody deduplicates; node 0's settings name "sink_a".
fn base(which: u8) -> Case {
    Case {
        which,
        source: 0,
        noise: 0xa5,
        monitoring_set: true,
        device_id: 0,
        monitoring_id: 0,
        duplicating: -1,
        host: false,
        host_def: -1,
        matches: true,
        settings_id: 0,
        flags: [DO_NOT_SELF_MONITOR, 0, DO_NOT_SELF_MONITOR | 1],
    }
}

// --- mutants ------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Change {
    IgnoreMonitoringSet,
    IgnoreFlags,
    WrongFlag,
    SourceIdOverHost,
    NoFree,
    SignalAfterConnect,
    StopAnySource,
    SignalBeforeDisconnect,
    NoLogStarted,
    ReleaseBeforeCheck,
    MatchSwapped,
}

fn mutant_changed(change: Change, source: *mut c_void, device_id: *const c_char) {
    let source = Ptr(source);
    let mut d = Libobs::default();
    let id = |p: *const c_char| (!p.is_null()).then_some(p);
    // Rebuild the core's steps with one change, through the real adapters.
    if change != Change::IgnoreMonitoringSet && !d.monitoring_device_set() {
        return;
    }
    let flag = if change == Change::WrongFlag {
        DO_NOT_SELF_MONITOR << 1
    } else {
        DO_NOT_SELF_MONITOR
    };
    if change != Change::IgnoreFlags && d.output_flags(source) & flag == 0 {
        return;
    }
    let monitoring_id = d.monitoring_id();
    let own = ids(id(device_id));
    let id_match = match d.host_default(own) {
        Some(def_id) if change != Change::SourceIdOverHost => {
            let m = if change == Change::MatchSwapped {
                d.devices_match(monitoring_id, def_id)
            } else {
                d.devices_match(def_id, monitoring_id)
            };
            if let (Some(def_id), false) = (def_id, change == Change::NoFree) {
                d.free_default(def_id);
            }
            m
        }
        _ => d.devices_match(own, monitoring_id),
    };
    if id_match {
        if change == Change::SignalAfterConnect {
            d.connect_activate(source);
            d.signal_deduplication(Some(source));
        } else {
            d.signal_deduplication(Some(source));
            d.connect_activate(source);
        }
        if change != Change::NoLogStarted {
            d.log_started(source);
        }
    } else if change == Change::StopAnySource || d.duplicating_source() == Some(source) {
        if change == Change::SignalBeforeDisconnect {
            d.signal_deduplication(None);
            d.disconnect_activate(source);
        } else {
            d.disconnect_activate(source);
            d.signal_deduplication(None);
        }
        d.log_stopped();
    }
}

/// A C id (NULL is `None`) as the shim's `Id`.
fn ids(p: Option<*const c_char>) -> Option<Id> {
    p.and_then(|p| NonNull::new(p.cast_mut())).map(Id::from_raw)
}

fn mutant_activated(change: Change, cd: *mut c_void) {
    let mut d = Libobs::default();
    let Some(source) = d.calldata_source(Ptr(cd)) else {
        return;
    };
    let settings = d.settings(source);
    if change == Change::ReleaseBeforeCheck {
        d.release_settings(settings);
    }
    let device_id = d.settings_device_id(settings);
    let raw = device_id.map_or(core::ptr::null(), |id| id.as_ptr());
    mutant_changed(change, source.0, raw);
    if change != Change::ReleaseBeforeCheck {
        d.release_settings(settings);
    }
}

macro_rules! mutant {
    ($changed:ident, $activated:ident, $change:expr) => {
        unsafe extern "C" fn $changed(source: *mut c_void, device_id: *const c_char) {
            mutant_changed($change, source, device_id);
        }
        unsafe extern "C" fn $activated(_data: *mut c_void, cd: *mut c_void) {
            mutant_activated($change, cd);
        }
    };
}

mutant!(c_monitoring, a_monitoring, Change::IgnoreMonitoringSet);
mutant!(c_flags, a_flags, Change::IgnoreFlags);
mutant!(c_wrong_flag, a_wrong_flag, Change::WrongFlag);
mutant!(c_source_id, a_source_id, Change::SourceIdOverHost);
mutant!(c_no_free, a_no_free, Change::NoFree);
mutant!(c_signal_after, a_signal_after, Change::SignalAfterConnect);
mutant!(c_stop_any, a_stop_any, Change::StopAnySource);
mutant!(
    c_signal_before,
    a_signal_before,
    Change::SignalBeforeDisconnect
);
mutant!(c_no_log, a_no_log, Change::NoLogStarted);
mutant!(c_release_first, a_release_first, Change::ReleaseBeforeCheck);
mutant!(c_swapped, a_swapped, Change::MatchSwapped);

// --- cases --------------------------------------------------------------

fn pinned() {
    // A match: host asked, match, signal with node 0, connect of node 0's
    // activate to the side's own handler, the started line.
    let out = check(&base(0));
    assert_eq!(out.kinds(), "HMSC");
    assert_eq!(out.trace[0][1..3], [0, 1]);
    assert_eq!(out.trace[1][1..3], [0, 0]);
    assert_eq!(out.trace[2][1..4], [0, 1, 0]);
    assert_eq!(out.trace[3][1..4], [1, 1, 1]);
    assert_eq!((out.log_count, out.log_level), (1, 300));
    assert_eq!(
        out.log(),
        "Device for 'Audio Output Capture' source node 0 is also used for audio \
         monitoring.\nDeduplication logic is being applied to all monitored sources."
    );
    assert!(out.untouched);

    // No monitoring device, or no flag: nothing.
    let mut c = base(0);
    c.monitoring_set = false;
    assert_eq!(check(&c).events, 0);
    let mut c = base(0);
    c.source = 1;
    let out = check(&c);
    assert_eq!((out.events, out.log_count), (0, 0));

    // No match, node 0 deduplicates: disconnect, then the NULL signal, then
    // the stopped line.
    let mut c = base(0);
    c.matches = false;
    c.duplicating = 0;
    let out = check(&c);
    assert_eq!(out.kinds(), "HMDS");
    assert_eq!(out.trace[3][3], NULL);
    assert_eq!(out.log(), "Deduplication logic stopped.");
    c.duplicating = 2;
    let out = check(&c);
    assert_eq!((out.kinds().as_str(), out.log_count), ("HM", 0));

    // The host answers: its id is matched, then freed.
    let mut c = base(0);
    c.host = true;
    c.host_def = 3;
    c.device_id = 2;
    let out = check(&c);
    assert_eq!(out.kinds(), "HMFSC");
    assert_eq!(out.trace[1][1..3], [3, 0]);
    assert_eq!(out.trace[2][1], 3);
    c.host_def = -1;
    assert_eq!(check(&c).kinds(), "HMSC");

    // The handler: the calldata's source, its settings, the check, release.
    let out = check(&base(1));
    assert_eq!(out.kinds(), "PGTHMSCR");
    let mut c = base(1);
    c.source = -1;
    assert_eq!(check(&c).kinds(), "P");
}

fn grid() -> Vec<Case> {
    let mut cases = Vec::new();
    for which in 0..2u8 {
        for source in [-1i8, 0, 1, 2] {
            for monitoring_set in [false, true] {
                for (device_id, settings_id) in [(-1i8, 1i8), (0, -1), (2, 3)] {
                    for monitoring_id in [-1i8, 0] {
                        for duplicating in [-1i8, 0, 2] {
                            for (host, host_def) in [(false, 0i8), (true, -1), (true, 3)] {
                                for matches in [false, true] {
                                    // A NULL source with a monitoring device
                                    // set faults on both sides.
                                    if which == 0 && source < 0 && monitoring_set {
                                        continue;
                                    }
                                    let mut c = base(which);
                                    c.source = source;
                                    c.monitoring_set = monitoring_set;
                                    c.device_id = device_id;
                                    c.settings_id = settings_id;
                                    c.monitoring_id = monitoring_id;
                                    c.duplicating = duplicating;
                                    c.host = host;
                                    c.host_def = host_def;
                                    c.matches = matches;
                                    cases.push(c);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    cases
}

fn grid_and_mutants() {
    let cases = grid();
    for c in &cases {
        assert!(check(c).untouched);
    }
    let mutants: [(&str, Changed, Activated); 11] = [
        ("no monitoring check", c_monitoring, a_monitoring),
        ("no flag check", c_flags, a_flags),
        ("wrong flag", c_wrong_flag, a_wrong_flag),
        ("source id over the host's", c_source_id, a_source_id),
        ("host id not freed", c_no_free, a_no_free),
        ("connect before the signal", c_signal_after, a_signal_after),
        ("any source stops", c_stop_any, a_stop_any),
        (
            "signal before the disconnect",
            c_signal_before,
            a_signal_before,
        ),
        ("no started line", c_no_log, a_no_log),
        ("settings released first", c_release_first, a_release_first),
        ("match arguments swapped", c_swapped, a_swapped),
    ];
    for (name, changed, activated) in mutants {
        assert!(
            cases
                .iter()
                .filter(|c| !(c.which == 0 && c.source < 0))
                .any(|c| check_with(changed, activated, c).is_none()),
            "the mutant '{name}' agreed with the original C"
        );
    }
}

fn random_cases() {
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases: obs_c_oracle::parity::cases(512),
        failure_persistence: None,
        ..ProptestConfig::default()
    });
    let string = -1i8..STRINGS;
    runner
        .run(
            &(
                (0u8..2, 0i8..3, any::<u8>(), any::<bool>()),
                (string.clone(), string.clone(), -1i8..3),
                (any::<bool>(), string.clone(), any::<bool>(), string),
                proptest::array::uniform3(any::<u32>()),
            ),
            |(
                (which, source, noise, monitoring_set),
                (device_id, monitoring_id, duplicating),
                (host, host_def, matches, settings_id),
                flags,
            )| {
                let c = Case {
                    which,
                    source,
                    noise,
                    monitoring_set,
                    device_id,
                    monitoring_id,
                    duplicating,
                    host,
                    host_def,
                    matches,
                    settings_id,
                    flags,
                };
                let out = check(&c);
                prop_assert!(out.untouched);
                prop_assert!(out.log_count <= 1);
                Ok(())
            },
        )
        .unwrap();
}

#[test]
fn output_capture_device_matches_c() {
    if !crate::output::isolated(concat!(module_path!(), "::output_capture_device_matches_c")) {
        return;
    }
    pinned();
    grid_and_mutants();
    random_cases();
}

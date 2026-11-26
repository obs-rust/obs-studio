//! Tier 3: the original C `obs_source_show_preloaded_video()` against the
//! Rust export, each on its own real private source
//! (`source_show_preloaded_video.c`). Both run over the same production
//! adapters, with the two mutexes, the graphics context, the texture upload,
//! the flag stores, the clock and the audio resets recorded, and the same
//! source state; the recorded calls with their arguments, the diagnostic and
//! the state left (both flags, the preload frame) must match. Four mutants
//! of the export, built on the same adapters, must not. No intentional
//! differences or exclusions.
use core::ffi::{c_char, c_int, c_void};
use obs_source::ffi::source_show_preloaded_video::obs_source_show_preloaded_video as rust_show;
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
type Show = unsafe extern "C" fn(*mut c_void);

unsafe extern "C" {
    fn oracle_obs_source_show_preloaded_video(source: *mut c_void);
    fn oracle_spv_events(
        kinds: *mut c_int,
        ptrs: *mut *const c_void,
        nums: *mut u64,
        max: usize,
    ) -> usize;
    fn oracle_spv_set_clock(now: u64);
    fn oracle_spv_source_new(
        destroying: bool,
        preload: bool,
        monitoring_type: c_int,
        last_frame_ts: u64,
        stale: bool,
        active: bool,
    ) -> *mut c_void;
    fn oracle_spv_source_free(source: *mut c_void);
    fn oracle_spv_addresses(source: *const c_void, out: *mut *const c_void);
    fn oracle_spv_state(
        source: *const c_void,
        stale: *mut bool,
        active: *mut bool,
        preload: *mut *const c_void,
    );
    fn oracle_spv_log_begin(get: GetLog, set: SetLog);
    fn oracle_spv_log_end(set: SetLog, level: *mut c_int, message: *mut c_char, size: usize)
    -> u32;

    // The production adapters, for the mutants.
    fn obs_source_caption_callback_destroying(source: *const c_void) -> bool;
    fn obs_source_preloaded_video_has_frame(source: *const c_void) -> bool;
    fn obs_source_preloaded_video_enter_graphics();
    fn obs_source_preloaded_video_set_texture_size(source: *mut c_void);
    fn obs_source_preloaded_video_update_textures(source: *mut c_void);
    fn obs_source_preloaded_video_mark_fresh(source: *mut c_void);
    fn obs_source_preloaded_video_mark_active(source: *mut c_void);
    fn obs_source_preloaded_video_leave_graphics();
    fn obs_source_preloaded_video_lock_async(source: *mut c_void);
    fn obs_source_preloaded_video_last_frame_ts(source: *const c_void) -> u64;
    fn obs_source_preloaded_video_unlock_async(source: *mut c_void);
    fn obs_source_preloaded_video_lock_audio(source: *mut c_void);
    fn obs_source_preloaded_video_monitor_only(source: *const c_void) -> bool;
    fn obs_source_preloaded_video_now() -> u64;
    fn obs_source_preloaded_video_reset_timing(source: *mut c_void, timestamp: u64, os_time: u64);
    fn obs_source_preloaded_video_reset_data(source: *mut c_void, os_time: u64);
    fn obs_source_preloaded_video_unlock_audio(source: *mut c_void);
}

/// `OBS_MONITORING_TYPE_MONITOR_ONLY`.
const MONITOR_ONLY: c_int = 1;

#[derive(Debug, Clone, Copy)]
struct Case {
    null_source: bool,
    destroying: bool,
    preload: bool,
    monitoring_type: c_int,
    last_frame_ts: u64,
    clock: u64,
    stale: bool,
    active: bool,
}

/// A pointer the run knows by role, or another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ptr {
    Null,
    Source,
    AsyncMutex,
    AudioMutex,
    Frame,
    Textures,
    Texrender,
    StaleFlag,
    ActiveFlag,
    Unknown,
}

type Event = (c_int, [Ptr; 3], [u64; 2]);

#[derive(Debug, Clone, PartialEq, Eq)]
struct Outcome {
    events: Vec<Event>,
    log: (u32, c_int, Vec<u8>),
    /// (stale, active, preload frame) after the call.
    state: Option<(bool, bool, Ptr)>,
}

/// The `update_async_textures()` event carries the texrender as a number.
const UPDATE: c_int = 6;

fn run(case: Case, show: Show) -> Outcome {
    // SAFETY: the oracle allocates a real source with both mutexes
    // initialized and frees it below; the address and event buffers are
    // the sizes the oracle writes.
    unsafe {
        let source = oracle_spv_source_new(
            case.destroying,
            case.preload,
            case.monitoring_type,
            case.last_frame_ts,
            case.stale,
            case.active,
        );
        assert!(!source.is_null());
        let mut addresses = [core::ptr::null::<c_void>(); 7];
        oracle_spv_addresses(source, addresses.as_mut_ptr());
        let name = |ptr: *const c_void| -> Ptr {
            if ptr.is_null() {
                return Ptr::Null;
            }
            if ptr == source.cast_const() {
                return Ptr::Source;
            }
            let roles = [
                Ptr::AsyncMutex,
                Ptr::AudioMutex,
                Ptr::Frame,
                Ptr::Textures,
                Ptr::Texrender,
                Ptr::StaleFlag,
                Ptr::ActiveFlag,
            ];
            addresses
                .iter()
                .position(|&a| a == ptr)
                .map_or(Ptr::Unknown, |i| roles[i])
        };

        oracle_spv_events(
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            0,
        );
        oracle_spv_set_clock(case.clock);
        oracle_spv_log_begin(base_get_log_handler, base_set_log_handler);
        show(if case.null_source {
            core::ptr::null_mut()
        } else {
            source
        });
        let mut level = 0;
        let mut message = [0 as c_char; 256];
        let count = oracle_spv_log_end(
            base_set_log_handler,
            &raw mut level,
            message.as_mut_ptr(),
            message.len(),
        );
        let text = core::ffi::CStr::from_ptr(message.as_ptr())
            .to_bytes()
            .to_vec();

        let mut kinds = [0 as c_int; 64];
        let mut ptrs = [core::ptr::null::<c_void>(); 64 * 3];
        let mut nums = [0_u64; 64 * 2];
        let n = oracle_spv_events(
            kinds.as_mut_ptr(),
            ptrs.as_mut_ptr(),
            nums.as_mut_ptr(),
            kinds.len(),
        );
        assert!(n <= kinds.len(), "event log overflowed");
        let events = (0..n)
            .map(|i| {
                let mut a = nums[i * 2];
                if kinds[i] == UPDATE {
                    // The texrender, as a pointer the run knows.
                    a = name(a as usize as *const c_void) as u64;
                }
                (
                    kinds[i],
                    [
                        name(ptrs[i * 3]),
                        name(ptrs[i * 3 + 1]),
                        name(ptrs[i * 3 + 2]),
                    ],
                    [a, nums[i * 2 + 1]],
                )
            })
            .collect();

        let mut stale = false;
        let mut active = false;
        let mut preload = core::ptr::null();
        oracle_spv_state(source, &mut stale, &mut active, &mut preload);
        let state = Some((stale, active, name(preload)));
        oracle_spv_source_free(source);
        Outcome {
            events,
            log: (count, level, text),
            state,
        }
    }
}

unsafe extern "C" fn c_show(source: *mut c_void) {
    // SAFETY: the caller's NULL or live source.
    unsafe { oracle_obs_source_show_preloaded_video(source) }
}

unsafe extern "C" fn r_show(source: *mut c_void) {
    // SAFETY: the caller's NULL or live source.
    unsafe { rust_show(source) }
}

/// The export's sequence on the adapters, with one change each.
#[derive(Clone, Copy)]
enum Mutant {
    /// Ignores `destroying()`.
    NoDestroyingCheck,
    /// Reads the clock for a monitor-only source too.
    AlwaysReadClock,
    /// Reads `last_frame_ts` without `async_mutex`.
    UnlockedLastFrame,
    /// Marks the source active before the texture update.
    ActiveFirst,
}

fn mutant_show(mutant: Mutant, source: *mut c_void) {
    if source.is_null() {
        // SAFETY: static strings and the core's own diagnostic.
        unsafe { rust_show(source) };
        return;
    }
    // SAFETY: a live source from the oracle; the adapters pair as the
    // export pairs them.
    unsafe {
        if !matches!(mutant, Mutant::NoDestroyingCheck)
            && obs_source_caption_callback_destroying(source)
        {
            return;
        }
        if !obs_source_preloaded_video_has_frame(source) {
            return;
        }
        obs_source_preloaded_video_enter_graphics();
        if matches!(mutant, Mutant::ActiveFirst) {
            obs_source_preloaded_video_mark_active(source);
        }
        obs_source_preloaded_video_set_texture_size(source);
        obs_source_preloaded_video_update_textures(source);
        obs_source_preloaded_video_mark_fresh(source);
        if !matches!(mutant, Mutant::ActiveFirst) {
            obs_source_preloaded_video_mark_active(source);
        }
        obs_source_preloaded_video_leave_graphics();
        let last = if matches!(mutant, Mutant::UnlockedLastFrame) {
            obs_source_preloaded_video_last_frame_ts(source)
        } else {
            obs_source_preloaded_video_lock_async(source);
            let last = obs_source_preloaded_video_last_frame_ts(source);
            obs_source_preloaded_video_unlock_async(source);
            last
        };
        obs_source_preloaded_video_lock_audio(source);
        let monitor_only = obs_source_preloaded_video_monitor_only(source);
        let sys = if monitor_only && !matches!(mutant, Mutant::AlwaysReadClock) {
            0
        } else {
            obs_source_preloaded_video_now()
        };
        obs_source_preloaded_video_reset_timing(source, last, sys);
        obs_source_preloaded_video_reset_data(source, sys);
        obs_source_preloaded_video_unlock_audio(source);
    }
}

macro_rules! mutant_fn {
    ($name:ident, $mutant:expr) => {
        unsafe extern "C" fn $name(source: *mut c_void) {
            mutant_show($mutant, source);
        }
    };
}
mutant_fn!(m_no_destroying, Mutant::NoDestroyingCheck);
mutant_fn!(m_always_clock, Mutant::AlwaysReadClock);
mutant_fn!(m_unlocked_last, Mutant::UnlockedLastFrame);
mutant_fn!(m_active_first, Mutant::ActiveFirst);

/// Serializes the runs: the oracle's event log, clock and log capture are
/// process globals (each test also runs in its own isolated child, which
/// owns the log handler).
static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn compare(case: Case) -> (Outcome, Outcome) {
    let _guard = LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    (run(case, c_show), run(case, r_show))
}

const BASE: Case = Case {
    null_source: false,
    destroying: false,
    preload: true,
    monitoring_type: 0,
    last_frame_ts: 5_000_000,
    clock: 9_000_000,
    stale: true,
    active: false,
};

fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for null_source in [false, true] {
        for destroying in [false, true] {
            for preload in [false, true] {
                for monitoring_type in [-1, 0, 1, 2, 3] {
                    for (stale, active) in [(true, false), (false, true)] {
                        for (last_frame_ts, clock) in
                            [(5_000_000, 9_000_000), (0, 0), (u64::MAX, 1), (1, u64::MAX)]
                        {
                            cases.push(Case {
                                null_source,
                                destroying,
                                preload,
                                monitoring_type,
                                last_frame_ts,
                                clock,
                                stale,
                                active,
                            });
                        }
                    }
                }
            }
        }
    }
    cases
}

#[test]
fn matches_c_over_every_combination() {
    if !crate::output::isolated(concat!(
        module_path!(),
        "::matches_c_over_every_combination"
    )) {
        return;
    }
    for case in cases() {
        let (c, rust) = compare(case);
        assert_eq!(c, rust, "{case:?}");
    }
}

/// The characterized answers, pinned on the C side.
#[test]
fn pins_the_characterized_sequence() {
    if !crate::output::isolated(concat!(module_path!(), "::pins_the_characterized_sequence")) {
        return;
    }
    let (c, rust) = compare(BASE);
    assert_eq!(c, rust);
    let kinds: Vec<c_int> = c.events.iter().map(|e| e.0).collect();
    // lock/unlock 1/2, enter/leave 3/4, size 5, update 6, flag 7, clock 8,
    // timing 9, data 10.
    assert_eq!(kinds, [3, 5, 6, 7, 7, 4, 1, 2, 1, 8, 9, 10, 2]);
    assert_eq!(c.events[1].1, [Ptr::Source, Ptr::Frame, Ptr::Null]);
    assert_eq!(c.events[2].1, [Ptr::Source, Ptr::Frame, Ptr::Textures]);
    assert_eq!(c.events[2].2[0], Ptr::Texrender as u64);
    assert_eq!(
        c.events[3],
        (7, [Ptr::StaleFlag, Ptr::Null, Ptr::Null], [0, 0])
    );
    assert_eq!(
        c.events[4],
        (7, [Ptr::ActiveFlag, Ptr::Null, Ptr::Null], [1, 0])
    );
    assert_eq!(c.events[6].1[0], Ptr::AsyncMutex);
    assert_eq!(c.events[8].1[0], Ptr::AudioMutex);
    assert_eq!(c.events[10].2, [5_000_000, 9_000_000]);
    assert_eq!(c.events[11].2, [9_000_000, 0]);
    assert_eq!(c.state, Some((false, true, Ptr::Frame)));
    assert_eq!(c.log.0, 0);

    let monitor_only = compare(Case {
        monitoring_type: MONITOR_ONLY,
        ..BASE
    });
    assert_eq!(monitor_only.0, monitor_only.1);
    assert!(
        monitor_only.0.events.iter().all(|e| e.0 != 8),
        "no clock read"
    );
    assert!(monitor_only.0.events.contains(&(
        9,
        [Ptr::Source, Ptr::Null, Ptr::Null],
        [5_000_000, 0]
    )));

    let null = compare(Case {
        null_source: true,
        ..BASE
    });
    assert_eq!(null.0, null.1);
    assert!(null.0.events.is_empty());
    assert_eq!(
        null.0.log,
        (
            1,
            400,
            b"obs_source_show_preloaded_video: Null 'source' parameter".to_vec()
        )
    );

    for case in [
        Case {
            destroying: true,
            ..BASE
        },
        Case {
            preload: false,
            ..BASE
        },
    ] {
        let (c, rust) = compare(case);
        assert_eq!(c, rust);
        assert!(c.events.is_empty());
        assert_eq!(c.state.map(|s| (s.0, s.1)), Some((true, false)));
    }
}

/// Each mutant differs from the C on at least one case, so the comparison
/// can see it.
#[test]
fn rejects_mutants() {
    if !crate::output::isolated(concat!(module_path!(), "::rejects_mutants")) {
        return;
    }
    let monitor_only = Case {
        monitoring_type: MONITOR_ONLY,
        ..BASE
    };
    let destroying = Case {
        destroying: true,
        ..BASE
    };
    for (mutant, case) in [
        (m_no_destroying as Show, destroying),
        (m_always_clock as Show, monitor_only),
        (m_unlocked_last as Show, BASE),
        (m_active_first as Show, BASE),
    ] {
        let _guard = LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let c = run(case, c_show);
        let m = run(case, mutant);
        assert_ne!(c, m, "{case:?}");
    }
}

fn random_case() -> impl Strategy<Value = Case> {
    (
        (any::<bool>(), any::<bool>(), any::<bool>(), -3..6_i32),
        (any::<u64>(), any::<u64>(), any::<bool>(), any::<bool>()),
    )
        .prop_map(
            |(
                (null_source, destroying, preload, monitoring_type),
                (last_frame_ts, clock, stale, active),
            )| Case {
                null_source,
                destroying,
                preload,
                monitoring_type,
                last_frame_ts,
                clock,
                stale,
                active,
            },
        )
}

#[test]
fn matches_c_on_random_states() {
    if !crate::output::isolated(concat!(module_path!(), "::matches_c_on_random_states")) {
        return;
    }
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    });
    runner
        .run(&random_case(), |case| {
            let (c, rust) = compare(case);
            prop_assert_eq!(c, rust);
            Ok(())
        })
        .unwrap();
}

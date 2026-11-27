//! Tier 3: the original C `obs_source_preload_video_internal()` against the
//! Rust export, each on its own real private source
//! (`source_preload_video_internal.c`). Both run over the same production
//! adapters, with `destroying()`, the preload frame's create, destroy and
//! copy and the `async_mutex` lock and unlock recorded, and the same source
//! state and frames; every recorded call in order with its arguments, the
//! diagnostic and the state left (the preload frame, its shape,
//! `last_frame_ts`) must match, over single calls and call sequences on one
//! source. Four mutants of the export, built on the same adapters, must not.
//! No intentional differences or exclusions.
use core::ffi::{c_char, c_int, c_void};
use obs_source::ffi::source_frame_copy::obs_source_frame_copy_shape;
use obs_source::ffi::source_preload_video_internal::obs_source_preload_video_internal as rust_preload;
use obs_source::frame_copy::video_format::{BGRA, I420, NV12};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
type Preload = unsafe extern "C" fn(*mut c_void, *const c_void);

unsafe extern "C" {
    fn oracle_obs_source_preload_video_internal(source: *mut c_void, frame: *const c_void);
    fn oracle_pvi_events(
        kinds: *mut c_int,
        ptrs: *mut *const c_void,
        nums: *mut u64,
        max: usize,
    ) -> usize;
    fn oracle_pvi_source_new(
        destroying: bool,
        preload: bool,
        format: c_int,
        width: u32,
        height: u32,
        last_frame_ts: u64,
    ) -> *mut c_void;
    fn oracle_pvi_source_free(source: *mut c_void);
    fn oracle_pvi_addresses(source: *const c_void, out: *mut *const c_void);
    fn oracle_pvi_state(
        source: *const c_void,
        preload: *mut *const c_void,
        format: *mut c_int,
        width: *mut u32,
        height: *mut u32,
        last_frame_ts: *mut u64,
    );
    fn oracle_pvi_frame_new(format: c_int, width: u32, height: u32, timestamp: u64) -> *mut c_void;
    fn oracle_pvi_frame_free(frame: *mut c_void);
    fn oracle_pvi_log_begin(get: GetLog, set: SetLog);
    fn oracle_pvi_log_end(set: SetLog, level: *mut c_int, message: *mut c_char, size: usize)
    -> u32;

    // The production adapters, for the mutants.
    fn obs_source_preload_internal_destroying(source: *const c_void) -> bool;
    fn obs_source_preload_internal_preload_shape(
        source: *const c_void,
        shape: *mut obs_source_frame_copy_shape,
    ) -> bool;
    fn obs_source_preload_internal_frame_shape(frame: *const c_void)
    -> obs_source_frame_copy_shape;
    fn obs_source_preload_internal_destroy_preload(source: *mut c_void);
    fn obs_source_preload_internal_create_preload(source: *mut c_void, frame: *const c_void);
    fn obs_source_preload_internal_copy(source: *mut c_void, frame: *const c_void);
    fn obs_source_preload_internal_lock_async(source: *mut c_void);
    fn obs_source_preload_internal_set_last_frame_ts(source: *mut c_void, frame: *const c_void);
    fn obs_source_preload_internal_unlock_async(source: *mut c_void);
}

/// A frame shape: format, width, height.
type Shape = (c_int, u32, u32);

/// One call: a NULL source, and the frame (shape and timestamp) or NULL.
#[derive(Debug, Clone, Copy)]
struct Call {
    null_source: bool,
    frame: Option<(Shape, u64)>,
}

#[derive(Debug, Clone)]
struct Case {
    destroying: bool,
    preload: Option<Shape>,
    last_frame_ts: u64,
    calls: Vec<Call>,
}

/// A pointer the run knows by role, or another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ptr {
    Null,
    Source,
    AsyncMutex,
    /// The preload frame the source started with.
    Preload,
    /// The frame of call n.
    Frame(usize),
    /// The nth frame the run created.
    Created(usize),
    Unknown,
}

type Event = (c_int, [Ptr; 2], [u64; 3]);
/// The diagnostics of one call: count, last level, last message.
type Log = (u32, c_int, Vec<u8>);
/// Every address the run knows, by role, oldest first.
type Roles = Vec<(*const c_void, Ptr)>;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Outcome {
    /// Each call's events and log.
    calls: Vec<(Vec<Event>, Log)>,
    /// The preload frame, its shape and last_frame_ts after the last call.
    state: (Ptr, Shape, u64),
}

/// `PVI_CREATE`.
const CREATE: c_int = 4;

/// The newest role of `ptr`: a created frame can reuse a destroyed frame's
/// address.
fn name(roles: &[(*const c_void, Ptr)], ptr: *const c_void) -> Ptr {
    if ptr.is_null() {
        return Ptr::Null;
    }
    roles
        .iter()
        .rev()
        .find(|(a, _)| *a == ptr)
        .map_or(Ptr::Unknown, |&(_, role)| role)
}

/// Runs one call with the log captured.
fn logged_call(preload: Preload, source: *mut c_void, frame: *const c_void) -> Log {
    // SAFETY: the caller's NULL or live source and frame; the message
    // buffer is the size passed.
    unsafe {
        oracle_pvi_log_begin(base_get_log_handler, base_set_log_handler);
        preload(source, frame);
        let mut level = 0;
        let mut message = [0 as c_char; 256];
        let count = oracle_pvi_log_end(
            base_set_log_handler,
            &raw mut level,
            message.as_mut_ptr(),
            message.len(),
        );
        let text = core::ffi::CStr::from_ptr(message.as_ptr())
            .to_bytes()
            .to_vec();
        (count, level, text)
    }
}

/// The events since the last read, by role; each created frame gets the
/// next `Created` role.
fn take_events(roles: &mut Roles, created: &mut usize) -> Vec<Event> {
    let mut kinds = [0 as c_int; 64];
    let mut ptrs = [core::ptr::null::<c_void>(); 64 * 2];
    let mut nums = [0_u64; 64 * 3];
    // SAFETY: the buffers are the sizes the oracle writes for `max`.
    let n = unsafe {
        oracle_pvi_events(
            kinds.as_mut_ptr(),
            ptrs.as_mut_ptr(),
            nums.as_mut_ptr(),
            kinds.len(),
        )
    };
    assert!(n <= kinds.len(), "event log overflowed");
    let mut events = Vec::new();
    for i in 0..n {
        if kinds[i] == CREATE {
            roles.push((ptrs[i * 2 + 1], Ptr::Created(*created)));
            *created += 1;
        }
        events.push((
            kinds[i],
            [name(roles, ptrs[i * 2]), name(roles, ptrs[i * 2 + 1])],
            [nums[i * 3], nums[i * 3 + 1], nums[i * 3 + 2]],
        ));
    }
    events
}

fn run(case: &Case, preload: Preload) -> Outcome {
    // SAFETY: the oracle allocates a real source with its async_mutex
    // initialized and each frame, and frees them below; the address
    // buffer is the size the oracle writes.
    unsafe {
        let (has_preload, (format, width, height)) = case
            .preload
            .map_or((false, (0, 0, 0)), |shape| (true, shape));
        let source = oracle_pvi_source_new(
            case.destroying,
            has_preload,
            format,
            width,
            height,
            case.last_frame_ts,
        );
        assert!(!source.is_null());
        let mut addresses = [core::ptr::null::<c_void>(); 2];
        oracle_pvi_addresses(source, addresses.as_mut_ptr());
        let frames: Vec<*mut c_void> = case
            .calls
            .iter()
            .map(|call| {
                call.frame.map_or(core::ptr::null_mut(), |((f, w, h), ts)| {
                    oracle_pvi_frame_new(f, w, h, ts)
                })
            })
            .collect();
        let mut roles: Roles = vec![
            (source.cast_const(), Ptr::Source),
            (addresses[0], Ptr::AsyncMutex),
        ];
        if !addresses[1].is_null() {
            roles.push((addresses[1], Ptr::Preload));
        }
        for (i, &frame) in frames.iter().enumerate() {
            if !frame.is_null() {
                roles.push((frame.cast_const(), Ptr::Frame(i)));
            }
        }

        oracle_pvi_events(
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            0,
        );
        let mut created = 0;
        let mut calls = Vec::new();
        for (call, &frame) in case.calls.iter().zip(&frames) {
            let target = if call.null_source {
                core::ptr::null_mut()
            } else {
                source
            };
            let log = logged_call(preload, target, frame);
            calls.push((take_events(&mut roles, &mut created), log));
        }

        let mut preload = core::ptr::null();
        let mut shape = (0, 0, 0);
        let mut last = 0;
        oracle_pvi_state(
            source,
            &raw mut preload,
            &raw mut shape.0,
            &raw mut shape.1,
            &raw mut shape.2,
            &raw mut last,
        );
        let state = (name(&roles, preload), shape, last);
        for frame in frames {
            if !frame.is_null() {
                oracle_pvi_frame_free(frame);
            }
        }
        oracle_pvi_source_free(source);
        Outcome { calls, state }
    }
}

unsafe extern "C" fn c_preload(source: *mut c_void, frame: *const c_void) {
    // SAFETY: the caller's NULL or live source and frame.
    unsafe { oracle_obs_source_preload_video_internal(source, frame) }
}

unsafe extern "C" fn r_preload(source: *mut c_void, frame: *const c_void) {
    // SAFETY: the caller's NULL or live source and frame.
    unsafe { rust_preload(source, frame) }
}

/// The export's sequence on the adapters, with one change each.
#[derive(Clone, Copy)]
enum Mutant {
    /// Ignores `destroying()`.
    NoDestroyingCheck,
    /// Compares only the format and width.
    IgnoresHeight,
    /// Copies the frame under `async_mutex`.
    CopyUnderLock,
    /// Writes `last_frame_ts` without `async_mutex`.
    UnlockedTimestamp,
}

fn mutant_preload(mutant: Mutant, source: *mut c_void, frame: *const c_void) {
    if source.is_null() || frame.is_null() {
        // SAFETY: the export's own NULL handling.
        unsafe { rust_preload(source, frame) };
        return;
    }
    // SAFETY: a live source and frame from the oracle; the adapters pair as
    // the export pairs them.
    unsafe {
        if !matches!(mutant, Mutant::NoDestroyingCheck)
            && obs_source_preload_internal_destroying(source)
        {
            return;
        }
        let mut preload = obs_source_frame_copy_shape::default();
        let has = obs_source_preload_internal_preload_shape(source, &raw mut preload);
        let shape = obs_source_preload_internal_frame_shape(frame);
        let changed = !has
            || preload.format != shape.format
            || preload.width != shape.width
            || (!matches!(mutant, Mutant::IgnoresHeight) && preload.height != shape.height);
        if changed {
            obs_source_preload_internal_destroy_preload(source);
            obs_source_preload_internal_create_preload(source, frame);
        }
        if matches!(mutant, Mutant::CopyUnderLock) {
            obs_source_preload_internal_lock_async(source);
            obs_source_preload_internal_copy(source, frame);
            obs_source_preload_internal_set_last_frame_ts(source, frame);
            obs_source_preload_internal_unlock_async(source);
            return;
        }
        obs_source_preload_internal_copy(source, frame);
        if matches!(mutant, Mutant::UnlockedTimestamp) {
            obs_source_preload_internal_set_last_frame_ts(source, frame);
            return;
        }
        obs_source_preload_internal_lock_async(source);
        obs_source_preload_internal_set_last_frame_ts(source, frame);
        obs_source_preload_internal_unlock_async(source);
    }
}

macro_rules! mutant_fn {
    ($name:ident, $mutant:expr) => {
        unsafe extern "C" fn $name(source: *mut c_void, frame: *const c_void) {
            mutant_preload($mutant, source, frame);
        }
    };
}
mutant_fn!(m_no_destroying, Mutant::NoDestroyingCheck);
mutant_fn!(m_ignores_height, Mutant::IgnoresHeight);
mutant_fn!(m_copy_under_lock, Mutant::CopyUnderLock);
mutant_fn!(m_unlocked_timestamp, Mutant::UnlockedTimestamp);

/// Serializes the runs: the oracle's event log and log capture are process
/// globals (each test also runs in its own isolated child, which owns the
/// log handler).
static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn compare(case: &Case) -> (Outcome, Outcome) {
    let _guard = LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    (run(case, c_preload), run(case, r_preload))
}

const BGRA_2X2: Shape = (BGRA, 2, 2);

const fn call(shape: Shape, timestamp: u64) -> Call {
    Call {
        null_source: false,
        frame: Some((shape, timestamp)),
    }
}

const NULL_FRAME: Call = Call {
    null_source: false,
    frame: None,
};

fn single(destroying: bool, preload: Option<Shape>, call: Call) -> Case {
    Case {
        destroying,
        preload,
        last_frame_ts: 7,
        calls: vec![call],
    }
}

/// Shapes around `BGRA_2X2`: the same, and each field changed.
const SHAPES: [Shape; 7] = [
    BGRA_2X2,
    (I420, 2, 2),
    (NV12, 2, 2),
    (BGRA, 4, 2),
    (BGRA, 2, 4),
    (-1, 2, 2),
    (BGRA, u32::MAX, 0),
];

fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for destroying in [false, true] {
        for preload in [None, Some(BGRA_2X2), Some((I420, 4, 4))] {
            for null_source in [false, true] {
                let frames = SHAPES
                    .iter()
                    .flat_map(|&shape| [0, 100, u64::MAX].map(|ts| Some((shape, ts))))
                    .chain([None]);
                for frame in frames {
                    cases.push(single(destroying, preload, Call { null_source, frame }));
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
        let (c, rust) = compare(&case);
        assert_eq!(c, rust, "{case:?}");
    }
}

/// The characterized answers, pinned on the C side: the cmocka sequence on
/// one source.
#[test]
fn pins_the_characterized_sequence() {
    if !crate::output::isolated(concat!(module_path!(), "::pins_the_characterized_sequence")) {
        return;
    }
    // destroying 3, create 4, destroy 5, copy 6, lock/unlock 1/2.
    let case = Case {
        destroying: false,
        preload: None,
        last_frame_ts: 7,
        calls: vec![
            NULL_FRAME,
            call(BGRA_2X2, 100),
            call(BGRA_2X2, 200),
            call((I420, 2, 2), 300),
            Call {
                null_source: true,
                frame: Some(((I420, 2, 2), 400)),
            },
        ],
    };
    let (c, rust) = compare(&case);
    assert_eq!(c, rust);
    let silent = (0, 0, Vec::new());
    // NULL frame: only destroying() is read.
    assert_eq!(
        c.calls[0],
        (vec![(3, [Ptr::Source, Ptr::Null], [0; 3])], silent.clone())
    );
    // First frame: the NULL preload frame destroyed, one created.
    let fresh = |frame: usize, created: usize, shape: Shape, ts: u64| {
        vec![
            (3, [Ptr::Source, Ptr::Null], [0; 3]),
            (
                5,
                [
                    created.checked_sub(1).map_or(Ptr::Null, Ptr::Created),
                    Ptr::Null,
                ],
                [0; 3],
            ),
            (
                4,
                [Ptr::Null, Ptr::Created(created)],
                [shape.0 as u64, u64::from(shape.1), u64::from(shape.2)],
            ),
            (6, [Ptr::Created(created), Ptr::Frame(frame)], [ts, 0, 0]),
            (1, [Ptr::AsyncMutex, Ptr::Null], [0; 3]),
            (2, [Ptr::AsyncMutex, Ptr::Null], [0; 3]),
        ]
    };
    assert_eq!(c.calls[1], (fresh(1, 0, BGRA_2X2, 100), silent.clone()));
    // Same shape: reused.
    assert_eq!(
        c.calls[2],
        (
            vec![
                (3, [Ptr::Source, Ptr::Null], [0; 3]),
                (6, [Ptr::Created(0), Ptr::Frame(2)], [200, 0, 0]),
                (1, [Ptr::AsyncMutex, Ptr::Null], [0; 3]),
                (2, [Ptr::AsyncMutex, Ptr::Null], [0; 3]),
            ],
            silent.clone()
        )
    );
    // New format: destroyed and created again.
    assert_eq!(c.calls[3], (fresh(3, 1, (I420, 2, 2), 300), silent));
    // NULL source: the public entry's line, nothing read.
    assert_eq!(
        c.calls[4],
        (
            Vec::new(),
            (
                1,
                400,
                b"obs_source_preload_video: Null 'source' parameter".to_vec()
            )
        )
    );
    assert_eq!(c.state, (Ptr::Created(1), (I420, 2, 2), 300));

    // A destroying source: destroying() read, nothing else.
    let (c, rust) = compare(&single(true, Some(BGRA_2X2), call((I420, 2, 2), 100)));
    assert_eq!(c, rust);
    assert_eq!(c.calls[0].0, [(3, [Ptr::Source, Ptr::Null], [0; 3])]);
    assert_eq!(c.state, (Ptr::Preload, BGRA_2X2, 7));
}

/// Each mutant differs from the C on at least one case, so the comparison
/// can see it.
#[test]
fn rejects_mutants() {
    if !crate::output::isolated(concat!(module_path!(), "::rejects_mutants")) {
        return;
    }
    let destroying = single(true, Some(BGRA_2X2), call(BGRA_2X2, 100));
    let taller = single(false, Some(BGRA_2X2), call((BGRA, 2, 4), 100));
    let same = single(false, Some(BGRA_2X2), call(BGRA_2X2, 100));
    for (mutant, case) in [
        (m_no_destroying as Preload, &destroying),
        (m_ignores_height as Preload, &taller),
        (m_copy_under_lock as Preload, &same),
        (m_unlocked_timestamp as Preload, &same),
    ] {
        let _guard = LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let c = run(case, c_preload);
        let m = run(case, mutant);
        assert_ne!(c, m, "{case:?}");
    }
}

fn random_shape() -> impl Strategy<Value = Shape> {
    prop_oneof![
        3 => prop::sample::select(SHAPES.to_vec()),
        1 => (-2..20_i32, 0..6_u32, 0..6_u32),
    ]
}

fn random_call() -> impl Strategy<Value = Call> {
    (
        prop::bool::weighted(0.1),
        prop::option::weighted(0.85, (random_shape(), any::<u64>())),
    )
        .prop_map(|(null_source, frame)| Call { null_source, frame })
}

fn random_case() -> impl Strategy<Value = Case> {
    (
        prop::bool::weighted(0.15),
        prop::option::of(random_shape()),
        any::<u64>(),
        prop::collection::vec(random_call(), 1..8),
    )
        .prop_map(|(destroying, preload, last_frame_ts, calls)| Case {
            destroying,
            preload,
            last_frame_ts,
            calls,
        })
}

#[test]
fn matches_c_on_random_sequences() {
    if !crate::output::isolated(concat!(module_path!(), "::matches_c_on_random_sequences")) {
        return;
    }
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    });
    runner
        .run(&random_case(), |case| {
            let (c, rust) = compare(&case);
            prop_assert_eq!(c, rust);
            Ok(())
        })
        .unwrap();
}

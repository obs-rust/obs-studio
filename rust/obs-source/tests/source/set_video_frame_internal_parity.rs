//! Tier 3: the original C `obs_source_set_video_frame_internal()` against
//! the Rust export, each on its own real private source
//! (`source_set_video_frame_internal.c`). Both run over the same production
//! adapters, with the preload frame's create, destroy and copy, the
//! `async_mutex` lock and unlock, the graphics enter and leave, the async
//! texture size and update and the stale flag store recorded in one log in
//! the order they ran, and the same source state and frames; every recorded
//! call with its arguments, the diagnostic and the state left (the preload
//! frame, its shape, `last_frame_ts`, `async_textures_stale`) must match,
//! over single calls and call sequences on one source. Five mutants of the
//! export, built on the same adapters, must not.
//! No intentional differences or exclusions.
use core::ffi::{c_char, c_int, c_void};
use obs_source::ffi::source_frame_copy::obs_source_frame_copy_shape;
use obs_source::ffi::source_set_video_frame_internal::obs_source_set_video_frame_internal as rust_set;
use obs_source::frame_copy::video_format::{BGRA, I420, NV12};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
type SetFrame = unsafe extern "C" fn(*mut c_void, *const c_void);

unsafe extern "C" {
    fn oracle_obs_source_set_video_frame_internal(source: *mut c_void, frame: *const c_void);
    fn oracle_svf_events(
        kinds: *mut c_int,
        ptrs: *mut *const c_void,
        nums: *mut u64,
        max: usize,
    ) -> usize;
    fn oracle_svf_source_new(
        preload: bool,
        format: c_int,
        width: u32,
        height: u32,
        last_frame_ts: u64,
        stale: bool,
        texrender: bool,
    ) -> *mut c_void;
    fn oracle_svf_source_free(source: *mut c_void);
    fn oracle_svf_addresses(source: *const c_void, out: *mut *const c_void);
    fn oracle_svf_state(
        source: *const c_void,
        preload: *mut *const c_void,
        format: *mut c_int,
        width: *mut u32,
        height: *mut u32,
        last_frame_ts: *mut u64,
        stale: *mut bool,
    );
    fn oracle_svf_log_begin(get: GetLog, set: SetLog);
    fn oracle_svf_log_end(set: SetLog, level: *mut c_int, message: *mut c_char, size: usize)
    -> u32;
    // source_preload_video_internal.c's frames.
    fn oracle_pvi_frame_new(format: c_int, width: u32, height: u32, timestamp: u64) -> *mut c_void;
    fn oracle_pvi_frame_free(frame: *mut c_void);

    // The production adapters, for the mutants.
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
    fn obs_source_set_frame_internal_enter_graphics();
    fn obs_source_set_frame_internal_set_texture_size(source: *mut c_void);
    fn obs_source_set_frame_internal_update_textures(source: *mut c_void);
    fn obs_source_set_frame_internal_mark_fresh(source: *mut c_void);
    fn obs_source_set_frame_internal_leave_graphics();
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
    preload: Option<Shape>,
    last_frame_ts: u64,
    stale: bool,
    texrender: bool,
    calls: Vec<Call>,
}

/// A pointer the run knows by role, or another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ptr {
    Null,
    Source,
    AsyncMutex,
    AsyncTextures,
    TexturesStale,
    Texrender,
    /// The preload frame the source started with.
    Preload,
    /// The frame of call n.
    Frame(usize),
    /// The nth frame the run created.
    Created(usize),
    Unknown,
}

type Event = (c_int, [Ptr; 3], [u64; 3]);
/// The diagnostics of one call: count, last level, last message.
type Log = (u32, c_int, Vec<u8>);
/// Every address the run knows, by role, oldest first.
type Roles = Vec<(*const c_void, Ptr)>;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Outcome {
    /// Each call's events and log.
    calls: Vec<(Vec<Event>, Log)>,
    /// The preload frame, its shape, last_frame_ts and async_textures_stale
    /// after the last call.
    state: (Ptr, Shape, u64, bool),
}

const LOCK: c_int = 1;
const UNLOCK: c_int = 2;
const CREATE: c_int = 4;
const DESTROY: c_int = 5;
const COPY: c_int = 6;
const ENTER: c_int = 7;
const LEAVE: c_int = 8;
const SIZE: c_int = 9;
const UPDATE: c_int = 10;
const SET_BOOL: c_int = 11;

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
fn logged_call(set: SetFrame, source: *mut c_void, frame: *const c_void) -> Log {
    // SAFETY: the caller's NULL or live source and frame; the message
    // buffer is the size passed.
    unsafe {
        oracle_svf_log_begin(base_get_log_handler, base_set_log_handler);
        set(source, frame);
        let mut level = 0;
        let mut message = [0 as c_char; 256];
        let count = oracle_svf_log_end(
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
/// next `Created` role. The texrender's address is the fourth number, so it
/// is named too.
fn take_events(roles: &mut Roles, created: &mut usize) -> Vec<Event> {
    let mut kinds = [0 as c_int; 64];
    let mut ptrs = [core::ptr::null::<c_void>(); 64 * 3];
    let mut nums = [0_u64; 64 * 3];
    // SAFETY: the buffers are the sizes the oracle writes for `max`.
    let n = unsafe {
        oracle_svf_events(
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
            roles.push((ptrs[i * 3 + 1], Ptr::Created(*created)));
            *created += 1;
        }
        let mut num = [nums[i * 3], nums[i * 3 + 1], nums[i * 3 + 2]];
        if kinds[i] == UPDATE {
            // The texrender pointer, by role (0 for NULL).
            let texrender = name(roles, num[0] as usize as *const c_void);
            num[0] = match texrender {
                Ptr::Null => 0,
                Ptr::Texrender => 1,
                _ => 2,
            };
        }
        events.push((
            kinds[i],
            [
                name(roles, ptrs[i * 3]),
                name(roles, ptrs[i * 3 + 1]),
                name(roles, ptrs[i * 3 + 2]),
            ],
            num,
        ));
    }
    events
}

fn run(case: &Case, set: SetFrame) -> Outcome {
    // SAFETY: the oracle allocates a real source with its async_mutex
    // initialized and each frame, and frees them below; the address
    // buffer is the size the oracle writes.
    unsafe {
        let (has_preload, (format, width, height)) = case
            .preload
            .map_or((false, (0, 0, 0)), |shape| (true, shape));
        let source = oracle_svf_source_new(
            has_preload,
            format,
            width,
            height,
            case.last_frame_ts,
            case.stale,
            case.texrender,
        );
        assert!(!source.is_null());
        let mut addresses = [core::ptr::null::<c_void>(); 5];
        oracle_svf_addresses(source, addresses.as_mut_ptr());
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
            (addresses[2], Ptr::AsyncTextures),
            (addresses[3], Ptr::TexturesStale),
            (addresses[4], Ptr::Texrender),
        ];
        if !addresses[1].is_null() {
            roles.push((addresses[1], Ptr::Preload));
        }
        for (i, &frame) in frames.iter().enumerate() {
            if !frame.is_null() {
                roles.push((frame.cast_const(), Ptr::Frame(i)));
            }
        }

        oracle_svf_events(
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
            let log = logged_call(set, target, frame);
            calls.push((take_events(&mut roles, &mut created), log));
        }

        let mut preload = core::ptr::null();
        let mut shape = (0, 0, 0);
        let mut last = 0;
        let mut stale = false;
        oracle_svf_state(
            source,
            &raw mut preload,
            &raw mut shape.0,
            &raw mut shape.1,
            &raw mut shape.2,
            &raw mut last,
            &raw mut stale,
        );
        let state = (name(&roles, preload), shape, last, stale);
        for frame in frames {
            if !frame.is_null() {
                oracle_pvi_frame_free(frame);
            }
        }
        oracle_svf_source_free(source);
        Outcome { calls, state }
    }
}

unsafe extern "C" fn c_set(source: *mut c_void, frame: *const c_void) {
    // SAFETY: the caller's NULL or live source and frame.
    unsafe { oracle_obs_source_set_video_frame_internal(source, frame) }
}

unsafe extern "C" fn r_set(source: *mut c_void, frame: *const c_void) {
    // SAFETY: the caller's NULL or live source and frame.
    unsafe { rust_set(source, frame) }
}

/// The export's sequence on the adapters, with one change each.
#[derive(Clone, Copy)]
enum Mutant {
    /// Enters (and leaves) the graphics context before the NULL frame check.
    GraphicsBeforeFrameCheck,
    /// Compares only the format and height.
    IgnoresWidth,
    /// Sizes the textures before the copy.
    SizeBeforeCopy,
    /// Never clears `async_textures_stale`.
    KeepsStale,
    /// Writes `last_frame_ts` after leaving the graphics context.
    TimestampOutsideGraphics,
}

fn mutant_set(mutant: Mutant, source: *mut c_void, frame: *const c_void) {
    if source.is_null() {
        // SAFETY: the export's own NULL handling.
        unsafe { rust_set(source, frame) };
        return;
    }
    // SAFETY: a live source and NULL or live frame from the oracle; the
    // adapters pair as the export pairs them.
    unsafe {
        if frame.is_null() {
            if matches!(mutant, Mutant::GraphicsBeforeFrameCheck) {
                obs_source_set_frame_internal_enter_graphics();
                obs_source_set_frame_internal_leave_graphics();
            }
            return;
        }
        obs_source_set_frame_internal_enter_graphics();
        let mut preload = obs_source_frame_copy_shape::default();
        let has = obs_source_preload_internal_preload_shape(source, &raw mut preload);
        let shape = obs_source_preload_internal_frame_shape(frame);
        let changed = !has
            || preload.format != shape.format
            || (!matches!(mutant, Mutant::IgnoresWidth) && preload.width != shape.width)
            || preload.height != shape.height;
        if changed {
            obs_source_preload_internal_destroy_preload(source);
            obs_source_preload_internal_create_preload(source, frame);
        }
        if matches!(mutant, Mutant::SizeBeforeCopy) {
            obs_source_set_frame_internal_set_texture_size(source);
            obs_source_preload_internal_copy(source, frame);
        } else {
            obs_source_preload_internal_copy(source, frame);
            obs_source_set_frame_internal_set_texture_size(source);
        }
        obs_source_set_frame_internal_update_textures(source);
        if !matches!(mutant, Mutant::KeepsStale) {
            obs_source_set_frame_internal_mark_fresh(source);
        }
        if matches!(mutant, Mutant::TimestampOutsideGraphics) {
            obs_source_set_frame_internal_leave_graphics();
        }
        obs_source_preload_internal_lock_async(source);
        obs_source_preload_internal_set_last_frame_ts(source, frame);
        obs_source_preload_internal_unlock_async(source);
        if !matches!(mutant, Mutant::TimestampOutsideGraphics) {
            obs_source_set_frame_internal_leave_graphics();
        }
    }
}

macro_rules! mutant_fn {
    ($name:ident, $mutant:expr) => {
        unsafe extern "C" fn $name(source: *mut c_void, frame: *const c_void) {
            mutant_set($mutant, source, frame);
        }
    };
}
mutant_fn!(m_graphics_first, Mutant::GraphicsBeforeFrameCheck);
mutant_fn!(m_ignores_width, Mutant::IgnoresWidth);
mutant_fn!(m_size_before_copy, Mutant::SizeBeforeCopy);
mutant_fn!(m_keeps_stale, Mutant::KeepsStale);
mutant_fn!(m_timestamp_outside, Mutant::TimestampOutsideGraphics);

/// Serializes the runs: the oracle's event logs and log capture are process
/// globals (each test also runs in its own isolated child, which owns the
/// log handler).
static RUN_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn compare(case: &Case) -> (Outcome, Outcome) {
    let _guard = RUN_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    (run(case, c_set), run(case, r_set))
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

fn single(preload: Option<Shape>, call: Call) -> Case {
    Case {
        preload,
        last_frame_ts: 7,
        stale: true,
        texrender: false,
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
    for preload in [None, Some(BGRA_2X2), Some((I420, 4, 4))] {
        for stale in [false, true] {
            for texrender in [false, true] {
                for null_source in [false, true] {
                    let frames = SHAPES
                        .iter()
                        .flat_map(|&shape| [0, 100, u64::MAX].map(|ts| Some((shape, ts))))
                        .chain([None]);
                    for frame in frames {
                        cases.push(Case {
                            stale,
                            texrender,
                            ..single(preload, Call { null_source, frame })
                        });
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
    let case = Case {
        preload: None,
        last_frame_ts: 7,
        stale: true,
        texrender: true,
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
    let none = [Ptr::Null; 3];
    // NULL frame: nothing, not even the graphics context.
    assert_eq!(c.calls[0], (Vec::new(), silent.clone()));
    // After the preload frame (`preload`) is settled for call `frame`.
    let tail = |preload: Ptr, frame: usize, ts: u64| {
        vec![
            (COPY, [preload, Ptr::Frame(frame), Ptr::Null], [ts, 0, 0]),
            (SIZE, [Ptr::Source, preload, Ptr::Null], [0; 3]),
            (
                UPDATE,
                [Ptr::Source, preload, Ptr::AsyncTextures],
                [1, 0, 0],
            ),
            (SET_BOOL, [Ptr::TexturesStale, Ptr::Null, Ptr::Null], [0; 3]),
            (LOCK, [Ptr::AsyncMutex, Ptr::Null, Ptr::Null], [0; 3]),
            (UNLOCK, [Ptr::AsyncMutex, Ptr::Null, Ptr::Null], [0; 3]),
            (LEAVE, none, [0; 3]),
        ]
    };
    // First frame: the NULL preload frame destroyed, one created.
    let fresh = |frame: usize, created: usize, shape: Shape, ts: u64| {
        let mut events = vec![
            (ENTER, none, [0; 3]),
            (
                DESTROY,
                [
                    created.checked_sub(1).map_or(Ptr::Null, Ptr::Created),
                    Ptr::Null,
                    Ptr::Null,
                ],
                [0; 3],
            ),
            (
                CREATE,
                [Ptr::Null, Ptr::Created(created), Ptr::Null],
                [shape.0 as u64, u64::from(shape.1), u64::from(shape.2)],
            ),
        ];
        events.extend(tail(Ptr::Created(created), frame, ts));
        events
    };
    assert_eq!(c.calls[1], (fresh(1, 0, BGRA_2X2, 100), silent.clone()));
    // Same shape: reused.
    let mut reused = vec![(ENTER, none, [0; 3])];
    reused.extend(tail(Ptr::Created(0), 2, 200));
    assert_eq!(c.calls[2], (reused, silent.clone()));
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
                b"obs_source_set_video_frame: Null 'source' parameter".to_vec()
            )
        )
    );
    assert_eq!(c.state, (Ptr::Created(1), (I420, 2, 2), 300, false));
}

/// Each mutant differs from the C on at least one case, so the comparison
/// can see it.
#[test]
fn rejects_mutants() {
    if !crate::output::isolated(concat!(module_path!(), "::rejects_mutants")) {
        return;
    }
    let null_frame = single(Some(BGRA_2X2), NULL_FRAME);
    let wider = single(Some(BGRA_2X2), call((BGRA, 4, 2), 100));
    let same = single(Some(BGRA_2X2), call(BGRA_2X2, 100));
    for (mutant, case) in [
        (m_graphics_first as SetFrame, &null_frame),
        (m_ignores_width as SetFrame, &wider),
        (m_size_before_copy as SetFrame, &same),
        (m_keeps_stale as SetFrame, &same),
        (m_timestamp_outside as SetFrame, &same),
    ] {
        let _guard = RUN_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let c = run(case, c_set);
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
        prop::option::of(random_shape()),
        any::<u64>(),
        any::<bool>(),
        any::<bool>(),
        prop::collection::vec(random_call(), 1..8),
    )
        .prop_map(|(preload, last_frame_ts, stale, texrender, calls)| Case {
            preload,
            last_frame_ts,
            stale,
            texrender,
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

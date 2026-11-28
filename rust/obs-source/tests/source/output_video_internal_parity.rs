//! Tier 3: the original C `obs_source_output_video_internal()` against the
//! Rust export, each on its own real private source
//! (`source_output_video_internal.c`). Both run over the same production
//! adapters, with every call the worker makes (the two mutexes, the atomic
//! stores, the profiler, `cache_video()`, `free_async_cache()`, the
//! reference drop, the destroy and the queue push) recorded in the order
//! they ran, each with a snapshot of `last_output_frame_ts`,
//! `last_frame_ts` and the queue length, and the same source state, cache
//! answers and frames; every recorded call with its arguments, the
//! diagnostic and the state left (both timestamps, `async_active`,
//! `async_textures_stale` and the queue) must match, over single calls and
//! call sequences on one source. Six mutants of the export, built on the
//! same adapters, must not.
//! No intentional differences or exclusions.
use core::ffi::{c_char, c_int, c_long, c_void};
use obs_source::ffi::source_output_video_internal::obs_source_output_video_internal as rust_output;
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
type Output = unsafe extern "C" fn(*mut c_void, *const c_void);

unsafe extern "C" {
    fn oracle_obs_source_output_video_internal(source: *mut c_void, frame: *const c_void);
    fn oracle_ovi_events(
        kinds: *mut c_int,
        ptrs: *mut *const c_void,
        nums: *mut u64,
        max: usize,
    ) -> usize;
    fn oracle_ovi_answers(some: *const bool, refs: *const i64, count: usize);
    fn oracle_ovi_source_new(
        last_output_frame_ts: u64,
        last_frame_ts: u64,
        active: bool,
        stale: bool,
        queued: usize,
    ) -> *mut c_void;
    fn oracle_ovi_source_free(source: *mut c_void);
    fn oracle_ovi_queued(source: *const c_void, frames: *mut *const c_void, max: usize) -> usize;
    fn oracle_ovi_addresses(source: *const c_void, out: *mut *const c_void);
    fn oracle_ovi_state(
        source: *const c_void,
        last_output_frame_ts: *mut u64,
        last_frame_ts: *mut u64,
        active: *mut bool,
        stale: *mut bool,
    );
    fn oracle_ovi_frame_new(timestamp: u64) -> *mut c_void;
    fn oracle_ovi_frame_free(frame: *mut c_void);
    fn oracle_ovi_log_begin(get: GetLog, set: SetLog);
    fn oracle_ovi_log_end(set: SetLog, level: *mut c_int, message: *mut c_char, size: usize)
    -> u32;

    // The production adapters, for the mutants.
    fn obs_source_output_internal_lock_audio_buf(source: *mut c_void);
    fn obs_source_output_internal_frame_timestamp(frame: *const c_void) -> u64;
    fn obs_source_output_internal_set_last_output_frame_ts(source: *mut c_void, timestamp: u64);
    fn obs_source_output_internal_unlock_audio_buf(source: *mut c_void);
    fn obs_source_output_internal_lock_async(source: *mut c_void);
    fn obs_source_output_internal_set_async_active(source: *mut c_void, active: bool);
    fn obs_source_output_internal_mark_stale(source: *mut c_void);
    fn obs_source_output_internal_clear_last_frame_ts(source: *mut c_void);
    fn obs_source_output_internal_free_cache(source: *mut c_void);
    fn obs_source_output_internal_unlock_async(source: *mut c_void);
    fn obs_source_output_internal_frame_received(source: *mut c_void);
    fn obs_source_output_internal_cache_video(
        source: *mut c_void,
        frame: *const c_void,
    ) -> *mut c_void;
    fn obs_source_output_internal_release_ref(frame: *mut c_void) -> c_long;
    fn obs_source_output_internal_destroy_frame(frame: *mut c_void);
    fn obs_source_output_internal_push_frame(source: *mut c_void, frame: *mut c_void);
}

/// One call: a NULL source, and the frame's timestamp or a NULL frame.
#[derive(Debug, Clone, Copy)]
struct Call {
    null_source: bool,
    frame: Option<u64>,
}

#[derive(Debug, Clone)]
struct Case {
    last_output_frame_ts: u64,
    last_frame_ts: u64,
    active: bool,
    stale: bool,
    /// Frames already queued.
    queued: usize,
    /// `cache_video()`'s answers in order: the references the frame it
    /// makes holds, or an overflow; past the last, frames holding 2.
    answers: Vec<Option<i32>>,
    calls: Vec<Call>,
}

/// A pointer the run knows by role, or another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ptr {
    Null,
    Source,
    AudioBufMutex,
    AsyncMutex,
    AsyncActive,
    TexturesStale,
    /// The `async_frames` darray.
    Queue,
    /// The nth frame queued before the run.
    Queued(usize),
    /// The frame of call n.
    Frame(usize),
    /// The nth frame the stand-in cache made.
    Made(usize),
    Unknown,
}

type Event = (c_int, [Ptr; 3], [u64; 4]);
/// The diagnostics of one call: count, last level, last message.
type Log = (u32, c_int, Vec<u8>);
/// Every address the run knows, by role, oldest first.
type Roles = Vec<(*const c_void, Ptr)>;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Outcome {
    /// Each call's events and log.
    calls: Vec<(Vec<Event>, Log)>,
    /// `last_output_frame_ts`, `last_frame_ts`, `async_active`,
    /// `async_textures_stale` and the queue after the last call.
    state: (u64, u64, bool, bool, Vec<Ptr>),
}

const LOCK: c_int = 1;
const UNLOCK: c_int = 2;
const SET_BOOL: c_int = 3;
const FREE_CACHE: c_int = 4;
const RECEIVED: c_int = 5;
const CACHE: c_int = 6;
const DEC: c_int = 7;
const DESTROY: c_int = 8;
const PUSH: c_int = 9;

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
fn logged_call(output: Output, source: *mut c_void, frame: *const c_void) -> Log {
    // SAFETY: the caller's NULL or live source and frame; the message
    // buffer is the size passed.
    unsafe {
        oracle_ovi_log_begin(base_get_log_handler, base_set_log_handler);
        output(source, frame);
        let mut level = 0;
        let mut message = [0 as c_char; 256];
        let count = oracle_ovi_log_end(
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

/// The events since the last read, by role; each frame the cache makes
/// gets the next `Made` role.
fn take_events(roles: &mut Roles, made: &mut usize) -> Vec<Event> {
    let mut kinds = [0 as c_int; 64];
    let mut ptrs = [core::ptr::null::<c_void>(); 64 * 3];
    let mut nums = [0_u64; 64 * 4];
    // SAFETY: the buffers are the sizes the oracle writes for `max`.
    let n = unsafe {
        oracle_ovi_events(
            kinds.as_mut_ptr(),
            ptrs.as_mut_ptr(),
            nums.as_mut_ptr(),
            kinds.len(),
        )
    };
    assert!(n <= kinds.len(), "event log overflowed");
    let mut events = Vec::new();
    for i in 0..n {
        if kinds[i] == CACHE && !ptrs[i * 3 + 2].is_null() {
            roles.push((ptrs[i * 3 + 2], Ptr::Made(*made)));
            *made += 1;
        }
        events.push((
            kinds[i],
            [
                name(roles, ptrs[i * 3]),
                name(roles, ptrs[i * 3 + 1]),
                name(roles, ptrs[i * 3 + 2]),
            ],
            [
                nums[i * 4],
                nums[i * 4 + 1],
                nums[i * 4 + 2],
                nums[i * 4 + 3],
            ],
        ));
    }
    events
}

fn run(case: &Case, output: Output) -> Outcome {
    // SAFETY: the oracle allocates a real source with both mutexes
    // initialized and each frame, and frees them below; the buffers are the
    // sizes passed.
    unsafe {
        let source = oracle_ovi_source_new(
            case.last_output_frame_ts,
            case.last_frame_ts,
            case.active,
            case.stale,
            case.queued,
        );
        assert!(!source.is_null());
        let some: Vec<bool> = case.answers.iter().map(Option::is_some).collect();
        let refs: Vec<i64> = case
            .answers
            .iter()
            .map(|a| i64::from(a.unwrap_or(0)))
            .collect();
        oracle_ovi_answers(some.as_ptr(), refs.as_ptr(), some.len());
        let mut addresses = [core::ptr::null::<c_void>(); 5];
        oracle_ovi_addresses(source, addresses.as_mut_ptr());
        let mut roles: Roles = vec![
            (source.cast_const(), Ptr::Source),
            (addresses[0], Ptr::AudioBufMutex),
            (addresses[1], Ptr::AsyncMutex),
            (addresses[2], Ptr::AsyncActive),
            (addresses[3], Ptr::TexturesStale),
            (addresses[4], Ptr::Queue),
        ];
        let mut queued = [core::ptr::null::<c_void>(); 64];
        let count = oracle_ovi_queued(source, queued.as_mut_ptr(), queued.len());
        for (i, &frame) in queued.iter().take(count).enumerate() {
            roles.push((frame, Ptr::Queued(i)));
        }
        let frames: Vec<*mut c_void> = case
            .calls
            .iter()
            .map(|call| {
                call.frame
                    .map_or(core::ptr::null_mut(), |ts| oracle_ovi_frame_new(ts))
            })
            .collect();
        for (i, &frame) in frames.iter().enumerate() {
            if !frame.is_null() {
                roles.push((frame.cast_const(), Ptr::Frame(i)));
            }
        }

        oracle_ovi_events(
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            0,
        );
        let mut made = 0;
        let mut calls = Vec::new();
        for (call, &frame) in case.calls.iter().zip(&frames) {
            let target = if call.null_source {
                core::ptr::null_mut()
            } else {
                source
            };
            let log = logged_call(output, target, frame);
            calls.push((take_events(&mut roles, &mut made), log));
        }

        let mut last_output = 0;
        let mut last_frame = 0;
        let mut active = false;
        let mut stale = false;
        oracle_ovi_state(
            source,
            &raw mut last_output,
            &raw mut last_frame,
            &raw mut active,
            &raw mut stale,
        );
        let count = oracle_ovi_queued(source, queued.as_mut_ptr(), queued.len());
        assert!(count <= queued.len());
        let queue = queued
            .iter()
            .take(count)
            .map(|&frame| name(&roles, frame))
            .collect();
        for frame in frames {
            if !frame.is_null() {
                oracle_ovi_frame_free(frame);
            }
        }
        oracle_ovi_source_free(source);
        Outcome {
            calls,
            state: (last_output, last_frame, active, stale, queue),
        }
    }
}

unsafe extern "C" fn c_output(source: *mut c_void, frame: *const c_void) {
    // SAFETY: the caller's NULL or live source and frame.
    unsafe { oracle_obs_source_output_video_internal(source, frame) }
}

unsafe extern "C" fn r_output(source: *mut c_void, frame: *const c_void) {
    // SAFETY: the caller's NULL or live source and frame.
    unsafe { rust_output(source, frame) }
}

/// The export's sequence on the adapters, with one change each.
#[derive(Clone, Copy)]
enum Mutant {
    /// Writes `last_output_frame_ts` after unlocking `audio_buf_mutex`.
    TimestampOutsideLock,
    /// A NULL frame leaves the textures as they were.
    NullFrameKeepsStale,
    /// Calls `cache_video()` with `async_mutex` held.
    CacheInsideAsyncMutex,
    /// Queues the frame even when its last reference went.
    QueuesTheLastReference,
    /// Sets `async_active` before the push.
    ActiveBeforePush,
    /// Tells the profiler after the cache.
    ProfilerAfterCache,
}

fn mutant_output(mutant: Mutant, source: *mut c_void, frame: *const c_void) {
    if source.is_null() {
        // SAFETY: the export's own NULL handling.
        unsafe { rust_output(source, frame) };
        return;
    }
    // SAFETY: a live source and NULL or live frame from the oracle; the
    // adapters pair as the export pairs them.
    unsafe {
        obs_source_output_internal_lock_audio_buf(source);
        let ts = if frame.is_null() {
            0
        } else {
            obs_source_output_internal_frame_timestamp(frame)
        };
        if matches!(mutant, Mutant::TimestampOutsideLock) {
            obs_source_output_internal_unlock_audio_buf(source);
            obs_source_output_internal_set_last_output_frame_ts(source, ts);
        } else {
            obs_source_output_internal_set_last_output_frame_ts(source, ts);
            obs_source_output_internal_unlock_audio_buf(source);
        }
        if frame.is_null() {
            obs_source_output_internal_lock_async(source);
            obs_source_output_internal_set_async_active(source, false);
            if !matches!(mutant, Mutant::NullFrameKeepsStale) {
                obs_source_output_internal_mark_stale(source);
            }
            obs_source_output_internal_clear_last_frame_ts(source);
            obs_source_output_internal_free_cache(source);
            obs_source_output_internal_unlock_async(source);
            return;
        }
        if !matches!(mutant, Mutant::ProfilerAfterCache) {
            obs_source_output_internal_frame_received(source);
        }
        let inside = matches!(mutant, Mutant::CacheInsideAsyncMutex);
        if inside {
            obs_source_output_internal_lock_async(source);
        }
        let cached = obs_source_output_internal_cache_video(source, frame);
        if matches!(mutant, Mutant::ProfilerAfterCache) {
            obs_source_output_internal_frame_received(source);
        }
        if !inside {
            obs_source_output_internal_lock_async(source);
        }
        if !cached.is_null() {
            let left = obs_source_output_internal_release_ref(cached);
            if left == 0 && !matches!(mutant, Mutant::QueuesTheLastReference) {
                obs_source_output_internal_destroy_frame(cached);
            } else if matches!(mutant, Mutant::ActiveBeforePush) {
                obs_source_output_internal_set_async_active(source, true);
                obs_source_output_internal_push_frame(source, cached);
            } else {
                obs_source_output_internal_push_frame(source, cached);
                obs_source_output_internal_set_async_active(source, true);
            }
        }
        obs_source_output_internal_unlock_async(source);
    }
}

macro_rules! mutant_fn {
    ($name:ident, $mutant:expr) => {
        unsafe extern "C" fn $name(source: *mut c_void, frame: *const c_void) {
            mutant_output($mutant, source, frame);
        }
    };
}
mutant_fn!(m_timestamp_outside, Mutant::TimestampOutsideLock);
mutant_fn!(m_keeps_stale, Mutant::NullFrameKeepsStale);
mutant_fn!(m_cache_inside, Mutant::CacheInsideAsyncMutex);
mutant_fn!(m_queues_last, Mutant::QueuesTheLastReference);
mutant_fn!(m_active_first, Mutant::ActiveBeforePush);
mutant_fn!(m_profiler_after, Mutant::ProfilerAfterCache);

/// Serializes the runs: the oracle's event log, cache answers and log
/// capture are process globals (each test also runs in its own isolated
/// child, which owns the log handler).
static RUN_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn compare(case: &Case) -> (Outcome, Outcome) {
    let _guard = RUN_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    (run(case, c_output), run(case, r_output))
}

const fn call(timestamp: u64) -> Call {
    Call {
        null_source: false,
        frame: Some(timestamp),
    }
}

const NULL_FRAME: Call = Call {
    null_source: false,
    frame: None,
};

fn single(answer: Option<Option<i32>>, call: Call) -> Case {
    Case {
        last_output_frame_ts: 9,
        last_frame_ts: 7,
        active: false,
        stale: false,
        queued: 2,
        answers: answer.into_iter().collect(),
        calls: vec![call],
    }
}

fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    let answers = [
        None,
        Some(None),
        Some(Some(1)),
        Some(Some(2)),
        Some(Some(3)),
        Some(Some(0)),
        Some(Some(-1)),
        Some(Some(i32::MAX)),
        Some(Some(i32::MIN)),
    ];
    for active in [false, true] {
        for stale in [false, true] {
            for queued in [0, 2] {
                for null_source in [false, true] {
                    let frames = [0, 100, u64::MAX].map(Some).into_iter().chain([None]);
                    for frame in frames {
                        for answer in answers {
                            cases.push(Case {
                                active,
                                stale,
                                queued,
                                last_output_frame_ts: 9,
                                last_frame_ts: 7,
                                answers: answer.into_iter().collect(),
                                calls: vec![Call { null_source, frame }],
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
        let (c, rust) = compare(&case);
        assert_eq!(c, rust, "{case:?}");
    }
}

/// The characterized answers, pinned on the C side: a frame queued, a
/// second, an overflow, the last reference destroyed, a NULL frame and a
/// NULL source on one source.
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the pinned event table: one block per characterized call"
)]
fn pins_the_characterized_sequence() {
    if !crate::output::isolated(concat!(module_path!(), "::pins_the_characterized_sequence")) {
        return;
    }
    let case = Case {
        last_output_frame_ts: 9,
        last_frame_ts: 7,
        active: false,
        stale: false,
        queued: 1,
        answers: vec![Some(2), None, Some(1), Some(2)],
        calls: vec![
            call(100),
            call(200),
            call(300),
            NULL_FRAME,
            call(400),
            Call {
                null_source: true,
                frame: Some(500),
            },
        ],
    };
    let (c, rust) = compare(&case);
    assert_eq!(c, rust);
    let silent = (0, 0, Vec::new());
    let lock = |m: Ptr, snap: [u64; 3]| {
        (
            LOCK,
            [m, Ptr::Null, Ptr::Null],
            [0, snap[0], snap[1], snap[2]],
        )
    };
    let unlock = |m: Ptr, snap: [u64; 3]| {
        (
            UNLOCK,
            [m, Ptr::Null, Ptr::Null],
            [0, snap[0], snap[1], snap[2]],
        )
    };
    let ev = |kind, p: [Ptr; 3], a: u64, snap: [u64; 3]| (kind, p, [a, snap[0], snap[1], snap[2]]);
    // A frame queued: the timestamp under audio_buf_mutex, the profiler and
    // the cache outside any lock, then the reference drop, the push and
    // async_active under async_mutex.
    assert_eq!(
        c.calls[0],
        (
            vec![
                lock(Ptr::AudioBufMutex, [9, 7, 1]),
                unlock(Ptr::AudioBufMutex, [100, 7, 1]),
                ev(
                    RECEIVED,
                    [Ptr::Source, Ptr::Null, Ptr::Null],
                    0,
                    [100, 7, 1]
                ),
                ev(
                    CACHE,
                    [Ptr::Source, Ptr::Frame(0), Ptr::Made(0)],
                    100,
                    [100, 7, 1]
                ),
                lock(Ptr::AsyncMutex, [100, 7, 1]),
                ev(DEC, [Ptr::Made(0), Ptr::Null, Ptr::Null], 1, [100, 7, 1]),
                ev(PUSH, [Ptr::Queue, Ptr::Made(0), Ptr::Null], 0, [100, 7, 1]),
                ev(
                    SET_BOOL,
                    [Ptr::AsyncActive, Ptr::Null, Ptr::Null],
                    1,
                    [100, 7, 2]
                ),
                unlock(Ptr::AsyncMutex, [100, 7, 2]),
            ],
            silent.clone()
        )
    );
    // An overflow: the queue emptied by the cache, nothing more.
    assert_eq!(
        c.calls[1],
        (
            vec![
                lock(Ptr::AudioBufMutex, [100, 7, 2]),
                unlock(Ptr::AudioBufMutex, [200, 7, 2]),
                ev(
                    RECEIVED,
                    [Ptr::Source, Ptr::Null, Ptr::Null],
                    0,
                    [200, 7, 2]
                ),
                ev(
                    CACHE,
                    [Ptr::Source, Ptr::Frame(1), Ptr::Null],
                    200,
                    [200, 7, 2]
                ),
                lock(Ptr::AsyncMutex, [200, 7, 0]),
                unlock(Ptr::AsyncMutex, [200, 7, 0]),
            ],
            silent.clone()
        )
    );
    // The last reference: destroyed, not queued.
    assert_eq!(
        c.calls[2],
        (
            vec![
                lock(Ptr::AudioBufMutex, [200, 7, 0]),
                unlock(Ptr::AudioBufMutex, [300, 7, 0]),
                ev(
                    RECEIVED,
                    [Ptr::Source, Ptr::Null, Ptr::Null],
                    0,
                    [300, 7, 0]
                ),
                ev(
                    CACHE,
                    [Ptr::Source, Ptr::Frame(2), Ptr::Made(1)],
                    300,
                    [300, 7, 0]
                ),
                lock(Ptr::AsyncMutex, [300, 7, 0]),
                ev(DEC, [Ptr::Made(1), Ptr::Null, Ptr::Null], 0, [300, 7, 0]),
                ev(
                    DESTROY,
                    [Ptr::Made(1), Ptr::Null, Ptr::Null],
                    0,
                    [300, 7, 0]
                ),
                unlock(Ptr::AsyncMutex, [300, 7, 0]),
            ],
            silent.clone()
        )
    );
    // A NULL frame: under async_mutex, async_active cleared, the textures
    // stale, last_frame_ts zeroed and the cache freed.
    assert_eq!(
        c.calls[3],
        (
            vec![
                lock(Ptr::AudioBufMutex, [300, 7, 0]),
                unlock(Ptr::AudioBufMutex, [0, 7, 0]),
                lock(Ptr::AsyncMutex, [0, 7, 0]),
                ev(
                    SET_BOOL,
                    [Ptr::AsyncActive, Ptr::Null, Ptr::Null],
                    0,
                    [0, 7, 0]
                ),
                ev(
                    SET_BOOL,
                    [Ptr::TexturesStale, Ptr::Null, Ptr::Null],
                    1,
                    [0, 7, 0]
                ),
                ev(
                    FREE_CACHE,
                    [Ptr::Source, Ptr::Null, Ptr::Null],
                    0,
                    [0, 0, 0]
                ),
                unlock(Ptr::AsyncMutex, [0, 0, 0]),
            ],
            silent.clone()
        )
    );
    assert_eq!(c.calls[4].0.len(), 9);
    // NULL source: the public entry's line, nothing read.
    assert_eq!(
        c.calls[5],
        (
            Vec::new(),
            (
                1,
                400,
                b"obs_source_output_video: Null 'source' parameter".to_vec()
            )
        )
    );
    assert_eq!(c.state, (400, 0, true, true, vec![Ptr::Made(2)]));
}

/// Each mutant differs from the C on at least one case, so the comparison
/// can see it.
#[test]
fn rejects_mutants() {
    if !crate::output::isolated(concat!(module_path!(), "::rejects_mutants")) {
        return;
    }
    let queued = single(None, call(100));
    let last = single(Some(Some(1)), call(100));
    let null_frame = single(None, NULL_FRAME);
    for (mutant, case) in [
        (m_timestamp_outside as Output, &queued),
        (m_keeps_stale as Output, &null_frame),
        (m_cache_inside as Output, &queued),
        (m_queues_last as Output, &last),
        (m_active_first as Output, &queued),
        (m_profiler_after as Output, &queued),
    ] {
        let _guard = RUN_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let c = run(case, c_output);
        let m = run(case, mutant);
        assert_ne!(c, m, "{case:?}");
    }
}

fn random_call() -> impl Strategy<Value = Call> {
    (
        prop::bool::weighted(0.1),
        prop::option::weighted(0.8, any::<u64>()),
    )
        .prop_map(|(null_source, frame)| Call { null_source, frame })
}

fn random_answer() -> impl Strategy<Value = Option<i32>> {
    prop_oneof![
        1 => Just(None),
        4 => (-2..5_i32).prop_map(Some),
        1 => any::<i32>().prop_map(Some),
    ]
}

fn random_case() -> impl Strategy<Value = Case> {
    (
        any::<u64>(),
        any::<u64>(),
        any::<bool>(),
        any::<bool>(),
        0..4_usize,
        prop::collection::vec(random_answer(), 0..8),
        prop::collection::vec(random_call(), 1..8),
    )
        .prop_map(
            |(last_output_frame_ts, last_frame_ts, active, stale, queued, answers, calls)| Case {
                last_output_frame_ts,
                last_frame_ts,
                active,
                stale,
                queued,
                answers,
                calls,
            },
        )
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

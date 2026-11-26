//! Tier 3: the original C `obs_source_get_frame()` and
//! `obs_source_release_frame()` against the Rust exports, each on its own
//! real private source and frames (`source_get_release_frame.c`). Both run
//! over the same production adapters, with the async mutex and the frame
//! destroy recorded, and the same call sequences; every call's result, the
//! recorded lock, unlock and destroy calls, the diagnostic, and the state
//! left (the current frame, each live frame's references and `prev_frame`,
//! every cache slot) must match. No intentional differences or exclusions.
use core::ffi::{c_char, c_int, c_long, c_void};
use obs_source::ffi::source_get_release_frame::{
    obs_source_get_frame as rust_get, obs_source_release_frame as rust_release,
};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
type Get = unsafe extern "C" fn(*mut c_void) -> *mut c_void;
type Release = unsafe extern "C" fn(*mut c_void, *mut c_void);

unsafe extern "C" {
    fn oracle_obs_source_get_frame(source: *mut c_void) -> *mut c_void;
    fn oracle_obs_source_release_frame(source: *mut c_void, frame: *mut c_void);
    fn oracle_grf_events(kinds: *mut c_int, args: *mut *const c_void, max: usize) -> usize;
    fn oracle_grf_source_new() -> *mut c_void;
    fn oracle_grf_source_free(source: *mut c_void);
    fn oracle_grf_mutex(source: *const c_void) -> *const c_void;
    fn oracle_grf_set_current(source: *mut c_void, frame: *mut c_void);
    fn oracle_grf_current(source: *const c_void) -> *mut c_void;
    fn oracle_grf_cache_push(source: *mut c_void, frame: *mut c_void, used: bool, unused: c_long);
    fn oracle_grf_cache_slot(
        source: *const c_void,
        index: usize,
        frame: *mut *const c_void,
        used: *mut bool,
        unused: *mut c_long,
    ) -> bool;
    fn oracle_grf_frame_new(refs: c_long, prev_frame: bool) -> *mut c_void;
    fn oracle_grf_frame_free(frame: *mut c_void);
    fn oracle_grf_frame_refs(frame: *const c_void) -> c_long;
    fn oracle_grf_frame_prev(frame: *const c_void) -> bool;
    fn oracle_grf_log_begin(get: GetLog, set: SetLog);
    fn oracle_grf_log_end(set: SetLog, level: *mut c_int, message: *mut c_char, size: usize)
    -> u32;
}

#[derive(Debug, Clone, Copy)]
enum Op {
    Get {
        null_source: bool,
    },
    Release {
        null_source: bool,
        frame: Option<usize>,
    },
}

#[derive(Debug, Clone)]
struct Scenario {
    /// (refs, prev_frame) of each frame.
    frames: Vec<(c_long, bool)>,
    current: Option<usize>,
    /// (frame, used, unused_count), in cache order.
    cache: Vec<(usize, bool, c_long)>,
    ops: Vec<Op>,
}

/// A pointer the run knows: NULL, the mutex, a frame by index, or other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ptr {
    Null,
    Mutex,
    Frame(usize),
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Step {
    op: usize,
    result: Ptr,
    events: Vec<(c_int, Ptr)>,
    log: (u32, c_int, Vec<u8>),
    current: Ptr,
    /// (refs, prev_frame) of each frame, None once destroyed.
    frames: Vec<Option<(c_long, bool)>>,
    cache: Vec<(Ptr, bool, c_long)>,
}

const DESTROY: c_int = 3;

/// One run's real source and frames, owned by the oracle.
struct World {
    source: *mut c_void,
    mutex: *const c_void,
    frames: Vec<*mut c_void>,
    live: Vec<bool>,
}

impl World {
    fn new(scenario: &Scenario) -> Self {
        // SAFETY: the oracle allocates a real source and frames and links
        // them as the scenario says; every index is below the frame count.
        unsafe {
            let source = oracle_grf_source_new();
            assert!(!source.is_null());
            let frames: Vec<*mut c_void> = scenario
                .frames
                .iter()
                .map(|&(refs, prev)| oracle_grf_frame_new(refs, prev))
                .collect();
            if let Some(current) = scenario.current {
                oracle_grf_set_current(source, frames[current]);
            }
            for &(frame, used, unused) in &scenario.cache {
                oracle_grf_cache_push(source, frames[frame], used, unused);
            }
            oracle_grf_events(core::ptr::null_mut(), core::ptr::null_mut(), 0);
            World {
                source,
                mutex: oracle_grf_mutex(source),
                live: vec![true; frames.len()],
                frames,
            }
        }
    }

    fn name(&self, ptr: *const c_void) -> Ptr {
        if ptr.is_null() {
            Ptr::Null
        } else if ptr == self.mutex {
            Ptr::Mutex
        } else {
            self.frames
                .iter()
                .position(|&f| f.cast_const() == ptr)
                .map_or(Ptr::Unknown, Ptr::Frame)
        }
    }

    fn current(&self) -> *mut c_void {
        // SAFETY: the live source.
        unsafe { oracle_grf_current(self.source) }
    }

    /// A frame is released only while live and no longer the source's
    /// current frame, as callers do after a get.
    fn skips(&self, op: Op) -> bool {
        matches!(op, Op::Release { frame: Some(frame), .. }
            if !self.live[frame] || self.current() == self.frames[frame])
    }

    fn call(&self, op: Op, get: Get, release: Release) -> *mut c_void {
        let source = |null: bool| {
            if null {
                core::ptr::null_mut()
            } else {
                self.source
            }
        };
        // SAFETY: NULL or the live source, NULL or a live frame (skips()).
        unsafe {
            match op {
                Op::Get { null_source } => get(source(null_source)),
                Op::Release { null_source, frame } => {
                    release(
                        source(null_source),
                        frame.map_or(core::ptr::null_mut(), |f| self.frames[f]),
                    );
                    core::ptr::null_mut()
                }
            }
        }
    }

    /// The recorded calls since the last read; marks destroyed frames.
    fn events(&mut self) -> Vec<(c_int, Ptr)> {
        let mut kinds = [0; 16];
        let mut args = [core::ptr::null(); 16];
        // SAFETY: both buffers hold the max passed.
        let n = unsafe { oracle_grf_events(kinds.as_mut_ptr(), args.as_mut_ptr(), kinds.len()) };
        assert!(n <= kinds.len());
        let events: Vec<(c_int, Ptr)> = (0..n).map(|i| (kinds[i], self.name(args[i]))).collect();
        for &(kind, ptr) in &events {
            if let (DESTROY, Ptr::Frame(f)) = (kind, ptr) {
                self.live[f] = false;
            }
        }
        events
    }

    fn cache(&self) -> Vec<(Ptr, bool, c_long)> {
        let mut cache = Vec::new();
        loop {
            let (mut frame, mut used, mut unused) = (core::ptr::null(), false, 0);
            // SAFETY: the live source; the outputs are plain locals.
            let present = unsafe {
                oracle_grf_cache_slot(
                    self.source,
                    cache.len(),
                    &raw mut frame,
                    &raw mut used,
                    &raw mut unused,
                )
            };
            if !present {
                return cache;
            }
            cache.push((self.name(frame), used, unused));
        }
    }

    fn frames(&self) -> Vec<Option<(c_long, bool)>> {
        self.frames
            .iter()
            .zip(&self.live)
            .map(|(&f, &alive)| {
                // SAFETY: a live frame.
                alive.then(|| unsafe { (oracle_grf_frame_refs(f), oracle_grf_frame_prev(f)) })
            })
            .collect()
    }
}

impl Drop for World {
    fn drop(&mut self) {
        // SAFETY: each live frame and the source are freed once; destroyed
        // frames were freed by the recorded destroy.
        unsafe {
            for (&frame, &alive) in self.frames.iter().zip(&self.live) {
                if alive {
                    oracle_grf_frame_free(frame);
                }
            }
            oracle_grf_source_free(self.source);
        }
    }
}

/// Runs the call and returns its diagnostics: count, level, message.
fn logged(call: impl FnOnce() -> *mut c_void) -> (*mut c_void, (u32, c_int, Vec<u8>)) {
    let mut level = 0;
    let mut message = [0 as c_char; 256];
    // SAFETY: the isolated child owns the log handler; the capture is
    // restored before returning, and the message buffer holds its size.
    unsafe {
        oracle_grf_log_begin(base_get_log_handler, base_set_log_handler);
        let result = call();
        let count = oracle_grf_log_end(
            base_set_log_handler,
            &raw mut level,
            message.as_mut_ptr(),
            message.len(),
        );
        let message = core::ffi::CStr::from_ptr(message.as_ptr())
            .to_bytes()
            .to_vec();
        (result, (count, level, message))
    }
}

fn run(scenario: &Scenario, get: Get, release: Release) -> Vec<Step> {
    let mut world = World::new(scenario);
    let mut steps = Vec::new();
    for (index, &op) in scenario.ops.iter().enumerate() {
        if world.skips(op) {
            continue;
        }
        let (result, log) = logged(|| world.call(op, get, release));
        let events = world.events();
        steps.push(Step {
            op: index,
            result: world.name(result),
            events,
            log,
            current: world.name(world.current()),
            frames: world.frames(),
            cache: world.cache(),
        });
    }
    steps
}

fn agree(scenario: &Scenario, get: Get, release: Release) -> bool {
    run(
        scenario,
        oracle_obs_source_get_frame,
        oracle_obs_source_release_frame,
    ) == run(scenario, get, release)
}

fn check(scenario: &Scenario) -> Vec<Step> {
    let c = run(
        scenario,
        oracle_obs_source_get_frame,
        oracle_obs_source_release_frame,
    );
    let rust = run(scenario, rust_get, rust_release);
    assert_eq!(c, rust, "{scenario:?}");
    c
}

/// Every `test_source_get_release_frame` case as one sequence, with the
/// shared answer pinned so a change on both sides still fails.
fn characterized() -> Scenario {
    Scenario {
        frames: vec![(1, true), (3, true), (5, true), (1, true), (0, true)],
        current: Some(0),
        cache: vec![
            (2, true, 1),
            (1, true, 2),
            (1, true, 5),
            (3, true, 4),
            (4, true, 0),
        ],
        ops: vec![
            Op::Get { null_source: true },
            Op::Get { null_source: false },
            Op::Get { null_source: false },
            Op::Release {
                null_source: false,
                frame: None,
            },
            Op::Release {
                null_source: true,
                frame: None,
            },
            Op::Release {
                null_source: false,
                frame: Some(0),
            },
            Op::Release {
                null_source: false,
                frame: Some(1),
            },
            Op::Release {
                null_source: false,
                frame: Some(1),
            },
            Op::Release {
                null_source: true,
                frame: Some(2),
            },
            Op::Release {
                null_source: false,
                frame: Some(3),
            },
            Op::Release {
                null_source: false,
                frame: Some(4),
            },
        ],
    }
}

#[test]
fn get_release_frame_matches_c() {
    if !crate::output::isolated(concat!(module_path!(), "::get_release_frame_matches_c")) {
        return;
    }
    let steps = check(&characterized());
    pin_gets(&steps);
    pin_releases(&steps);
}

/// The shared answer of the characterized gets, pinned.
fn pin_gets(steps: &[Step]) {
    let lock = (1, Ptr::Mutex);
    let unlock = (2, Ptr::Mutex);
    // NULL source: the obs_source_valid() line and nothing else.
    assert_eq!(steps[0].result, Ptr::Null);
    assert!(steps[0].events.is_empty());
    assert_eq!(
        steps[0].log,
        (
            1,
            400,
            b"obs_source_get_frame: Null 'source' parameter".to_vec()
        )
    );
    // The current frame is taken with one more reference, then the slot is
    // empty.
    assert_eq!(steps[1].result, Ptr::Frame(0));
    assert_eq!(steps[1].events, [lock, unlock]);
    assert_eq!(steps[1].current, Ptr::Null);
    assert_eq!(steps[1].frames[0], Some((2, true)));
    assert_eq!(steps[2].result, Ptr::Null);
}

/// The shared answer of the characterized releases, pinned.
fn pin_releases(steps: &[Step]) {
    let lock = (1, Ptr::Mutex);
    let unlock = (2, Ptr::Mutex);
    let destroy = |f| (DESTROY, Ptr::Frame(f));
    // NULL frames: nothing.
    assert!(steps[3].events.is_empty() && steps[4].events.is_empty());
    assert_eq!(steps[4].log.0, 0);
    // Back from 2 to 1: returned to the cache (not in it here).
    assert_eq!(steps[5].events, [lock, unlock]);
    assert_eq!(steps[5].frames[0], Some((1, false)));
    // 3 to 2: the first slot holding it is marked unused; then 2 to 1.
    assert_eq!(steps[6].frames[1], Some((2, false)));
    assert_eq!(steps[6].cache[1], (Ptr::Frame(1), false, 2));
    assert_eq!(steps[6].cache[2], (Ptr::Frame(1), true, 5));
    assert_eq!(steps[7].frames[1], Some((1, false)));
    assert_eq!(steps[7].cache[2], (Ptr::Frame(1), true, 5));
    // A NULL source destroys whatever the references, without the lock.
    assert_eq!(steps[8].events, [destroy(2)]);
    assert_eq!(steps[8].frames[2], None);
    assert!(steps[8].cache[0].1);
    // The last reference destroys under the lock; the cache keeps its slot.
    assert_eq!(steps[9].events, [lock, destroy(3), unlock]);
    assert_eq!(steps[9].cache[3], (Ptr::Frame(3), true, 4));
    // Zero goes below zero and is returned, not destroyed.
    assert_eq!(steps[10].events, [lock, unlock]);
    assert_eq!(steps[10].frames[4], Some((-1, false)));
    assert_eq!(steps[10].cache[4], (Ptr::Frame(4), false, 0));
}

/// Mutants of the exports: each must disagree with the original.
unsafe extern "C" fn get_silent_null(source: *mut c_void) -> *mut c_void {
    if source.is_null() {
        return core::ptr::null_mut();
    }
    // SAFETY: a live oracle source.
    unsafe { rust_get(source) }
}
unsafe extern "C" fn get_without_reference(source: *mut c_void) -> *mut c_void {
    // SAFETY: NULL or a live oracle source.
    let frame = unsafe { rust_get(source) };
    if !frame.is_null() {
        // SAFETY: the live frame the export returned; drop the reference it
        // added.
        unsafe { oracle_grf_frame_drop_ref(frame) };
    }
    frame
}
unsafe extern "C" fn release_unlocked(source: *mut c_void, frame: *mut c_void) {
    let _ = source;
    // SAFETY: NULL or the caller's live frame: destroyed without the lock.
    unsafe { rust_release(core::ptr::null_mut(), frame) }
}
unsafe extern "C" fn release_keeps_null_source(source: *mut c_void, frame: *mut c_void) {
    if source.is_null() {
        return;
    }
    // SAFETY: a live oracle source and NULL or a live frame.
    unsafe { rust_release(source, frame) }
}

unsafe extern "C" {
    fn oracle_grf_frame_drop_ref(frame: *mut c_void);
}

#[test]
fn get_release_frame_mutants_are_caught() {
    if !crate::output::isolated(concat!(
        module_path!(),
        "::get_release_frame_mutants_are_caught"
    )) {
        return;
    }
    let scenario = characterized();
    assert!(agree(&scenario, rust_get, rust_release));
    for get in [get_silent_null as Get, get_without_reference] {
        assert!(
            !agree(&scenario, get, rust_release),
            "a get mutant agreed with C"
        );
    }
    for release in [release_unlocked as Release, release_keeps_null_source] {
        assert!(
            !agree(&scenario, rust_get, release),
            "a release mutant agreed with C"
        );
    }
}

fn scenarios() -> impl Strategy<Value = Scenario> {
    (1usize..5).prop_flat_map(|count| {
        let op = prop_oneof![
            any::<bool>().prop_map(|null_source| Op::Get { null_source }),
            (any::<bool>(), proptest::option::of(0..count))
                .prop_map(|(null_source, frame)| Op::Release { null_source, frame }),
        ];
        (
            proptest::collection::vec((-1 as c_long..4, any::<bool>()), count),
            proptest::option::of(0..count),
            proptest::collection::vec((0..count, any::<bool>(), 0 as c_long..3), 0..6),
            proptest::collection::vec(op, 1..12),
        )
            .prop_map(|(frames, current, cache, ops)| Scenario {
                frames,
                current,
                cache,
                ops,
            })
    })
}

#[test]
fn get_release_frame_sequences_match_c() {
    if !crate::output::isolated(concat!(
        module_path!(),
        "::get_release_frame_sequences_match_c"
    )) {
        return;
    }
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    });
    runner
        .run(&scenarios(), |scenario| {
            check(&scenario);
            Ok(())
        })
        .unwrap();
}

//! Tier 3: the shims of `obs_scene::ffi::item_audio_actions` against the
//! original `obs-scene-item-audio-actions.c.h` (the oracle, over the
//! original C core).
//!
//! Each probe builds two real `struct obs_scene_item`s in C from the same
//! queue, visibility and active references, runs one of the three calls on
//! each (the C on one, the Rust shim on the other, both removing the child
//! through the oracle's recorder), and compares:
//! - the return value of `apply_scene_item_volume()`;
//! - the item's `visible` and `active_refs`;
//! - its queue: `num`, `capacity` and every byte of the items it held,
//!   the stale ones past `num` included;
//! - the gain buffer bit for bit, and the gain past the tick;
//! - the trace of every lock, unlock, `visible` load and store,
//!   `active_refs` decrement (with its result) and child removal (with its
//!   two sources), in order. The C reaches them directly, the Rust through
//!   the production field adapter; the oracle compiles both with the same
//!   recorders.
//!
//! With an opt-in clobber, every unlock then rewrites the time of the first
//! queued action (on both sides alike), so a read of the queue moved out of
//! the lock gives a different result.
//!
//! Runs: every queue of up to 2 actions over chosen times (before the
//! tick, frames 0, 1 and 511, the last ns of frame 1023, frames 1024 and
//! 2000, and `u64::MAX`), each call, each start visibility, 0 to 2
//! references, on a tick and with `ts` 0, with and without a buffer; and a
//! proptest over random queues of up to 24 actions in any time order,
//! references from -3 to 8, ticks at 0, near 0, anywhere and near
//! `u64::MAX`, rates from 1 to 384 kHz plus the common ones, with and
//! without the clobber; and the window's edges (the last ns that is due and
//! the first that is not) at each common rate.
//!
//! Excluded, exactly the core's intentional differences (see
//! `obs_scene::audio_actions`): a buffer with `ts` 0, where the C writes
//! past the tick (libobs passes `ts` 0 only without one), and a rate of 0
//! for the window of `apply_scene_item_volume()` and
//! `process_all_audio_actions()`, where the C divides by 0. A rate of 0 for
//! `apply_scene_item_audio_actions()` is compared (the video tick passes
//! it). The null item, which the C dereferences, is checked on the Rust
//! side only.

use std::ffi::{c_long, c_void};
use std::sync::Mutex;

use obs_scene::ffi::item_audio_actions as shim;
use obs_scene::ffi::scene_audio_actions::item_action;
use proptest::prelude::*;

type ApplyFn = unsafe extern "C" fn(*mut c_void, *mut f32, u64, usize);
type VolumeFn = unsafe extern "C" fn(*mut c_void, *mut f32, u64, usize) -> bool;
type AllFn = unsafe extern "C" fn(*mut c_void, usize);

unsafe extern "C" {
    fn oracle_item_audio_actions_probe(
        op: u32,
        actions: *const item_action,
        count: usize,
        visible: bool,
        refs: c_long,
        ts: u64,
        sample_rate: usize,
        with_buf: bool,
        clobber: bool,
        clobber_ts: u64,
        apply: ApplyFn,
        volume: VolumeFn,
        all: AllFn,
    ) -> u32;
    fn oracle_item_audio_actions_remove(parent: *mut c_void, child: *mut c_void);
}

unsafe extern "C" fn rust_apply(item: *mut c_void, buf: *mut f32, ts: u64, rate: usize) {
    // SAFETY: the oracle passes its live item and buffer.
    unsafe {
        shim::apply_scene_item_audio_actions_with(
            item,
            buf,
            ts,
            rate,
            oracle_item_audio_actions_remove,
        )
    }
}

unsafe extern "C" fn rust_volume(item: *mut c_void, buf: *mut f32, ts: u64, rate: usize) -> bool {
    // SAFETY: as `rust_apply`.
    unsafe {
        shim::apply_scene_item_volume_with(item, buf, ts, rate, oracle_item_audio_actions_remove)
    }
}

unsafe extern "C" fn rust_all(item: *mut c_void, rate: usize) {
    // SAFETY: as `rust_apply`.
    unsafe { shim::process_all_audio_actions_with(item, rate, oracle_item_audio_actions_remove) }
}

/// The oracle's trace is one global: one probe at a time.
static PROBE: Mutex<()> = Mutex::new(());

const NS: u64 = 1_000_000_000;
const TICK: u64 = 1_000_000_000_000;
/// `apply_scene_item_audio_actions`, `apply_scene_item_volume`,
/// `process_all_audio_actions`.
const OPS: [u32; 3] = [0, 1, 2];

/// Whether the inputs are outside the excluded ones.
fn compared(op: u32, ts: u64, rate: usize, with_buf: bool) -> bool {
    !(with_buf && (ts == 0 || op == 2)) && !(op != 0 && rate == 0)
}

fn check(
    op: u32,
    actions: &[item_action],
    visible: bool,
    refs: c_long,
    ts: u64,
    rate: usize,
    with_buf: bool,
) {
    check_clobbered(op, actions, visible, refs, ts, rate, with_buf, None);
}

/// [`check`], with every unlock rewriting the first queued action's time
/// to `clobber` when it is set.
#[allow(clippy::too_many_arguments)]
fn check_clobbered(
    op: u32,
    actions: &[item_action],
    visible: bool,
    refs: c_long,
    ts: u64,
    rate: usize,
    with_buf: bool,
    clobber: Option<u64>,
) {
    let _probe = PROBE.lock().unwrap_or_else(|e| e.into_inner());
    // SAFETY: the oracle copies the actions and owns both items.
    let diff = unsafe {
        oracle_item_audio_actions_probe(
            op,
            actions.as_ptr(),
            actions.len(),
            visible,
            refs,
            ts,
            rate,
            with_buf,
            clobber.is_some(),
            clobber.unwrap_or(0),
            rust_apply,
            rust_volume,
            rust_all,
        )
    };
    assert_eq!(
        diff, 0,
        "diff bits {diff:#x}: op {op}, actions {actions:?}, visible {visible}, refs {refs}, \
         ts {ts}, rate {rate}, buf {with_buf}, clobber {clobber:?}"
    );
}

fn action(visible: bool, timestamp: u64) -> item_action {
    item_action { visible, timestamp }
}

/// The first time on frame `k` of the tick at `ts`.
fn at_frame(ts: u64, k: u64, rate: u64) -> u64 {
    ts + (k * NS).div_ceil(rate)
}

#[test]
fn bad_inputs_are_refused() {
    // The probe refuses exactly the excluded inputs.
    let _probe = PROBE.lock().unwrap_or_else(|e| e.into_inner());
    for (op, ts, rate, with_buf) in [
        (0, 0, 48000, true),
        (2, TICK, 48000, true),
        (1, TICK, 0, false),
        (2, 0, 0, false),
    ] {
        assert!(!compared(op, ts, rate, with_buf));
        // SAFETY: an empty queue; the probe returns before running anything.
        let diff = unsafe {
            oracle_item_audio_actions_probe(
                op,
                std::ptr::null(),
                0,
                true,
                1,
                ts,
                rate,
                with_buf,
                false,
                0,
                rust_apply,
                rust_volume,
                rust_all,
            )
        };
        assert_eq!(diff, 1 << 6);
    }
}

#[test]
fn chosen_queues_match_c() {
    let rate = 48000u64;
    let times = [
        TICK - 1,
        TICK,
        at_frame(TICK, 1, rate),
        at_frame(TICK, 511, rate),
        at_frame(TICK, 1024, rate) - 1,
        at_frame(TICK, 1024, rate),
        at_frame(TICK, 2000, rate),
        u64::MAX,
    ];
    let mut queues: Vec<Vec<item_action>> = vec![Vec::new()];
    for &t in &times {
        for v in [false, true] {
            queues.push(vec![action(v, t)]);
            for &t2 in &times {
                for v2 in [false, true] {
                    queues.push(vec![action(v, t), action(v2, t2)]);
                }
            }
        }
    }
    for queue in &queues {
        for op in OPS {
            for visible in [false, true] {
                for refs in 0..=2 {
                    for ts in [TICK, 0] {
                        for with_buf in [false, true] {
                            if compared(op, ts, rate as usize, with_buf) {
                                check(op, queue, visible, refs, ts, rate as usize, with_buf);
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn window_edges_match_c() {
    // The last ns that is due, and the first that is not, at each rate.
    for rate in [8000u64, 22050, 44100, 48000, 96000, 192000, 1, 7, 384_000] {
        let duration = 1024 * NS / rate;
        for t in [TICK + duration - 1, TICK + duration, TICK + duration + 1] {
            for op in [0, 1] {
                for visible in [false, true] {
                    check(
                        op,
                        &[action(!visible, t)],
                        visible,
                        1,
                        TICK,
                        rate as usize,
                        true,
                    );
                }
            }
        }
    }
}

#[test]
fn queue_read_under_the_lock() {
    // Each unlock rewrites the first action's time: a read of the queue
    // after the unlock would see it.
    for first in [TICK, TICK + NS] {
        for clobber in [0, TICK, TICK + NS, u64::MAX] {
            for op in OPS {
                let queue = [action(false, first), action(true, first)];
                let with_buf = op != 2;
                check_clobbered(op, &queue, true, 2, TICK, 48000, with_buf, Some(clobber));
            }
        }
    }
}

#[test]
fn video_tick_rate_0_matches_c() {
    // scene_video_tick() passes ts 0 and rate 0, with no buffer.
    for refs in -1..=2 {
        check(
            0,
            &[
                action(false, TICK),
                action(true, 0),
                action(false, u64::MAX),
            ],
            true,
            refs,
            0,
            0,
            false,
        );
    }
    // A tick with rate 0 puts every action on frame 0.
    check(
        0,
        &[action(false, TICK + 3600 * NS)],
        true,
        1,
        TICK,
        0,
        true,
    );
}

#[test]
fn null_item_does_nothing() {
    // Rust only: the C dereferences it.
    let mut gains = [0.5f32; 1024];
    // SAFETY: a null item returns before anything is read or written.
    unsafe {
        shim::apply_scene_item_audio_actions(std::ptr::null_mut(), gains.as_mut_ptr(), TICK, 48000);
        assert!(!shim::apply_scene_item_volume(
            std::ptr::null_mut(),
            gains.as_mut_ptr(),
            TICK,
            48000
        ));
        shim::process_all_audio_actions(std::ptr::null_mut(), 48000);
    }
    assert!(gains.iter().all(|g| *g == 0.5));
}

fn timestamp(tick: u64) -> impl Strategy<Value = u64> {
    prop_oneof![
        Just(0u64),
        Just(u64::MAX),
        (0u64..2_000_000_000).prop_map(move |d| tick.saturating_add(d)),
        (0u64..100_000_000).prop_map(move |d| tick.saturating_sub(d)),
        any::<u64>(),
    ]
}

fn tick() -> impl Strategy<Value = u64> {
    prop_oneof![
        Just(0u64),
        1u64..1_000,
        Just(TICK),
        any::<u64>(),
        (u64::MAX - 1_000_000_000)..=u64::MAX,
    ]
}

fn rate() -> impl Strategy<Value = usize> {
    prop_oneof![
        prop::sample::select(vec![8000usize, 22050, 44100, 48000, 96000, 192000]),
        1usize..=384_000,
    ]
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(512),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn random_queues_match_c(
        op in 0u32..3,
        (ts, queue) in tick().prop_flat_map(|ts| {
            (Just(ts), prop::collection::vec((any::<bool>(), timestamp(ts)), 0..=24))
        }),
        visible in any::<bool>(),
        refs in (-3i8..=8).prop_map(c_long::from),
        rate in rate(),
        with_buf in any::<bool>(),
        clobber in prop::option::of(any::<u64>()),
    ) {
        let queue: Vec<item_action> = queue.into_iter().map(|(v, t)| action(v, t)).collect();
        let with_buf = with_buf && compared(op, ts, rate, true);
        check_clobbered(op, &queue, visible, refs, ts, rate, with_buf, clobber);
    }
}

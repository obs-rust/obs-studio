//! Tier 1: `obs_scene::audio_actions`, the scene item visibility audio ramp.
//!
//! The `steps_*` cases port `test/cmocka/test_scene_audio_action_steps.c`
//! 1:1, under the same case names, in the same order, with the same steps
//! and expectations. That test runs the C item side
//! (`obs-scene-item-audio-actions.c.h`) on a fake scene item, so [`Fake`]
//! repeats that item side around the core: the item's queue, visibility and
//! active references, the removal of the active child when the count
//! reaches 0 (counted), `apply_scene_item_volume()` and
//! `process_all_audio_actions()`. The buffer has one gain more than a tick,
//! which no case may write.
//!
//! The `edge_*` cases are new: the frame helper, the window at the ends of
//! the `u64` range, the iterator left unread after the stop, and the two
//! intentional differences from C (the `ts` 0 fill stops at the buffer end;
//! a rate of 0 makes every action due).

use obs_scene::audio_actions::{
    AUDIO_OUTPUT_FRAMES, Applied, Gains, ItemAction, action_due, apply, frame, tick_duration,
};

const FRAMES: usize = AUDIO_OUTPUT_FRAMES;
const NS: u64 = 1_000_000_000;
/// A tick timestamp well away from 0, which the C treats as "no tick".
const TICK: u64 = 1_000_000_000_000;
/// Neither 0 nor 1: a gain the core did not write.
const UNWRITTEN: f32 = 0.5;

/// The first time that the C places on frame `k` of the tick at `ts`.
fn at_frame(ts: u64, k: u64, rate: u64) -> u64 {
    ts + (k * NS).div_ceil(rate)
}

/// The C's `util_mul_div64` for the values the cases use (no overflow).
fn mul_div(num: u64, mul: u64, div: u64) -> u64 {
    (u128::from(num) * u128::from(mul) / u128::from(div)) as u64
}

/// The fake scene item of the C test, with the C item side around the core.
struct Fake {
    visible: bool,
    active_refs: i64,
    removals: i64,
    queue: Vec<ItemAction>,
    buf: [f32; FRAMES + 1],
}

impl Fake {
    fn new(visible: bool, refs: i64) -> Self {
        Fake {
            visible,
            active_refs: refs,
            removals: 0,
            queue: Vec::new(),
            buf: [UNWRITTEN; FRAMES + 1],
        }
    }

    fn push(&mut self, visible: bool, timestamp: u64) {
        self.queue.push(ItemAction { visible, timestamp });
    }

    /// `apply_scene_item_audio_actions()`: the core on the queue and the
    /// visibility, the consumed actions erased from the front, then one
    /// reference per hide, the child removed when the count reaches 0.
    fn apply(&mut self, with_buf: bool, ts: u64, rate: u64) {
        let gains: Option<&mut Gains> = if with_buf {
            Some((&mut self.buf[..FRAMES]).try_into().unwrap())
        } else {
            None
        };
        let out = apply(self.queue.iter().copied(), self.visible, ts, rate, gains);
        assert!(out.consumed <= self.queue.len());
        self.queue.drain(..out.consumed);
        self.visible = out.visible;
        for _ in 0..out.hides {
            self.active_refs -= 1;
            if self.active_refs == 0 {
                self.removals += 1;
            }
        }
    }

    /// `apply_scene_item_volume()`: applies the tick when the first queued
    /// action is due.
    fn volume(&mut self, with_buf: bool, ts: u64, rate: u64) -> bool {
        match self.queue.first() {
            Some(first) if action_due(first.timestamp, ts, rate) => {
                self.apply(with_buf, ts, rate);
                true
            }
            _ => false,
        }
    }

    /// `process_all_audio_actions()`.
    fn process_all(&mut self, rate: u64) {
        while self.volume(false, 0, rate) {}
    }

    /// `buf[from..to]` is all `value`.
    fn check_span(&self, from: usize, to: usize, value: f32) {
        for (i, gain) in self.buf[from..to].iter().enumerate() {
            assert!(
                *gain == value,
                "sample {} is {gain}, expected {value} (span {from}-{to})",
                from + i
            );
        }
    }

    fn check_untouched(&self) {
        self.check_span(0, FRAMES + 1, UNWRITTEN);
    }

    fn check_item(&self, visible: bool, refs: i64, removed: i64, pending: usize) {
        assert_eq!(self.visible, visible, "visible");
        assert_eq!(self.active_refs, refs, "active_refs");
        assert_eq!(self.removals, removed, "removals");
        assert_eq!(self.queue.len(), pending, "pending");
    }
}

impl Drop for Fake {
    /// `fake_free()`: the C never writes past the tick.
    fn drop(&mut self) {
        if !std::thread::panicking() {
            assert!(
                self.buf[FRAMES] == UNWRITTEN,
                "a gain past the tick was written"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// apply_scene_item_audio_actions

/// No actions: the buffer is the current visibility, whole.
#[test]
fn steps_no_actions() {
    let mut f = Fake::new(true, 1);
    f.apply(true, TICK, 48000);
    f.check_span(0, FRAMES, 1.0);
    f.check_item(true, 1, 0, 0);

    let mut f = Fake::new(false, 1);
    f.apply(true, TICK, 48000);
    f.check_span(0, FRAMES, 0.0);
    f.check_item(false, 1, 0, 0);

    // no buffer: nothing to do
    let mut f = Fake::new(true, 1);
    f.apply(false, TICK, 48000);
    f.check_item(true, 1, 0, 0);
}

/// A hide, and a show, on every frame of the tick at 48 kHz: the change is
/// on exactly that frame, and one nanosecond earlier is one frame earlier.
#[test]
fn steps_one_change_on_each_frame() {
    for k in 0..FRAMES as u64 {
        let ku = k as usize;
        let mut f = Fake::new(true, 1);
        f.push(false, at_frame(TICK, k, 48000));
        f.apply(true, TICK, 48000);
        f.check_span(0, ku, 1.0);
        f.check_span(ku, FRAMES, 0.0);
        f.check_item(false, 0, 1, 0);

        let mut f = Fake::new(false, 1);
        f.push(true, at_frame(TICK, k, 48000));
        f.apply(true, TICK, 48000);
        f.check_span(0, ku, 0.0);
        f.check_span(ku, FRAMES, 1.0);
        f.check_item(true, 1, 0, 0);

        if k != 0 {
            let mut f = Fake::new(true, 1);
            f.push(false, at_frame(TICK, k, 48000) - 1);
            f.apply(true, TICK, 48000);
            f.check_span(0, ku - 1, 1.0);
            f.check_span(ku - 1, FRAMES, 0.0);
        }
    }
}

/// The same placement at the rates libobs offers, and odd ones.
#[test]
fn steps_one_change_at_each_rate() {
    const RATES: [u64; 11] = [
        8000, 11025, 16000, 22050, 24000, 32000, 44100, 48000, 88200, 96000, 192000,
    ];
    const AT: [u64; 8] = [0, 1, 2, 511, 512, 1000, 1022, 1023];

    for rate in RATES {
        for k in AT {
            let mut f = Fake::new(true, 1);
            f.push(false, at_frame(TICK, k, rate));
            f.apply(true, TICK, rate);
            f.check_span(0, k as usize, 1.0);
            f.check_span(k as usize, FRAMES, 0.0);
            f.check_item(false, 0, 1, 0);
        }
    }
}

/// An action on frame 1024 or later belongs to a later tick: it stays
/// queued and the buffer keeps the current visibility.
#[test]
fn steps_action_past_the_tick() {
    let mut f = Fake::new(true, 1);
    f.push(false, at_frame(TICK, FRAMES as u64, 48000));
    f.apply(true, TICK, 48000);
    f.check_span(0, FRAMES, 1.0);
    f.check_item(true, 1, 0, 1);
    assert_eq!(f.queue[0].timestamp, at_frame(TICK, FRAMES as u64, 48000));

    // the last nanosecond of frame 1023 is still this tick
    let mut f = Fake::new(true, 1);
    f.push(false, at_frame(TICK, FRAMES as u64, 48000) - 1);
    f.apply(true, TICK, 48000);
    f.check_span(0, FRAMES - 1, 1.0);
    f.check_span(FRAMES - 1, FRAMES, 0.0);
    f.check_item(false, 0, 1, 0);
}

/// An action timed before the tick (late) lands on frame 0.
#[test]
fn steps_late_action_on_frame_0() {
    let mut f = Fake::new(true, 1);
    f.push(false, TICK - 5 * NS);
    f.apply(true, TICK, 48000);
    f.check_span(0, FRAMES, 0.0);
    f.check_item(false, 0, 1, 0);

    // timestamp 0 too
    let mut f = Fake::new(false, 1);
    f.push(true, 0);
    f.apply(true, TICK, 48000);
    f.check_span(0, FRAMES, 1.0);
    f.check_item(true, 1, 0, 0);
}

/// Several toggles in one tick: each span has the visibility before its
/// change, and every hide is counted.
#[test]
fn steps_several_changes_in_one_tick() {
    let mut f = Fake::new(true, 2);
    f.push(false, at_frame(TICK, 100, 48000));
    f.push(true, at_frame(TICK, 300, 48000));
    f.push(false, at_frame(TICK, 700, 48000));
    f.apply(true, TICK, 48000);
    f.check_span(0, 100, 1.0);
    f.check_span(100, 300, 0.0);
    f.check_span(300, 700, 1.0);
    f.check_span(700, FRAMES, 0.0);
    f.check_item(false, 0, 1, 0);

    // two changes on one frame: the span between them is empty
    let mut f = Fake::new(true, 2);
    f.push(false, at_frame(TICK, 400, 48000));
    f.push(true, at_frame(TICK, 400, 48000));
    f.apply(true, TICK, 48000);
    f.check_span(0, FRAMES, 1.0);
    f.check_item(true, 1, 0, 0);

    // on frame 0, then again later
    let mut f = Fake::new(true, 2);
    f.push(false, TICK);
    f.push(true, at_frame(TICK, 1, 48000));
    f.push(false, at_frame(TICK, 1023, 48000));
    f.apply(true, TICK, 48000);
    f.check_span(0, 1, 0.0);
    f.check_span(1, 1023, 1.0);
    f.check_span(1023, FRAMES, 0.0);
    f.check_item(false, 0, 1, 0);
}

/// Actions in this tick are consumed; the first one past it and everything
/// after it stay queued, in order.
#[test]
fn steps_later_actions_stay_queued() {
    let mut f = Fake::new(true, 1);
    f.push(false, at_frame(TICK, 200, 48000));
    f.push(true, at_frame(TICK, 1500, 48000));
    f.push(false, at_frame(TICK, 3000, 48000));
    f.apply(true, TICK, 48000);
    f.check_span(0, 200, 1.0);
    f.check_span(200, FRAMES, 0.0);
    f.check_item(false, 0, 1, 2);
    assert!(f.queue[0].visible);
    assert_eq!(f.queue[0].timestamp, at_frame(TICK, 1500, 48000));
    assert!(!f.queue[1].visible);
    assert_eq!(f.queue[1].timestamp, at_frame(TICK, 3000, 48000));

    // the next tick picks them up from its own start
    let next = TICK + mul_div(FRAMES as u64, NS, 48000);
    f.apply(true, next, 48000);
    let show = mul_div(at_frame(TICK, 1500, 48000) - next, 48000, NS) as usize;
    assert_eq!(show, 1500 - FRAMES);
    f.check_span(0, show, 0.0);
    f.check_span(show, FRAMES, 1.0);
    f.check_item(true, 0, 1, 1);
}

/// Actions out of time order (two threads queuing at once).
/// Characterized, not endorsed: a change timed before the previous one does
/// not rewrite the buffer, so the hide below never reaches the gain buffer,
/// but it is counted. And the loop stops at the first action past the tick,
/// even when a later one is due.
#[test]
fn steps_actions_out_of_order() {
    let mut f = Fake::new(true, 1);
    f.push(false, at_frame(TICK, 600, 48000));
    f.push(true, at_frame(TICK, 200, 48000));
    f.apply(true, TICK, 48000);
    f.check_span(0, FRAMES, 1.0);
    f.check_item(true, 0, 1, 0);

    let mut f = Fake::new(true, 1);
    f.push(true, at_frame(TICK, 1500, 48000));
    f.push(false, at_frame(TICK, 100, 48000));
    f.apply(true, TICK, 48000);
    f.check_span(0, FRAMES, 1.0);
    f.check_item(true, 1, 0, 2);
}

/// The hide count: each applied hide drops one active reference, and the
/// one that reaches 0 removes the active child, once. Shows take no
/// reference here (obs_sceneitem_set_visible() takes it when it queues the
/// show).
#[test]
fn steps_hides_drop_references() {
    // hide, show, hide, show, hide, from 3 references
    let mut f = Fake::new(true, 3);
    for i in 0..5u64 {
        f.push(i % 2 != 0, at_frame(TICK, 100 + 100 * i, 48000));
    }
    f.apply(true, TICK, 48000);
    f.check_item(false, 0, 1, 0);

    // references to spare: no removal
    let mut f = Fake::new(true, 5);
    f.push(false, at_frame(TICK, 10, 48000));
    f.push(true, at_frame(TICK, 20, 48000));
    f.push(false, at_frame(TICK, 30, 48000));
    f.apply(true, TICK, 48000);
    f.check_item(false, 3, 0, 0);

    // a hide of a hidden item still counts
    let mut f = Fake::new(false, 2);
    f.push(false, at_frame(TICK, 10, 48000));
    f.apply(true, TICK, 48000);
    f.check_span(0, FRAMES, 0.0);
    f.check_item(false, 1, 0, 0);

    // Characterized, not endorsed: more hides than references go below 0
    // and remove the child only at 0. obs_sceneitem_set_visible() keeps
    // them paired, so libobs does not get here.
    let mut f = Fake::new(true, 1);
    f.push(false, at_frame(TICK, 10, 48000));
    f.push(true, at_frame(TICK, 20, 48000));
    f.push(false, at_frame(TICK, 30, 48000));
    f.apply(true, TICK, 48000);
    f.check_item(false, -1, 1, 0);
}

/// ts 0 (the preview-only scene's video tick: no buffer, rate 0): every
/// action is applied, whatever its time, and the hides are counted.
#[test]
fn steps_ts_0_applies_everything() {
    let mut f = Fake::new(true, 2);
    f.push(false, TICK);
    f.push(true, TICK + 3600 * NS);
    f.push(false, u64::MAX);
    f.apply(false, 0, 0);
    f.check_item(false, 0, 1, 0);

    // with a rate (process_all_audio_actions() passes one)
    let mut f = Fake::new(false, 1);
    f.push(true, TICK);
    f.push(false, TICK + 3600 * NS);
    f.push(true, u64::MAX);
    f.apply(false, 0, 192000);
    f.check_item(true, 0, 1, 0);

    // a tick with rate 0 puts every action on frame 0 (libobs passes rate 0
    // only with ts 0)
    let mut f = Fake::new(true, 1);
    f.push(false, TICK + 3600 * NS);
    f.apply(true, TICK, 0);
    f.check_span(0, FRAMES, 0.0);
    f.check_item(false, 0, 1, 0);
}

// ---------------------------------------------------------------------------
// apply_scene_item_volume

/// Nothing queued: false, and the buffer is not written (the mix then uses
/// the item's visibility, not the buffer).
#[test]
fn steps_volume_without_actions() {
    let mut f = Fake::new(true, 1);
    assert!(!f.volume(true, TICK, 48000));
    f.check_untouched();
    f.check_item(true, 1, 0, 0);
}

/// The first action decides: due before ts + duration (1024 frames in ns,
/// rounded down), it applies the tick and returns true; later, false with
/// the buffer not written.
#[test]
fn steps_volume_window() {
    const RATES: [u64; 6] = [8000, 22050, 44100, 48000, 96000, 192000];

    for rate in RATES {
        let duration = FRAMES as u64 * NS / rate;

        let mut f = Fake::new(true, 1);
        f.push(false, TICK + duration - 1);
        assert!(f.volume(true, TICK, rate));
        f.check_span(0, FRAMES - 1, 1.0);
        f.check_span(FRAMES - 1, FRAMES, 0.0);
        f.check_item(false, 0, 1, 0);

        let mut f = Fake::new(true, 1);
        f.push(false, TICK + duration);
        assert!(!f.volume(true, TICK, rate));
        f.check_untouched();
        f.check_item(true, 1, 0, 1);
    }

    // Characterized, not endorsed: where 1024 frames are not a whole number
    // of nanoseconds, the rounded-down duration leaves out the end of the
    // last frame. At 44.1 kHz an action at ts + duration is on frame 1023,
    // but it waits for the next tick, which puts it on that tick's frame 0
    // (one frame late).
    let mut f = Fake::new(true, 1);
    let duration = FRAMES as u64 * NS / 44100;
    assert_eq!(mul_div(duration, 44100, NS), FRAMES as u64 - 1);
    f.push(false, TICK + duration);
    assert!(!f.volume(true, TICK, 44100));
    f.check_item(true, 1, 0, 1);
}

/// The window looks at the first action only (characterized, not endorsed:
/// with actions out of order, a due one waits behind one that is not).
#[test]
fn steps_volume_first_action_only() {
    let mut f = Fake::new(true, 1);
    f.push(true, at_frame(TICK, 5000, 48000));
    f.push(false, at_frame(TICK, 10, 48000));
    assert!(!f.volume(true, TICK, 48000));
    f.check_untouched();
    f.check_item(true, 1, 0, 2);
}

/// ts 0: true whenever an action is queued, applied whatever its time.
#[test]
fn steps_volume_ts_0() {
    let mut f = Fake::new(true, 1);
    f.push(false, u64::MAX);
    assert!(f.volume(false, 0, 48000));
    f.check_item(false, 0, 1, 0);
    assert!(!f.volume(false, 0, 48000));
}

// ---------------------------------------------------------------------------
// process_all_audio_actions

/// No audio playing in the scene: everything queued is applied now, future
/// actions included, and every hide is counted.
#[test]
fn steps_process_all() {
    let mut f = Fake::new(true, 2);
    f.push(false, TICK);
    f.push(true, TICK + NS);
    f.push(false, TICK + 3600 * NS);
    f.process_all(48000);
    f.check_item(false, 0, 1, 0);

    let mut f = Fake::new(false, 1);
    f.process_all(48000);
    f.check_item(false, 1, 0, 0);
}

// ---------------------------------------------------------------------------
// The core: obs_scene_audio_actions_apply() and obs_scene_audio_action_due()

/// The core on a queue and a visibility of its own, as
/// `obs_scene_audio_actions_apply()` takes them: the consumed actions are
/// erased from the front, and the hide count is returned.
fn core(
    actions: &mut Vec<ItemAction>,
    visible: &mut bool,
    ts: u64,
    rate: u64,
    buf: Option<&mut Gains>,
) -> usize {
    let out = apply(actions.iter().copied(), *visible, ts, rate, buf);
    actions.drain(..out.consumed);
    *visible = out.visible;
    out.hides
}

fn gains(f: &mut Fake) -> Option<&mut Gains> {
    Some((&mut f.buf[..FRAMES]).try_into().unwrap())
}

/// The core works on the fields alone: a queue and a visibility, with no
/// item, no lock and no source. It consumes the same actions, writes the
/// same buffer and visibility, and returns the hide count; it drops no
/// reference and removes no child (that stays with the item's caller).
#[test]
fn steps_core_applies_the_fields() {
    let mut actions = vec![
        ItemAction {
            visible: false,
            timestamp: at_frame(TICK, 100, 48000),
        },
        ItemAction {
            visible: true,
            timestamp: at_frame(TICK, 300, 48000),
        },
        ItemAction {
            visible: false,
            timestamp: at_frame(TICK, 700, 48000),
        },
        ItemAction {
            visible: true,
            timestamp: at_frame(TICK, 1500, 48000),
        },
    ];

    let mut f = Fake::new(true, 0);
    let mut visible = true;
    assert_eq!(
        core(&mut actions, &mut visible, TICK, 48000, gains(&mut f)),
        2
    );
    f.check_span(0, 100, 1.0);
    f.check_span(100, 300, 0.0);
    f.check_span(300, 700, 1.0);
    f.check_span(700, FRAMES, 0.0);
    assert!(!visible);
    assert_eq!(actions.len(), 1);
    assert!(actions[0].visible);
    assert_eq!(actions[0].timestamp, at_frame(TICK, 1500, 48000));
    assert_eq!(f.removals, 0);
    // the fake item is not touched
    f.check_item(true, 0, 0, 0);

    // nothing due: 0 hides, the buffer the visibility, the queue kept
    assert_eq!(
        core(&mut actions, &mut visible, TICK, 48000, gains(&mut f)),
        0
    );
    f.check_span(0, FRAMES, 0.0);
    assert!(!visible);
    assert_eq!(actions.len(), 1);

    // ts 0 with no buffer: everything, whatever its time
    actions.push(ItemAction {
        visible: false,
        timestamp: u64::MAX,
    });
    assert_eq!(core(&mut actions, &mut visible, 0, 0, None), 1);
    assert!(!visible);
    assert_eq!(actions.len(), 0);

    // an empty queue
    visible = true;
    assert_eq!(
        core(&mut actions, &mut visible, TICK, 48000, gains(&mut f)),
        0
    );
    f.check_span(0, FRAMES, 1.0);
    assert!(visible);
}

/// The volume window of apply_scene_item_volume(), on its own: an action is
/// due before ts + 1024 frames in ns (rounded down), or whenever ts is 0.
#[test]
fn steps_core_action_due() {
    const RATES: [u64; 6] = [8000, 22050, 44100, 48000, 96000, 192000];

    for rate in RATES {
        let duration = FRAMES as u64 * NS / rate;
        assert!(action_due(0, TICK, rate));
        assert!(action_due(TICK + duration - 1, TICK, rate));
        assert!(!action_due(TICK + duration, TICK, rate));
        assert!(!action_due(u64::MAX, TICK, rate));
    }

    // ts 0: due whatever the time
    assert!(action_due(u64::MAX, 0, 48000));
    assert!(action_due(0, 0, 48000));
}

// ---------------------------------------------------------------------------
// New edge cases

/// The frame of a time: floor, from the tick's start, a late one on 0.
#[test]
fn edge_frame() {
    assert_eq!(frame(TICK, TICK, 48000), 0);
    assert_eq!(frame(0, TICK, 48000), 0);
    assert_eq!(frame(TICK - 1, TICK, 48000), 0);
    assert_eq!(frame(at_frame(TICK, 7, 48000), TICK, 48000), 7);
    assert_eq!(frame(at_frame(TICK, 7, 48000) - 1, TICK, 48000), 6);
    assert_eq!(frame(TICK + 3600 * NS, TICK, 0), 0);
    // ts 0: the time itself, not bounded by a tick
    assert_eq!(frame(NS, 0, 48000), 48000);
    // util_mul_div64 wraps where (n % 1e9) * rate overflows; at real rates
    // it does not, so the frame of u64::MAX is exact
    assert_eq!(frame(u64::MAX, 0, 48000), mul_div(u64::MAX, 48000, NS));
}

/// The tick duration and the window at the ends of the u64 range: the sum
/// wraps as the C's uint64_t does.
#[test]
fn edge_due_wraps() {
    assert_eq!(tick_duration(48000), Some(21_333_333));
    assert_eq!(tick_duration(44100), Some(23_219_954));
    assert_eq!(tick_duration(1), Some(1024 * NS));
    let ts = u64::MAX - 10;
    // ts + duration wraps to a small value: nothing at or after it is due
    assert!(!action_due(u64::MAX, ts, 48000));
    assert!(!action_due(ts, ts, 48000));
    assert!(action_due(5, ts, 48000));
}

/// Intentional difference: a rate of 0 makes every action due (C divides
/// by 0). apply() puts them all on frame 0 at rate 0, so they all apply.
#[test]
fn edge_due_at_rate_0() {
    assert_eq!(tick_duration(0), None);
    assert!(action_due(u64::MAX, TICK, 0));
    assert!(action_due(0, TICK, 0));
    assert!(action_due(TICK, 0, 0));
}

/// Intentional difference: with ts 0 and a buffer, the C fill is not
/// bounded and writes past the tick; the core writes the same gains inside
/// the buffer and none past it.
#[test]
fn edge_ts_0_fill_stops_at_the_buffer() {
    let mut f = Fake::new(true, 1);
    // frame 500, then frame 48000 (one second), then u64::MAX
    f.push(false, 500 * NS / 48000 + 1);
    f.push(true, NS);
    f.push(false, u64::MAX);
    f.apply(true, 0, 48000);
    f.check_span(0, 500, 1.0);
    f.check_span(500, FRAMES, 0.0);
    f.check_item(false, -1, 1, 0);
    // Drop checks the gain past the buffer

    // the tail after a change in the buffer still fills
    let mut f = Fake::new(false, 1);
    f.push(true, 10 * NS / 48000 + 1);
    f.apply(true, 0, 48000);
    f.check_span(0, 10, 0.0);
    f.check_span(10, FRAMES, 1.0);
}

/// The core reads the queue in order and stops at the first action past the
/// tick: what it consumed is a prefix, and it reads no further.
#[test]
fn edge_consumes_a_prefix() {
    let actions = [
        ItemAction {
            visible: false,
            timestamp: at_frame(TICK, 10, 48000),
        },
        ItemAction {
            visible: true,
            timestamp: at_frame(TICK, 2000, 48000),
        },
        ItemAction {
            visible: false,
            timestamp: at_frame(TICK, 20, 48000),
        },
    ];
    let mut read = 0;
    let out = apply(
        actions.iter().inspect(|_| read += 1).copied(),
        true,
        TICK,
        48000,
        None,
    );
    assert_eq!(
        out,
        Applied {
            consumed: 1,
            visible: false,
            hides: 1
        }
    );
    assert_eq!(read, 2);

    // nothing consumed: the visibility is the one passed in
    let out = apply(actions[1..2].iter().copied(), false, TICK, 48000, None);
    assert_eq!(
        out,
        Applied {
            consumed: 0,
            visible: false,
            hides: 0
        }
    );
}

/// The gains are exactly 1.0 and 0.0, as the C writes them.
#[test]
fn edge_gain_values() {
    let mut f = Fake::new(true, 1);
    f.push(false, at_frame(TICK, 1, 48000));
    f.apply(true, TICK, 48000);
    assert_eq!(f.buf[0].to_bits(), 1.0f32.to_bits());
    assert_eq!(f.buf[1].to_bits(), 0.0f32.to_bits());
}

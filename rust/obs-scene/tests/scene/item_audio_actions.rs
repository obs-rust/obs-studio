//! Tier 1: `obs_scene::item_audio_actions`, the scene item's side of the
//! visibility audio ramp.
//!
//! The `steps_*` cases port every item case of
//! `test/cmocka/test_scene_audio_action_steps.c` (all but the two `core_*`
//! cases, which `audio_actions` covers) 1:1, under the same names, in the
//! same order, with the same steps and expectations. Its fake item is [`Model`] here: the queue, the
//! visibility and the active references, with the real Rust core
//! (`audio_actions::apply`) behind `apply_queue`, and every call the item
//! side makes recorded in order. The buffer has one gain more than a tick,
//! which no case may write.
//!
//! The `order_*` cases are new: they pin where each call happens, which the
//! C test can't see. The visibility is read and written back under the
//! lock, the first action is read under it, and the references are dropped
//! and the child removed only after the unlock.

use std::cell::RefCell;

use obs_scene::audio_actions::{self, AUDIO_OUTPUT_FRAMES, Gains, ItemAction};
use obs_scene::item_audio_actions::{
    ItemAudio, apply_scene_item_audio_actions, apply_scene_item_volume, process_all_audio_actions,
};

const FRAMES: usize = AUDIO_OUTPUT_FRAMES;
const NS: u64 = 1_000_000_000;
/// A tick timestamp well away from 0, which the C treats as "no tick".
const TICK: u64 = 1_000_000_000_000;
/// Neither 0 nor 1: a gain the item side did not write.
const UNWRITTEN: f32 = 0.5;

/// The first time that the C places on frame `k` of the tick at `ts`.
fn at_frame(ts: u64, k: u64, rate: u64) -> u64 {
    ts + (k * NS).div_ceil(rate)
}

/// The C's `util_mul_div64` for the values the cases use (no overflow).
fn mul_div(num: u64, mul: u64, div: u64) -> u64 {
    (u128::from(num) * u128::from(mul) / u128::from(div)) as u64
}

/// One call the item side made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Call {
    Lock,
    Unlock,
    Visible(bool),
    SetVisible(bool),
    First,
    Apply { visible: bool, ts: u64, buf: bool },
    Release(i64),
    Remove,
}

/// The fake scene item: what `test_scene_audio_action_steps.c`'s `struct
/// fake` holds, and a log of the calls.
struct Model {
    queue: RefCell<Vec<ItemAction>>,
    visible: RefCell<bool>,
    refs: RefCell<i64>,
    locked: RefCell<bool>,
    calls: RefCell<Vec<Call>>,
}

impl Model {
    fn new(visible: bool, refs: i64) -> Self {
        Model {
            queue: RefCell::new(Vec::new()),
            visible: RefCell::new(visible),
            refs: RefCell::new(refs),
            locked: RefCell::new(false),
            calls: RefCell::new(Vec::new()),
        }
    }

    fn push(&self, visible: bool, timestamp: u64) {
        self.queue
            .borrow_mut()
            .push(ItemAction { visible, timestamp });
    }

    fn log(&self, call: Call) {
        self.calls.borrow_mut().push(call);
    }

    fn removals(&self) -> usize {
        self.calls
            .borrow()
            .iter()
            .filter(|c| **c == Call::Remove)
            .count()
    }

    /// `check_item()`: visibility, references, removals and queue length.
    fn check(&self, visible: bool, refs: i64, removed: usize, pending: usize) {
        assert_eq!(*self.visible.borrow(), visible, "visible");
        assert_eq!(*self.refs.borrow(), refs, "active_refs");
        assert_eq!(self.removals(), removed, "removals");
        assert_eq!(self.queue.borrow().len(), pending, "pending actions");
        assert!(!*self.locked.borrow(), "left locked");
    }
}

impl ItemAudio for Model {
    fn lock(&self) {
        assert!(!*self.locked.borrow(), "locked twice");
        *self.locked.borrow_mut() = true;
        self.log(Call::Lock);
    }
    fn unlock(&self) {
        assert!(*self.locked.borrow(), "unlocked while not locked");
        *self.locked.borrow_mut() = false;
        self.log(Call::Unlock);
    }
    fn visible(&self) -> bool {
        let v = *self.visible.borrow();
        self.log(Call::Visible(v));
        v
    }
    fn set_visible(&self, visible: bool) {
        *self.visible.borrow_mut() = visible;
        self.log(Call::SetVisible(visible));
    }
    fn first_action(&self) -> Option<ItemAction> {
        assert!(*self.locked.borrow(), "first action read without the lock");
        self.log(Call::First);
        self.queue.borrow().first().copied()
    }
    fn apply_queue(
        &self,
        visible: &mut bool,
        ts: u64,
        sample_rate: usize,
        buf: Option<&mut Gains>,
    ) -> usize {
        assert!(*self.locked.borrow(), "queue applied without the lock");
        self.log(Call::Apply {
            visible: *visible,
            ts,
            buf: buf.is_some(),
        });
        let queue = self.queue.borrow().clone();
        let applied = audio_actions::apply(queue, *visible, ts, sample_rate as u64, buf);
        self.queue.borrow_mut().drain(..applied.consumed);
        if applied.consumed != 0 {
            *visible = applied.visible;
        }
        applied.hides
    }
    fn release_active_ref(&self) -> bool {
        assert!(!*self.locked.borrow(), "reference dropped under the lock");
        let mut refs = self.refs.borrow_mut();
        *refs -= 1;
        self.log(Call::Release(*refs));
        *refs == 0
    }
    fn remove_active_child(&self) {
        assert!(!*self.locked.borrow(), "child removed under the lock");
        self.log(Call::Remove);
    }
}

/// A tick's gains and one more, all [`UNWRITTEN`].
struct Buf([f32; FRAMES + 1]);

impl Buf {
    fn new() -> Self {
        Buf([UNWRITTEN; FRAMES + 1])
    }
    fn tick(&mut self) -> Option<&mut Gains> {
        Some((&mut self.0[..FRAMES]).try_into().unwrap())
    }
    fn span(&self, from: usize, to: usize, value: f32) {
        for (i, g) in self.0[from..to].iter().enumerate() {
            assert_eq!(*g, value, "sample {} (span {from}-{to})", from + i);
        }
    }
    fn untouched(&self) {
        self.span(0, FRAMES + 1, UNWRITTEN);
    }
    /// `fake_free()`: the C never writes past the tick.
    fn done(&self) {
        assert_eq!(self.0[FRAMES], UNWRITTEN, "past the tick");
    }
}

#[test]
fn steps_no_actions() {
    let f = Model::new(true, 1);
    let mut b = Buf::new();
    apply_scene_item_audio_actions(&f, b.tick(), TICK, 48000);
    b.span(0, FRAMES, 1.0);
    f.check(true, 1, 0, 0);
    b.done();

    let f = Model::new(false, 1);
    let mut b = Buf::new();
    apply_scene_item_audio_actions(&f, b.tick(), TICK, 48000);
    b.span(0, FRAMES, 0.0);
    f.check(false, 1, 0, 0);
    b.done();

    // no buffer: nothing to do
    let f = Model::new(true, 1);
    apply_scene_item_audio_actions(&f, None, TICK, 48000);
    f.check(true, 1, 0, 0);
}

#[test]
fn steps_one_change_on_each_frame() {
    for k in 0..FRAMES {
        let ku = k as u64;
        let f = Model::new(true, 1);
        let mut b = Buf::new();
        f.push(false, at_frame(TICK, ku, 48000));
        apply_scene_item_audio_actions(&f, b.tick(), TICK, 48000);
        b.span(0, k, 1.0);
        b.span(k, FRAMES, 0.0);
        f.check(false, 0, 1, 0);
        b.done();

        let f = Model::new(false, 1);
        let mut b = Buf::new();
        f.push(true, at_frame(TICK, ku, 48000));
        apply_scene_item_audio_actions(&f, b.tick(), TICK, 48000);
        b.span(0, k, 0.0);
        b.span(k, FRAMES, 1.0);
        f.check(true, 1, 0, 0);
        b.done();

        if k != 0 {
            let f = Model::new(true, 1);
            let mut b = Buf::new();
            f.push(false, at_frame(TICK, ku, 48000) - 1);
            apply_scene_item_audio_actions(&f, b.tick(), TICK, 48000);
            b.span(0, k - 1, 1.0);
            b.span(k - 1, FRAMES, 0.0);
            b.done();
        }
    }
}

#[test]
fn steps_one_change_at_each_rate() {
    let rates = [
        8000u64, 11025, 16000, 22050, 24000, 32000, 44100, 48000, 88200, 96000, 192000,
    ];
    let frames = [0usize, 1, 2, 511, 512, 1000, 1022, 1023];
    for rate in rates {
        for k in frames {
            let f = Model::new(true, 1);
            let mut b = Buf::new();
            f.push(false, at_frame(TICK, k as u64, rate));
            apply_scene_item_audio_actions(&f, b.tick(), TICK, rate as usize);
            b.span(0, k, 1.0);
            b.span(k, FRAMES, 0.0);
            f.check(false, 0, 1, 0);
            b.done();
        }
    }
}

#[test]
fn steps_action_past_the_tick() {
    let past = at_frame(TICK, FRAMES as u64, 48000);
    let f = Model::new(true, 1);
    let mut b = Buf::new();
    f.push(false, past);
    apply_scene_item_audio_actions(&f, b.tick(), TICK, 48000);
    b.span(0, FRAMES, 1.0);
    f.check(true, 1, 0, 1);
    assert_eq!(f.queue.borrow()[0].timestamp, past);
    b.done();

    // the last nanosecond of frame 1023 is still this tick
    let f = Model::new(true, 1);
    let mut b = Buf::new();
    f.push(false, past - 1);
    apply_scene_item_audio_actions(&f, b.tick(), TICK, 48000);
    b.span(0, FRAMES - 1, 1.0);
    b.span(FRAMES - 1, FRAMES, 0.0);
    f.check(false, 0, 1, 0);
    b.done();
}

#[test]
fn steps_late_action_on_frame_0() {
    let f = Model::new(true, 1);
    let mut b = Buf::new();
    f.push(false, TICK - 5 * NS);
    apply_scene_item_audio_actions(&f, b.tick(), TICK, 48000);
    b.span(0, FRAMES, 0.0);
    f.check(false, 0, 1, 0);
    b.done();

    // timestamp 0 too
    let f = Model::new(false, 1);
    let mut b = Buf::new();
    f.push(true, 0);
    apply_scene_item_audio_actions(&f, b.tick(), TICK, 48000);
    b.span(0, FRAMES, 1.0);
    f.check(true, 1, 0, 0);
    b.done();
}

#[test]
fn steps_several_changes_in_one_tick() {
    let f = Model::new(true, 2);
    let mut b = Buf::new();
    f.push(false, at_frame(TICK, 100, 48000));
    f.push(true, at_frame(TICK, 300, 48000));
    f.push(false, at_frame(TICK, 700, 48000));
    apply_scene_item_audio_actions(&f, b.tick(), TICK, 48000);
    b.span(0, 100, 1.0);
    b.span(100, 300, 0.0);
    b.span(300, 700, 1.0);
    b.span(700, FRAMES, 0.0);
    f.check(false, 0, 1, 0);
    b.done();

    // two changes on one frame: the span between them is empty
    let f = Model::new(true, 2);
    let mut b = Buf::new();
    f.push(false, at_frame(TICK, 400, 48000));
    f.push(true, at_frame(TICK, 400, 48000));
    apply_scene_item_audio_actions(&f, b.tick(), TICK, 48000);
    b.span(0, FRAMES, 1.0);
    f.check(true, 1, 0, 0);
    b.done();

    // on frame 0, then again later
    let f = Model::new(true, 2);
    let mut b = Buf::new();
    f.push(false, TICK);
    f.push(true, at_frame(TICK, 1, 48000));
    f.push(false, at_frame(TICK, 1023, 48000));
    apply_scene_item_audio_actions(&f, b.tick(), TICK, 48000);
    b.span(0, 1, 0.0);
    b.span(1, 1023, 1.0);
    b.span(1023, FRAMES, 0.0);
    f.check(false, 0, 1, 0);
    b.done();
}

#[test]
fn steps_later_actions_stay_queued() {
    let f = Model::new(true, 1);
    let mut b = Buf::new();
    f.push(false, at_frame(TICK, 200, 48000));
    f.push(true, at_frame(TICK, 1500, 48000));
    f.push(false, at_frame(TICK, 3000, 48000));
    apply_scene_item_audio_actions(&f, b.tick(), TICK, 48000);
    b.span(0, 200, 1.0);
    b.span(200, FRAMES, 0.0);
    f.check(false, 0, 1, 2);
    assert_eq!(
        *f.queue.borrow(),
        [
            ItemAction {
                visible: true,
                timestamp: at_frame(TICK, 1500, 48000)
            },
            ItemAction {
                visible: false,
                timestamp: at_frame(TICK, 3000, 48000)
            },
        ]
    );

    // the next tick picks them up from its own start
    let next = TICK + mul_div(FRAMES as u64, NS, 48000);
    apply_scene_item_audio_actions(&f, b.tick(), next, 48000);
    let show = mul_div(at_frame(TICK, 1500, 48000) - next, 48000, NS) as usize;
    assert_eq!(show, 1500 - FRAMES);
    b.span(0, show, 0.0);
    b.span(show, FRAMES, 1.0);
    f.check(true, 0, 1, 1);
    b.done();
}

#[test]
fn steps_actions_out_of_order() {
    // Characterized, not endorsed: a change timed before the previous one
    // does not rewrite the buffer, but its hide is counted; the loop stops
    // at the first action past the tick, even when a later one is due.
    let f = Model::new(true, 1);
    let mut b = Buf::new();
    f.push(false, at_frame(TICK, 600, 48000));
    f.push(true, at_frame(TICK, 200, 48000));
    apply_scene_item_audio_actions(&f, b.tick(), TICK, 48000);
    b.span(0, FRAMES, 1.0);
    f.check(true, 0, 1, 0);
    b.done();

    let f = Model::new(true, 1);
    let mut b = Buf::new();
    f.push(true, at_frame(TICK, 1500, 48000));
    f.push(false, at_frame(TICK, 100, 48000));
    apply_scene_item_audio_actions(&f, b.tick(), TICK, 48000);
    b.span(0, FRAMES, 1.0);
    f.check(true, 1, 0, 2);
    b.done();
}

#[test]
fn steps_hides_drop_references() {
    // hide, show, hide, show, hide, from 3 references
    let f = Model::new(true, 3);
    let mut b = Buf::new();
    for i in 0..5u64 {
        f.push(i % 2 != 0, at_frame(TICK, 100 + 100 * i, 48000));
    }
    apply_scene_item_audio_actions(&f, b.tick(), TICK, 48000);
    f.check(false, 0, 1, 0);
    b.done();

    // references to spare: no removal
    let f = Model::new(true, 5);
    let mut b = Buf::new();
    f.push(false, at_frame(TICK, 10, 48000));
    f.push(true, at_frame(TICK, 20, 48000));
    f.push(false, at_frame(TICK, 30, 48000));
    apply_scene_item_audio_actions(&f, b.tick(), TICK, 48000);
    f.check(false, 3, 0, 0);
    b.done();

    // a hide of a hidden item still counts
    let f = Model::new(false, 2);
    let mut b = Buf::new();
    f.push(false, at_frame(TICK, 10, 48000));
    apply_scene_item_audio_actions(&f, b.tick(), TICK, 48000);
    b.span(0, FRAMES, 0.0);
    f.check(false, 1, 0, 0);
    b.done();

    // Characterized, not endorsed: more hides than references go below 0
    // and remove the child only at 0.
    let f = Model::new(true, 1);
    let mut b = Buf::new();
    f.push(false, at_frame(TICK, 10, 48000));
    f.push(true, at_frame(TICK, 20, 48000));
    f.push(false, at_frame(TICK, 30, 48000));
    apply_scene_item_audio_actions(&f, b.tick(), TICK, 48000);
    f.check(false, -1, 1, 0);
    b.done();
}

#[test]
fn steps_ts_0_applies_everything() {
    let f = Model::new(true, 2);
    f.push(false, TICK);
    f.push(true, TICK + 3600 * NS);
    f.push(false, u64::MAX);
    apply_scene_item_audio_actions(&f, None, 0, 0);
    f.check(false, 0, 1, 0);

    // with a rate (process_all_audio_actions() passes one)
    let f = Model::new(false, 1);
    f.push(true, TICK);
    f.push(false, TICK + 3600 * NS);
    f.push(true, u64::MAX);
    apply_scene_item_audio_actions(&f, None, 0, 192000);
    f.check(true, 0, 1, 0);

    // a tick with rate 0 puts every action on frame 0
    let f = Model::new(true, 1);
    let mut b = Buf::new();
    f.push(false, TICK + 3600 * NS);
    apply_scene_item_audio_actions(&f, b.tick(), TICK, 0);
    b.span(0, FRAMES, 0.0);
    f.check(false, 0, 1, 0);
    b.done();
}

#[test]
fn steps_volume_without_actions() {
    let f = Model::new(true, 1);
    let mut b = Buf::new();
    assert!(!apply_scene_item_volume(&f, b.tick(), TICK, 48000));
    b.untouched();
    f.check(true, 1, 0, 0);
}

#[test]
fn steps_volume_window() {
    for rate in [8000u64, 22050, 44100, 48000, 96000, 192000] {
        let duration = FRAMES as u64 * NS / rate;

        let f = Model::new(true, 1);
        let mut b = Buf::new();
        f.push(false, TICK + duration - 1);
        assert!(apply_scene_item_volume(&f, b.tick(), TICK, rate as usize));
        b.span(0, FRAMES - 1, 1.0);
        b.span(FRAMES - 1, FRAMES, 0.0);
        f.check(false, 0, 1, 0);
        b.done();

        let f = Model::new(true, 1);
        let mut b = Buf::new();
        f.push(false, TICK + duration);
        assert!(!apply_scene_item_volume(&f, b.tick(), TICK, rate as usize));
        b.untouched();
        f.check(true, 1, 0, 1);
    }

    // Characterized, not endorsed: at 44.1 kHz an action at ts + duration
    // is on frame 1023 but waits for the next tick.
    let f = Model::new(true, 1);
    let mut b = Buf::new();
    let duration = FRAMES as u64 * NS / 44100;
    assert_eq!(
        audio_actions::frame(TICK + duration, TICK, 44100),
        FRAMES as u64 - 1
    );
    f.push(false, TICK + duration);
    assert!(!apply_scene_item_volume(&f, b.tick(), TICK, 44100));
    f.check(true, 1, 0, 1);
}

#[test]
fn steps_volume_first_action_only() {
    let f = Model::new(true, 1);
    let mut b = Buf::new();
    f.push(true, at_frame(TICK, 5000, 48000));
    f.push(false, at_frame(TICK, 10, 48000));
    assert!(!apply_scene_item_volume(&f, b.tick(), TICK, 48000));
    b.untouched();
    f.check(true, 1, 0, 2);
}

#[test]
fn steps_volume_ts_0() {
    let f = Model::new(true, 1);
    f.push(false, u64::MAX);
    assert!(apply_scene_item_volume(&f, None, 0, 48000));
    f.check(false, 0, 1, 0);
    assert!(!apply_scene_item_volume(&f, None, 0, 48000));
}

#[test]
fn steps_process_all() {
    let f = Model::new(true, 2);
    f.push(false, TICK);
    f.push(true, TICK + NS);
    f.push(false, TICK + 3600 * NS);
    process_all_audio_actions(&f, 48000);
    f.check(false, 0, 1, 0);

    let f = Model::new(false, 1);
    process_all_audio_actions(&f, 48000);
    f.check(false, 1, 0, 0);
}

#[test]
fn order_apply_unlocks_before_the_drops() {
    let f = Model::new(true, 2);
    let mut b = Buf::new();
    f.push(false, at_frame(TICK, 10, 48000));
    f.push(true, at_frame(TICK, 20, 48000));
    f.push(false, at_frame(TICK, 30, 48000));
    apply_scene_item_audio_actions(&f, b.tick(), TICK, 48000);
    assert_eq!(
        *f.calls.borrow(),
        [
            Call::Lock,
            Call::Visible(true),
            Call::Apply {
                visible: true,
                ts: TICK,
                buf: true
            },
            Call::SetVisible(false),
            Call::Unlock,
            Call::Release(1),
            Call::Release(0),
            Call::Remove,
        ]
    );
}

#[test]
fn order_visibility_written_back_without_actions() {
    // The copy is written back even when nothing was consumed.
    let f = Model::new(false, 1);
    apply_scene_item_audio_actions(&f, None, TICK, 48000);
    assert_eq!(
        *f.calls.borrow(),
        [
            Call::Lock,
            Call::Visible(false),
            Call::Apply {
                visible: false,
                ts: TICK,
                buf: false
            },
            Call::SetVisible(false),
            Call::Unlock,
        ]
    );
}

#[test]
fn order_every_drop_checks_for_zero() {
    // Below 0 there is no removal; each drop is its own decrement.
    let f = Model::new(true, 0);
    f.push(false, TICK);
    f.push(false, TICK);
    apply_scene_item_audio_actions(&f, None, TICK, 48000);
    let calls = f.calls.borrow();
    assert_eq!(
        calls[calls.len() - 2..],
        [Call::Release(-1), Call::Release(-2)]
    );
    assert_eq!(f.removals(), 0);
}

#[test]
fn order_volume_peeks_under_the_lock() {
    // Not due: one lock round for the peek, nothing else.
    let f = Model::new(true, 1);
    f.push(false, TICK + NS);
    assert!(!apply_scene_item_volume(&f, None, TICK, 48000));
    assert_eq!(*f.calls.borrow(), [Call::Lock, Call::First, Call::Unlock]);

    // Empty: the same.
    let f = Model::new(true, 1);
    assert!(!apply_scene_item_volume(&f, None, TICK, 48000));
    assert_eq!(*f.calls.borrow(), [Call::Lock, Call::First, Call::Unlock]);

    // Due: the peek's lock is released before the apply takes it again.
    let f = Model::new(true, 2);
    f.push(false, TICK);
    assert!(apply_scene_item_volume(&f, None, TICK, 48000));
    assert_eq!(
        *f.calls.borrow(),
        [
            Call::Lock,
            Call::First,
            Call::Unlock,
            Call::Lock,
            Call::Visible(true),
            Call::Apply {
                visible: true,
                ts: TICK,
                buf: false
            },
            Call::SetVisible(false),
            Call::Unlock,
            Call::Release(1),
        ]
    );
}

#[test]
fn order_process_all_stops_on_an_empty_queue() {
    let f = Model::new(true, 3);
    f.push(false, u64::MAX);
    f.push(true, 0);
    process_all_audio_actions(&f, 48000);
    let calls = f.calls.borrow();
    // One volume round applies everything, the next finds the queue empty.
    assert_eq!(calls.iter().filter(|c| **c == Call::First).count(), 2);
    assert_eq!(
        calls
            .iter()
            .filter(|c| matches!(
                c,
                Call::Apply {
                    ts: 0,
                    buf: false,
                    ..
                }
            ))
            .count(),
        1
    );
    drop(calls);
    f.check(true, 2, 0, 0);
}

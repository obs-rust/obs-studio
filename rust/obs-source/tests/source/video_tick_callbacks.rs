//! Tier 1: `obs_source::video_tick_callbacks` over a recording model of the
//! source, its type's callbacks, its deferred count and its signals,
//! mirroring every `test_source_video_tick_callbacks` case one step at a
//! time: the callback before the signals, the signals without a callback
//! or data, the deferred update with and without `update`, and an update
//! deferred again from inside the callback surviving the swap.
use core::ffi::{CStr, c_long};
use obs_source::video_tick_callbacks::{Host, Step, deferred_update, run_step};
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Event {
    CanUpdate,
    LoadDeferCount(c_long),
    CallUpdate,
    SwapDeferCount(c_long),
    HasCallback(Step),
    Call(Step),
    Signal(&'static CStr, &'static CStr),
}

#[derive(Default)]
struct State {
    /// `context.data` is set.
    data: bool,
    /// The type's `update`, `activate`, `deactivate`, `show`, `hide`.
    update: bool,
    callbacks: [bool; 4],
    defer_update_count: c_long,
    /// Added to the count from inside `update` (an update deferred again).
    redefer: c_long,
    events: Vec<Event>,
}

struct Model(RefCell<State>);

const fn index(step: Step) -> usize {
    match step {
        Step::Activate => 0,
        Step::Deactivate => 1,
        Step::Show => 2,
        Step::Hide => 3,
    }
}

impl Model {
    fn new(data: bool, update: bool, callbacks: bool, count: c_long) -> Self {
        Model(RefCell::new(State {
            data,
            update,
            callbacks: [callbacks; 4],
            defer_update_count: count,
            ..State::default()
        }))
    }
    fn events(&self) -> Vec<Event> {
        std::mem::take(&mut self.0.borrow_mut().events)
    }
    fn push(&self, event: Event) {
        self.0.borrow_mut().events.push(event);
    }
    fn count(&self) -> c_long {
        self.0.borrow().defer_update_count
    }
}

impl Host for Model {
    type Source = ();
    fn can_update(&self, (): ()) -> bool {
        self.push(Event::CanUpdate);
        let s = self.0.borrow();
        s.data && s.update
    }
    fn load_defer_count(&self, (): ()) -> c_long {
        let count = self.count();
        self.push(Event::LoadDeferCount(count));
        count
    }
    fn call_update(&self, (): ()) {
        assert!(self.0.borrow().data && self.0.borrow().update);
        self.push(Event::CallUpdate);
        let mut s = self.0.borrow_mut();
        s.defer_update_count = s.defer_update_count.wrapping_add(s.redefer);
    }
    fn swap_defer_count(&self, (): (), expected: c_long) {
        self.push(Event::SwapDeferCount(expected));
        let mut s = self.0.borrow_mut();
        if s.defer_update_count == expected {
            s.defer_update_count = 0;
        }
    }
    fn has_callback(&self, (): (), step: Step) -> bool {
        self.push(Event::HasCallback(step));
        let s = self.0.borrow();
        s.data && s.callbacks[index(step)]
    }
    fn call(&self, (): (), step: Step) {
        assert!(self.0.borrow().data && self.0.borrow().callbacks[index(step)]);
        self.push(Event::Call(step));
    }
    fn signal(&self, (): (), global: &'static CStr, local: &'static CStr) {
        self.push(Event::Signal(global, local));
    }
}

const STEPS: [(Step, &CStr, &CStr); 4] = [
    (Step::Activate, c"source_activate", c"activate"),
    (Step::Deactivate, c"source_deactivate", c"deactivate"),
    (Step::Show, c"source_show", c"show"),
    (Step::Hide, c"source_hide", c"hide"),
];

#[test]
fn each_step_calls_its_callback_then_sends_its_signals() {
    for (step, global, local) in STEPS {
        let model = Model::new(true, false, true, 0);
        run_step(&model, (), step);
        assert_eq!(
            model.events(),
            [
                Event::HasCallback(step),
                Event::Call(step),
                Event::Signal(global, local)
            ]
        );
        assert_eq!(step.signals(), (global, local));
    }
}

#[test]
fn each_step_signals_without_a_callback() {
    for (step, global, local) in STEPS {
        let model = Model::new(true, true, false, 0);
        run_step(&model, (), step);
        assert_eq!(
            model.events(),
            [Event::HasCallback(step), Event::Signal(global, local)]
        );
    }
}

#[test]
fn each_step_signals_without_data() {
    for (step, global, local) in STEPS {
        let model = Model::new(false, true, true, 0);
        run_step(&model, (), step);
        assert_eq!(
            model.events(),
            [Event::HasCallback(step), Event::Signal(global, local)]
        );
    }
}

#[test]
fn steps_leave_the_deferred_count_alone() {
    for (step, _, _) in STEPS {
        let model = Model::new(true, true, true, 3);
        run_step(&model, (), step);
        assert_eq!(model.count(), 3);
    }
}

#[test]
fn deferred_update_runs_swaps_then_signals() {
    let model = Model::new(true, true, true, 1);
    deferred_update(&model, ());
    assert_eq!(
        model.events(),
        [
            Event::CanUpdate,
            Event::LoadDeferCount(1),
            Event::CallUpdate,
            Event::SwapDeferCount(1),
            Event::Signal(c"source_update", c"update"),
        ]
    );
    assert_eq!(model.count(), 0);
}

#[test]
fn deferred_update_without_update_or_data_does_nothing() {
    for (data, update) in [(true, false), (false, true), (false, false)] {
        let model = Model::new(data, update, true, 2);
        deferred_update(&model, ());
        assert_eq!(model.events(), [Event::CanUpdate]);
        assert_eq!(model.count(), 2);
    }
}

#[test]
fn update_deferred_again_survives_the_swap() {
    let model = Model::new(true, true, false, 1);
    model.0.borrow_mut().redefer = 1;
    deferred_update(&model, ());
    assert_eq!(
        model.events(),
        [
            Event::CanUpdate,
            Event::LoadDeferCount(1),
            Event::CallUpdate,
            Event::SwapDeferCount(1),
            Event::Signal(c"source_update", c"update"),
        ]
    );
    assert_eq!(model.count(), 2);

    // The next tick runs it again and clears it.
    model.0.borrow_mut().redefer = 0;
    deferred_update(&model, ());
    assert_eq!(model.events()[1], Event::LoadDeferCount(2));
    assert_eq!(model.count(), 0);
}

#[test]
fn swaps_against_the_count_loaded_before_the_callback() {
    for count in [c_long::MIN, -1, 0, 1, 7, c_long::MAX] {
        for redefer in [-1, 0, 1] {
            let model = Model::new(true, true, false, count);
            model.0.borrow_mut().redefer = redefer;
            deferred_update(&model, ());
            assert_eq!(model.events()[3], Event::SwapDeferCount(count));
            let expected = if redefer == 0 {
                0
            } else {
                count.wrapping_add(redefer)
            };
            assert_eq!(model.count(), expected, "{count} {redefer}");
        }
    }
}

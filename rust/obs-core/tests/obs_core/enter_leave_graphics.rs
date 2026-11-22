//! Safe mirrors of test_enter_leave_graphics.c over a host whose graphics
//! layer behaves as libobs-graphics does: `gs_enter_context` takes the
//! context's mutex once per thread and counts nested entries,
//! `gs_leave_context` drops one count and releases the context at zero, and
//! reports a leave from a thread outside any context. Plus what the C test
//! cannot see: each call reads the core's graphics once, enter forwards that
//! same graphics, and with no graphics neither call reaches the layer.
use obs_core::enter_leave_graphics::{Graphics, enter_graphics, leave_graphics};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, ThreadId};
use std::time::Duration;

/// The core's graphics context, as one id.
const CORE: usize = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Event {
    ReadGraphics,
    Enter(usize),
    Leave,
}

/// The graphics layer: one context (`CORE`), its mutex and entry count, and
/// the diagnostics it logs.
#[derive(Default)]
struct Layer {
    state: Mutex<LayerState>,
    released: Condvar,
}

#[derive(Default)]
struct LayerState {
    owner: Option<ThreadId>,
    depth: usize,
    enter_diagnostics: usize,
    leave_diagnostics: usize,
}

thread_local! {
    /// `thread_graphics`: the context the calling thread is in.
    static CURRENT: Cell<Option<usize>> = const { Cell::new(None) };
}

fn current() -> Option<usize> {
    CURRENT.with(Cell::get)
}

impl Layer {
    fn enter(&self, graphics: usize) {
        let me = thread::current().id();
        let mut state = self.state.lock().unwrap();
        if current() != Some(graphics) {
            while state.owner.is_some() {
                state = self.released.wait(state).unwrap();
            }
            state.owner = Some(me);
            CURRENT.with(|c| c.set(Some(graphics)));
        }
        state.depth += 1;
    }

    fn leave(&self) {
        let mut state = self.state.lock().unwrap();
        if current().is_none() {
            state.leave_diagnostics += 1;
            return;
        }
        state.depth -= 1;
        if state.depth == 0 {
            state.owner = None;
            CURRENT.with(|c| c.set(None));
            self.released.notify_all();
        }
    }

    fn diagnostics(&self) -> (usize, usize) {
        let state = self.state.lock().unwrap();
        (state.enter_diagnostics, state.leave_diagnostics)
    }
}

/// The core as the two calls see it: its graphics field and the layer.
struct Host {
    graphics: Option<usize>,
    layer: Arc<Layer>,
    events: Vec<Event>,
}

impl Host {
    fn new(graphics: Option<usize>, layer: &Arc<Layer>) -> Self {
        Host {
            graphics,
            layer: Arc::clone(layer),
            events: Vec::new(),
        }
    }

    fn enter(&mut self) {
        enter_graphics(self);
    }

    fn leave(&mut self) {
        leave_graphics(self);
    }
}

impl Graphics for Host {
    type Context = usize;
    fn graphics(&mut self) -> Option<usize> {
        self.events.push(Event::ReadGraphics);
        self.graphics
    }
    fn enter_context(&mut self, graphics: usize) {
        self.events.push(Event::Enter(graphics));
        self.layer.enter(graphics);
    }
    fn leave_context(&mut self) {
        self.events.push(Event::Leave);
        self.layer.leave();
    }
}

#[test]
fn no_graphics_before_video_reset() {
    let layer = Arc::new(Layer::default());
    let mut host = Host::new(None, &layer);
    assert_eq!(current(), None);
    for _ in 0..4 {
        host.enter();
        assert_eq!(current(), None);
    }
    for _ in 0..4 {
        host.leave();
        assert_eq!(current(), None);
    }
    // Neither reaches the graphics layer: no gs_* diagnostic, and each call
    // only read the core's graphics.
    assert_eq!(layer.diagnostics(), (0, 0));
    assert_eq!(host.events, vec![Event::ReadGraphics; 8]);
}

#[test]
fn enter_makes_core_context_current() {
    let layer = Arc::new(Layer::default());
    let mut host = Host::new(Some(CORE), &layer);
    assert_eq!(current(), None);

    host.enter();
    assert_eq!(current(), Some(CORE));
    host.leave();
    assert_eq!(current(), None);
    assert_eq!(layer.diagnostics(), (0, 0));

    // Again, so leaving really released the context for a new entry.
    host.enter();
    assert_eq!(current(), Some(CORE));
    host.leave();
    assert_eq!(current(), None);

    assert_eq!(
        host.events,
        [
            Event::ReadGraphics,
            Event::Enter(CORE),
            Event::ReadGraphics,
            Event::Leave
        ]
        .repeat(2)
    );
}

#[test]
fn enter_is_counted() {
    let layer = Arc::new(Layer::default());
    let mut host = Host::new(Some(CORE), &layer);

    host.enter();
    host.enter();
    host.enter();
    assert_eq!(current(), Some(CORE));
    host.leave();
    assert_eq!(current(), Some(CORE));
    host.leave();
    assert_eq!(current(), Some(CORE));
    host.leave();
    assert_eq!(current(), None);

    // With a graphics subsystem, leaving while outside is forwarded: the
    // layer reports it and nothing changes.
    host.events.clear();
    host.leave();
    assert_eq!(current(), None);
    assert_eq!(layer.diagnostics(), (0, 1));
    assert_eq!(host.events, [Event::ReadGraphics, Event::Leave]);

    // And it did not unbalance the next entry.
    host.enter();
    assert_eq!(current(), Some(CORE));
    host.leave();
    assert_eq!(current(), None);
}

#[test]
fn other_thread_waits_for_leave() {
    let layer = Arc::new(Layer::default());
    let mut host = Host::new(Some(CORE), &layer);
    let entered = Arc::new(AtomicBool::new(false));

    host.enter();
    assert_eq!(current(), Some(CORE));
    let other = {
        let layer = Arc::clone(&layer);
        let entered = Arc::clone(&entered);
        thread::spawn(move || {
            let mut host = Host::new(Some(CORE), &layer);
            host.enter();
            let seen = current();
            entered.store(true, Ordering::SeqCst);
            host.leave();
            (seen, current())
        })
    };

    // While this thread is inside, the other one cannot enter.
    thread::sleep(Duration::from_millis(100));
    assert!(!entered.load(Ordering::SeqCst));

    // A nested entry here does not let it in either.
    host.enter();
    host.leave();
    thread::sleep(Duration::from_millis(30));
    assert!(!entered.load(Ordering::SeqCst));

    host.leave();
    assert_eq!(current(), None);

    // It got the same core context, and left it again.
    assert_eq!(other.join().unwrap(), (Some(CORE), None));
    assert!(entered.load(Ordering::SeqCst));
    assert_eq!(current(), None);
}

#[test]
fn each_call_reads_the_graphics_once_and_enters_that_one() {
    let layer = Arc::new(Layer::default());
    let mut host = Host::new(Some(CORE), &layer);
    host.enter();
    assert_eq!(host.events, [Event::ReadGraphics, Event::Enter(CORE)]);
    host.events.clear();
    host.leave();
    assert_eq!(host.events, [Event::ReadGraphics, Event::Leave]);
}

#[test]
fn graphics_gone_between_enter_and_leave_skips_the_leave() {
    // The read is per call: a leave after the graphics went away does not
    // reach the layer, as in the C.
    let layer = Arc::new(Layer::default());
    let mut host = Host::new(Some(CORE), &layer);
    host.enter();
    host.graphics = None;
    host.events.clear();
    host.leave();
    assert_eq!(host.events, [Event::ReadGraphics]);
    assert_eq!(current(), Some(CORE));
    host.graphics = Some(CORE);
    host.leave();
    assert_eq!(current(), None);
}

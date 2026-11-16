//! Tier 1: assertions from test/cmocka/test_canvas_release.c over safe fixtures.
use obs_core::{canvas_ref, canvas_release, object_ref::Host};
use obs_util::platform::{CLong, CLongAtomic};
use std::sync::atomic::Ordering::SeqCst;

#[derive(Debug, PartialEq)]
enum Event {
    Warn,
    Control,
    Destroy { refs: i128, weak: i128 },
    WeakRelease { freed: bool },
}

struct Fixture {
    initialized: bool,
    refs: CLongAtomic,
    weak: CLongAtomic,
    events: Vec<Event>,
}

impl Fixture {
    fn new(refs: CLong, weak: CLong) -> Self {
        Self {
            initialized: true,
            refs: CLongAtomic::new(refs),
            weak: CLongAtomic::new(weak),
            events: Vec::new(),
        }
    }
    fn counts(&self) -> (CLong, CLong) {
        (self.refs.load(SeqCst), self.weak.load(SeqCst))
    }
}

impl Host for Fixture {
    type Control = ();
    fn initialized(&self) -> bool {
        self.initialized
    }
    fn warn_shutdown(&mut self) {
        self.events.push(Event::Warn);
    }
    fn control(&self) -> Option<()> {
        // Record through interior state: the trait reads control by &self.
        CONTROL_READS.with(|reads| reads.set(reads.get() + 1));
        Some(())
    }
    fn strong(&self, (): ()) -> &CLongAtomic {
        &self.refs
    }
    fn destroy(&mut self) {
        let (refs, weak) = self.counts();
        self.events.push(Event::Destroy {
            refs: refs.into(),
            weak: weak.into(),
        });
    }
    fn release_weak(&mut self, (): ()) {
        let freed = canvas_ref::release(Some(&self.weak));
        self.events.push(Event::WeakRelease { freed });
    }
}

thread_local! {
    static CONTROL_READS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn release(present: bool, fixture: &mut Fixture) -> usize {
    CONTROL_READS.with(|reads| reads.set(0));
    canvas_release::release(present, fixture);
    let reads = CONTROL_READS.with(std::cell::Cell::get);
    if reads > 0 {
        fixture.events.insert(0, Event::Control);
    }
    reads
}

#[test]
fn shutdown_core_guards_before_dereferencing() {
    let mut f = Fixture::new(0, 0);
    f.initialized = false;
    // NULL stays silent after shutdown, unlike obs_object_release.
    assert_eq!(release(false, &mut f), 0);
    assert!(f.events.is_empty());
    // A non-NULL handle only warns: no control read, no count change.
    assert_eq!(release(true, &mut f), 0);
    assert_eq!(f.events, [Event::Warn]);
    assert_eq!(f.counts(), (0, 0));
}

#[test]
fn null_with_live_core_is_silent() {
    let mut f = Fixture::new(0, 0);
    assert_eq!(release(false, &mut f), 0);
    assert!(f.events.is_empty());
    assert_eq!(f.counts(), (0, 0));
}

#[test]
fn nonfinal_and_final_release_order() {
    // Two extra strong owners and one extra weak owner, as in the C test.
    let mut f = Fixture::new(2, 1);
    for remaining in [1, 0] {
        assert_eq!(release(true, &mut f), 1);
        assert_eq!(f.counts(), (remaining, 1));
        assert_eq!(f.events, [Event::Control]);
        f.events.clear();
    }
    assert_eq!(release(true, &mut f), 1);
    // Destroy runs after the final decrement while the canvas's own weak
    // reference is still held; the weak release follows destroy.
    assert_eq!(
        f.events,
        [
            Event::Control,
            Event::Destroy { refs: -1, weak: 1 },
            Event::WeakRelease { freed: false },
        ]
    );
    assert_eq!(f.counts(), (-1, 0));
}

#[test]
fn final_release_frees_a_lone_weak_control() {
    let mut f = Fixture::new(0, 0);
    assert_eq!(release(true, &mut f), 1);
    assert_eq!(
        f.events,
        [
            Event::Control,
            Event::Destroy { refs: -1, weak: 0 },
            Event::WeakRelease { freed: true },
        ]
    );
    assert_eq!(f.counts(), (-1, -1));
}

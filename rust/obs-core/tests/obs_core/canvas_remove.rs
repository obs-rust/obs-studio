//! Tier 1 mirrors test_canvas_remove_lifecycle and remove_rules unchanged.
use obs_core::canvas_remove::{Host, remove};
use obs_core::canvas_rules::{ACTIVATE, MAIN, PROGRAM, REMOVED, SCENE_REF};

#[derive(Debug, PartialEq, Eq)]
enum Event {
    Read(usize),
    Retain(usize),
    Write(usize, u32),
    Signal(usize, u32, i32, bool),
    Release(usize),
}

struct Canvas {
    flags: u32,
    refs: i32,
    weak: i32,
    private: bool,
}
struct Fixture {
    canvases: [Canvas; 2],
    promoted: usize,
    events: Vec<Event>,
}
impl Fixture {
    fn new(flags: u32, private: bool, refs: i32) -> Self {
        Self {
            canvases: [
                Canvas {
                    flags,
                    refs,
                    weak: 3,
                    private,
                },
                Canvas {
                    flags: SCENE_REF,
                    refs: 4,
                    weak: 5,
                    private: true,
                },
            ],
            promoted: 0,
            events: Vec::new(),
        }
    }
}
impl Host for Fixture {
    type Canvas = usize;
    fn flags(&mut self, canvas: usize) -> u32 {
        self.events.push(Event::Read(canvas));
        self.canvases[canvas].flags
    }
    fn retain(&mut self, canvas: usize) -> Option<usize> {
        self.events.push(Event::Retain(canvas));
        let target = &mut self.canvases[self.promoted];
        if target.refs < 0 {
            return None;
        }
        target.refs += 1;
        Some(self.promoted)
    }
    fn set_flags(&mut self, canvas: usize, flags: u32) {
        self.events.push(Event::Write(canvas, flags));
        self.canvases[canvas].flags = flags;
    }
    fn signal(&mut self, canvas: usize) {
        let c = &self.canvases[canvas];
        if !c.private {
            self.events
                .push(Event::Signal(canvas, c.flags, c.refs, true));
        }
        self.events
            .push(Event::Signal(canvas, c.flags, c.refs, false));
    }
    fn release(&mut self, canvas: usize) {
        self.events.push(Event::Release(canvas));
        self.canvases[canvas].refs -= 1;
    }
}

#[test]
fn test_canvas_remove_lifecycle_public_canvas_order() {
    let mut f = Fixture::new(PROGRAM, false, 2);
    remove(&mut f, 0);
    assert_eq!(
        f.events,
        [
            Event::Read(0),
            Event::Retain(0),
            Event::Read(0),
            Event::Write(0, PROGRAM | REMOVED),
            Event::Signal(0, PROGRAM | REMOVED, 3, true),
            Event::Signal(0, PROGRAM | REMOVED, 3, false),
            Event::Release(0)
        ]
    );
    assert_eq!(f.canvases[0].refs, 2);
    assert_eq!(f.canvases[0].weak, 3);
    assert_eq!(f.canvases[0].flags, PROGRAM | REMOVED);
    assert!(obs_core::canvas_rules::removed(f.canvases[0].flags));
}

#[test]
fn test_canvas_remove_lifecycle_private_canvas_local_only() {
    let mut f = Fixture::new(SCENE_REF, true, 2);
    remove(&mut f, 0);
    assert_eq!(
        f.events,
        [
            Event::Read(0),
            Event::Retain(0),
            Event::Read(0),
            Event::Write(0, SCENE_REF | REMOVED),
            Event::Signal(0, SCENE_REF | REMOVED, 3, false),
            Event::Release(0)
        ]
    );
    assert_eq!(f.canvases[0].refs, 2);
    assert_eq!(f.canvases[0].flags, SCENE_REF | REMOVED);
}

#[test]
fn test_canvas_remove_lifecycle_main_and_removed_do_nothing() {
    for flags in [MAIN, MAIN | ACTIVATE, ACTIVATE | REMOVED] {
        let mut f = Fixture::new(flags, false, 2);
        remove(&mut f, 0);
        assert_eq!(f.events, [Event::Read(0)]);
        assert_eq!(f.canvases[0].flags, flags);
        assert_eq!(f.canvases[0].refs, 2);
    }
    let mut f = Fixture::new(ACTIVATE, false, 2);
    remove(&mut f, 0);
    assert_eq!(f.canvases[0].flags, ACTIVATE | REMOVED);
    f.events.clear();
    remove(&mut f, 0);
    assert_eq!(f.events, [Event::Read(0)]);
    assert_eq!(f.canvases[0].refs, 2);
}

#[test]
fn test_canvas_remove_lifecycle_failed_retain_does_nothing() {
    let mut f = Fixture::new(PROGRAM, false, -1);
    remove(&mut f, 0);
    assert_eq!(f.events, [Event::Read(0), Event::Retain(0)]);
    assert_eq!(f.canvases[0].flags, PROGRAM);
    assert_eq!(f.canvases[0].refs, -1);
    assert_eq!(f.canvases[0].weak, 3);
}

#[test]
fn reads_and_removes_the_promoted_canvas() {
    let mut f = Fixture::new(PROGRAM, false, 2);
    f.promoted = 1;
    remove(&mut f, 0);
    assert_eq!(
        f.events,
        [
            Event::Read(0),
            Event::Retain(0),
            Event::Read(1),
            Event::Write(1, SCENE_REF | REMOVED),
            Event::Signal(1, SCENE_REF | REMOVED, 5, false),
            Event::Release(1)
        ]
    );
    assert_eq!(f.canvases[0].flags, PROGRAM);
    assert_eq!(f.canvases[0].refs, 2);
    assert_eq!(f.canvases[1].refs, 4);
}

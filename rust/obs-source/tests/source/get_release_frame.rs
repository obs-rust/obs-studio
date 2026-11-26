//! Tier 1: `obs_source::get_release_frame` over a recording model of the
//! source and its frames, mirroring every `test_source_get_release_frame`
//! case plus the order of every step and the lock's scope.
use obs_source::get_release_frame::{Host, get_frame, release_frame};
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Event {
    NullSource(&'static core::ffi::CStr),
    Lock,
    Unlock,
    Take,
    Addref(usize),
    ReleaseRef(usize),
    Destroy(usize),
    Return(usize),
}

#[derive(Default)]
struct Frame {
    refs: i64,
    prev_frame: bool,
    destroyed: bool,
}

#[derive(Default)]
struct State {
    locked: bool,
    current: Option<usize>,
    frames: Vec<Frame>,
    /// (frame, used), in cache order.
    cache: Vec<(usize, bool)>,
    events: Vec<Event>,
}

struct Model(RefCell<State>);

impl Model {
    fn new(frames: &[(i64, bool)]) -> Self {
        Model(RefCell::new(State {
            frames: frames
                .iter()
                .map(|&(refs, prev_frame)| Frame {
                    refs,
                    prev_frame,
                    destroyed: false,
                })
                .collect(),
            ..State::default()
        }))
    }
    fn events(&self) -> Vec<Event> {
        std::mem::take(&mut self.0.borrow_mut().events)
    }
}

impl Host for Model {
    type Source = ();
    type Frame = usize;
    fn null_source(&self, func: &'static core::ffi::CStr) {
        self.0.borrow_mut().events.push(Event::NullSource(func));
    }
    fn lock(&self, (): ()) {
        let mut s = self.0.borrow_mut();
        assert!(!s.locked, "locks once");
        s.locked = true;
        s.events.push(Event::Lock);
    }
    fn unlock(&self, (): ()) {
        let mut s = self.0.borrow_mut();
        assert!(s.locked);
        s.locked = false;
        s.events.push(Event::Unlock);
    }
    fn take_current(&self, (): ()) -> Option<usize> {
        let mut s = self.0.borrow_mut();
        assert!(s.locked, "cur_async_frame is read under async_mutex");
        s.events.push(Event::Take);
        s.current.take()
    }
    fn addref(&self, frame: usize) {
        let mut s = self.0.borrow_mut();
        assert!(s.locked);
        s.events.push(Event::Addref(frame));
        s.frames[frame].refs += 1;
    }
    fn release_ref(&self, frame: usize) -> bool {
        let mut s = self.0.borrow_mut();
        assert!(s.locked);
        s.events.push(Event::ReleaseRef(frame));
        s.frames[frame].refs -= 1;
        s.frames[frame].refs == 0
    }
    fn destroy(&self, frame: usize) {
        let mut s = self.0.borrow_mut();
        s.events.push(Event::Destroy(frame));
        assert!(!s.frames[frame].destroyed);
        s.frames[frame].destroyed = true;
    }
    fn return_to_cache(&self, (): (), frame: usize) {
        let mut s = self.0.borrow_mut();
        assert!(s.locked);
        s.events.push(Event::Return(frame));
        s.frames[frame].prev_frame = false;
        if let Some(slot) = s.cache.iter_mut().find(|(f, _)| *f == frame) {
            slot.1 = false;
        }
    }
}

use Event::*;

#[test]
fn get_null_source() {
    let m = Model::new(&[(1, true)]);
    m.0.borrow_mut().current = Some(0);
    assert_eq!(get_frame(&m, None), None);
    assert_eq!(m.events(), [NullSource(c"obs_source_get_frame")]);
    assert_eq!(m.0.borrow().current, Some(0));
}

#[test]
fn get_without_frame() {
    let m = Model::new(&[]);
    assert_eq!(get_frame(&m, Some(())), None);
    assert_eq!(m.events(), [Lock, Take, Unlock]);
    assert!(!m.0.borrow().locked);
}

#[test]
fn get_takes_current_with_a_reference() {
    let m = Model::new(&[(1, true), (1, false)]);
    m.0.borrow_mut().current = Some(0);
    m.0.borrow_mut().cache = vec![(0, true)];
    assert_eq!(get_frame(&m, Some(())), Some(0));
    assert_eq!(m.events(), [Lock, Take, Addref(0), Unlock]);
    let s = m.0.borrow();
    assert_eq!(s.current, None);
    assert_eq!(s.frames[0].refs, 2);
    assert!(s.frames[0].prev_frame);
    assert_eq!(s.cache, [(0, true)]);
    drop(s);
    // Taken once: the slot is empty now.
    assert_eq!(get_frame(&m, Some(())), None);
    assert_eq!(m.events(), [Lock, Take, Unlock]);
    assert_eq!(m.0.borrow().frames[0].refs, 2);
}

#[test]
fn release_null_frame() {
    let m = Model::new(&[(2, true)]);
    release_frame(&m, Some(()), None);
    release_frame(&m, None, None);
    assert_eq!(m.events(), []);
    assert_eq!(m.0.borrow().frames[0].refs, 2);
}

#[test]
fn release_null_source_destroys() {
    let m = Model::new(&[(5, true)]);
    release_frame(&m, None, Some(0));
    // Whatever its references, and without the lock.
    assert_eq!(m.events(), [Destroy(0)]);
    assert_eq!(m.0.borrow().frames[0].refs, 5);
}

#[test]
fn release_last_reference_destroys() {
    let m = Model::new(&[(1, true)]);
    m.0.borrow_mut().cache = vec![(0, true)];
    release_frame(&m, Some(()), Some(0));
    assert_eq!(m.events(), [Lock, ReleaseRef(0), Destroy(0), Unlock]);
    // The cache is left alone (characterized, not endorsed).
    assert_eq!(m.0.borrow().cache, [(0, true)]);
}

#[test]
fn release_returns_to_cache() {
    let m = Model::new(&[(1, true), (3, true)]);
    m.0.borrow_mut().cache = vec![(0, true), (1, true), (1, true)];
    release_frame(&m, Some(()), Some(1));
    assert_eq!(m.events(), [Lock, ReleaseRef(1), Return(1), Unlock]);
    {
        let s = m.0.borrow();
        assert_eq!(s.frames[1].refs, 2);
        assert!(!s.frames[1].prev_frame);
        assert!(s.frames[0].prev_frame);
        assert_eq!(s.cache, [(0, true), (1, false), (1, true)]);
    }
    release_frame(&m, Some(()), Some(1));
    assert_eq!(m.events(), [Lock, ReleaseRef(1), Return(1), Unlock]);
    assert_eq!(m.0.borrow().frames[1].refs, 1);
}

#[test]
fn release_uncached() {
    let m = Model::new(&[(1, true), (2, true)]);
    m.0.borrow_mut().cache = vec![(0, true)];
    release_frame(&m, Some(()), Some(1));
    assert_eq!(m.events(), [Lock, ReleaseRef(1), Return(1), Unlock]);
    let s = m.0.borrow();
    assert_eq!(s.frames[1].refs, 1);
    assert!(!s.frames[1].prev_frame);
    assert_eq!(s.cache, [(0, true)]);
}

#[test]
fn release_unreferenced() {
    // Zero goes below zero and is not destroyed (characterized, not
    // endorsed).
    let m = Model::new(&[(0, true)]);
    m.0.borrow_mut().cache = vec![(0, true)];
    release_frame(&m, Some(()), Some(0));
    assert_eq!(m.events(), [Lock, ReleaseRef(0), Return(0), Unlock]);
    let s = m.0.borrow();
    assert_eq!(s.frames[0].refs, -1);
    assert!(!s.frames[0].destroyed);
    assert_eq!(s.cache, [(0, false)]);
}

#[test]
fn get_then_release() {
    let m = Model::new(&[(1, false)]);
    m.0.borrow_mut().current = Some(0);
    m.0.borrow_mut().cache = vec![(0, true)];
    let got = get_frame(&m, Some(()));
    release_frame(&m, Some(()), got);
    assert_eq!(
        m.events(),
        [
            Lock,
            Take,
            Addref(0),
            Unlock,
            Lock,
            ReleaseRef(0),
            Return(0),
            Unlock
        ]
    );
    let s = m.0.borrow();
    assert_eq!(s.frames[0].refs, 1);
    assert_eq!(s.cache, [(0, false)]);
    assert_eq!(s.current, None);
}

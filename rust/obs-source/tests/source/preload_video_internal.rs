//! Tier 1: `obs_source::preload_video_internal` over a recording model of the
//! source and its preload frame, mirroring every
//! `test_source_preload_video_internal` case plus the worker's own NULL
//! source and destroying checks (which the public entry never reaches), the
//! order of every step and the scope of `async_mutex`.
use core::ffi::CStr;
use obs_source::frame_copy::Shape;
use obs_source::preload_video_internal::{Host, preload_video_internal};
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Event {
    NullSource(&'static CStr),
    Destroying,
    PreloadShape,
    FrameShape,
    /// The preload frame destroyed (its id, 0 for none).
    DestroyPreload(u32),
    /// A preload frame created (its new id) at the frame's shape.
    CreatePreload(u32, Shape),
    /// The frame (its timestamp) copied into the preload frame (its id).
    CopyToPreload(u32, u64),
    LockAsync,
    SetLastFrameTs(u64),
    UnlockAsync,
}

/// A frame the caller hands over: its shape and timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Frame {
    shape: Shape,
    timestamp: u64,
}

#[derive(Default)]
struct State {
    destroying: bool,
    /// The preload frame: its id and shape.
    preload: Option<(u32, Shape)>,
    next_id: u32,
    last_frame_ts: u64,
    async_locked: bool,
    events: Vec<Event>,
}

struct Model(RefCell<State>);

impl Model {
    fn new(destroying: bool, preload: Option<Shape>) -> Self {
        Model(RefCell::new(State {
            destroying,
            preload: preload.map(|shape| (1, shape)),
            next_id: 2,
            last_frame_ts: 7,
            ..State::default()
        }))
    }
    fn events(&self) -> Vec<Event> {
        std::mem::take(&mut self.0.borrow_mut().events)
    }
    fn push(&self, event: Event) {
        self.0.borrow_mut().events.push(event);
    }
    fn unlocked(&self) -> bool {
        !self.0.borrow().async_locked
    }
    fn preload(&self) -> Option<(u32, Shape)> {
        self.0.borrow().preload
    }
    fn last_frame_ts(&self) -> u64 {
        self.0.borrow().last_frame_ts
    }
}

impl Host for Model {
    type Source = ();
    type Frame = Frame;
    fn null_source(&self, func: &'static CStr) {
        assert!(self.unlocked());
        self.push(Event::NullSource(func));
    }
    fn destroying(&self, (): ()) -> bool {
        assert!(self.unlocked());
        self.push(Event::Destroying);
        self.0.borrow().destroying
    }
    fn preload_shape(&self, (): ()) -> Option<Shape> {
        assert!(self.unlocked());
        self.push(Event::PreloadShape);
        self.preload().map(|(_, shape)| shape)
    }
    fn frame_shape(&self, frame: Frame) -> Shape {
        assert!(self.unlocked());
        self.push(Event::FrameShape);
        frame.shape
    }
    fn destroy_preload(&self, (): ()) {
        assert!(self.unlocked());
        let id = self.preload().map_or(0, |(id, _)| id);
        self.push(Event::DestroyPreload(id));
        // The field still names the freed frame until the create.
    }
    fn create_preload(&self, (): (), frame: Frame) {
        assert!(self.unlocked());
        let mut s = self.0.borrow_mut();
        let id = s.next_id;
        s.next_id += 1;
        s.preload = Some((id, frame.shape));
        s.events.push(Event::CreatePreload(id, frame.shape));
    }
    fn copy_to_preload(&self, (): (), frame: Frame) {
        assert!(self.unlocked(), "the copy runs outside async_mutex");
        let id = self.preload().map_or(0, |(id, _)| id);
        self.push(Event::CopyToPreload(id, frame.timestamp));
    }
    fn lock_async(&self, (): ()) {
        assert!(self.unlocked());
        self.0.borrow_mut().async_locked = true;
        self.push(Event::LockAsync);
    }
    fn set_last_frame_ts(&self, (): (), frame: Frame) {
        assert!(
            self.0.borrow().async_locked,
            "last_frame_ts under async_mutex"
        );
        self.0.borrow_mut().last_frame_ts = frame.timestamp;
        self.push(Event::SetLastFrameTs(frame.timestamp));
    }
    fn unlock_async(&self, (): ()) {
        assert!(self.0.borrow().async_locked);
        self.0.borrow_mut().async_locked = false;
        self.push(Event::UnlockAsync);
    }
}

use obs_source::frame_copy::video_format::{BGRA, I420};

const fn shape(format: i32, width: u32, height: u32) -> Shape {
    Shape {
        format,
        width,
        height,
    }
}

const fn frame(shape: Shape, timestamp: u64) -> Frame {
    Frame { shape, timestamp }
}

const BGRA_2X2: Shape = shape(BGRA, 2, 2);

/// The events of a call that makes a new preload frame (id `new`) after
/// destroying `old` (0 for none).
fn recreated(old: u32, new: u32, f: Frame) -> Vec<Event> {
    vec![
        Event::Destroying,
        Event::PreloadShape,
        Event::FrameShape,
        Event::DestroyPreload(old),
        Event::CreatePreload(new, f.shape),
        Event::CopyToPreload(new, f.timestamp),
        Event::LockAsync,
        Event::SetLastFrameTs(f.timestamp),
        Event::UnlockAsync,
    ]
}

/// The events of a call that reuses preload frame `id`.
fn reused(id: u32, f: Frame) -> Vec<Event> {
    vec![
        Event::Destroying,
        Event::PreloadShape,
        Event::FrameShape,
        Event::CopyToPreload(id, f.timestamp),
        Event::LockAsync,
        Event::SetLastFrameTs(f.timestamp),
        Event::UnlockAsync,
    ]
}

#[test]
fn null_source_logs_the_public_entry() {
    let model = Model::new(false, Some(BGRA_2X2));
    preload_video_internal(&model, None, Some(frame(BGRA_2X2, 100)));
    assert_eq!(
        model.events(),
        [Event::NullSource(c"obs_source_preload_video")]
    );
    assert_eq!(model.preload(), Some((1, BGRA_2X2)));
    assert_eq!(model.last_frame_ts(), 7);

    // A NULL frame too: the source is checked first.
    preload_video_internal(&model, None, None);
    assert_eq!(
        model.events(),
        [Event::NullSource(c"obs_source_preload_video")]
    );
}

#[test]
fn destroying_source_is_left_alone() {
    for preload in [None, Some(BGRA_2X2)] {
        for f in [None, Some(frame(BGRA_2X2, 100))] {
            let model = Model::new(true, preload);
            preload_video_internal(&model, Some(()), f);
            assert_eq!(model.events(), [Event::Destroying]);
            assert_eq!(model.preload(), preload.map(|s| (1, s)));
            assert_eq!(model.last_frame_ts(), 7);
        }
    }
}

/// `null_frame_without_preload`, `null_frame_keeps_preload`.
#[test]
fn null_frame_changes_nothing() {
    for preload in [None, Some(BGRA_2X2)] {
        let model = Model::new(false, preload);
        preload_video_internal(&model, Some(()), None);
        assert_eq!(model.events(), [Event::Destroying]);
        assert_eq!(model.preload(), preload.map(|s| (1, s)));
        assert_eq!(model.last_frame_ts(), 7);
    }
}

/// `first_frame_creates_preload`: the NULL preload frame is still passed to
/// the (NULL-safe) destroy.
#[test]
fn first_frame_creates_preload() {
    let model = Model::new(false, None);
    let f = frame(BGRA_2X2, 100);
    preload_video_internal(&model, Some(()), Some(f));
    assert_eq!(model.events(), recreated(0, 2, f));
    assert_eq!(model.preload(), Some((2, BGRA_2X2)));
    assert_eq!(model.last_frame_ts(), 100);
    assert!(model.unlocked());
}

/// `same_shape_reuses_preload`.
#[test]
fn same_shape_reuses_preload() {
    let model = Model::new(false, Some(BGRA_2X2));
    let f = frame(BGRA_2X2, 200);
    preload_video_internal(&model, Some(()), Some(f));
    assert_eq!(model.events(), reused(1, f));
    assert_eq!(model.preload(), Some((1, BGRA_2X2)));
    assert_eq!(model.last_frame_ts(), 200);
}

/// `changed_shape_recreates_preload`: a changed format, width or height
/// each makes a new preload frame; the timestamp plays no part.
#[test]
fn changed_shape_recreates_preload() {
    for changed in [
        shape(I420, 2, 2),
        shape(BGRA, 4, 2),
        shape(BGRA, 2, 4),
        shape(-1, 2, 2),
        shape(BGRA, 0, 0),
        shape(BGRA, u32::MAX, 2),
    ] {
        let model = Model::new(false, Some(BGRA_2X2));
        let f = frame(changed, 7);
        preload_video_internal(&model, Some(()), Some(f));
        assert_eq!(model.events(), recreated(1, 2, f), "{changed:?}");
        assert_eq!(model.preload(), Some((2, changed)));
        assert_eq!(model.last_frame_ts(), 7);
    }
}

/// The cmocka case sequence on one source: create, reuse, then each change.
#[test]
fn sequence_tracks_the_preload_frame() {
    let model = Model::new(false, None);
    let steps = [
        (frame(BGRA_2X2, 100), true),
        (frame(BGRA_2X2, 150), false),
        (frame(shape(I420, 2, 2), 200), true),
        (frame(BGRA_2X2, 250), true),
        (frame(shape(BGRA, 4, 2), 300), true),
        (frame(BGRA_2X2, 350), true),
        (frame(shape(BGRA, 2, 4), 400), true),
        (frame(shape(BGRA, 2, 4), u64::MAX), false),
        (frame(shape(BGRA, 2, 4), 0), false),
    ];
    let mut id = 0;
    for (f, new) in steps {
        preload_video_internal(&model, Some(()), Some(f));
        if new {
            assert_eq!(model.events(), recreated(id, id.max(1) + 1, f));
            id = id.max(1) + 1;
        } else {
            assert_eq!(model.events(), reused(id, f));
        }
        assert_eq!(model.preload(), Some((id, f.shape)));
        assert_eq!(model.last_frame_ts(), f.timestamp);
        assert!(model.unlocked());
    }
    // A NULL frame after all that keeps the last one.
    preload_video_internal(&model, Some(()), None);
    assert_eq!(model.events(), [Event::Destroying]);
    assert_eq!(model.preload(), Some((id, shape(BGRA, 2, 4))));
    assert_eq!(model.last_frame_ts(), 0);
}

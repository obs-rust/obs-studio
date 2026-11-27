//! Tier 1: `obs_source::set_video_frame_internal` over a recording model of
//! the source, its preload frame, its async textures and the graphics
//! context, mirroring every `test_source_set_video_frame_internal` case
//! plus the worker's own NULL source and NULL frame checks (which the public
//! entry never reaches), the order of every step, the scope of the graphics
//! context and of `async_mutex`.
use core::ffi::CStr;
use obs_source::frame_copy::Shape;
use obs_source::set_video_frame_internal::{Host, set_video_frame_internal};
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Event {
    NullSource(&'static CStr),
    EnterGraphics,
    PreloadShape,
    FrameShape,
    /// The preload frame destroyed (its id, 0 for none).
    DestroyPreload(u32),
    /// A preload frame created (its new id) at the frame's shape.
    CreatePreload(u32, Shape),
    /// The frame (its timestamp) copied into the preload frame (its id).
    CopyToPreload(u32, u64),
    /// The async textures sized to the preload frame (its id).
    SetTextureSize(u32),
    /// The async textures filled from the preload frame (its id).
    UpdateTextures(u32),
    MarkTexturesFresh,
    LockAsync,
    SetLastFrameTs(u64),
    UnlockAsync,
    LeaveGraphics,
}

/// A frame the caller hands over: its shape and timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Frame {
    shape: Shape,
    timestamp: u64,
}

#[derive(Default)]
struct State {
    /// The preload frame: its id and shape.
    preload: Option<(u32, Shape)>,
    /// The preload frame's id and the timestamp last copied into it.
    copied: Option<(u32, u64)>,
    /// The shape the async textures were last sized to.
    texture_shape: Option<Shape>,
    textures_stale: bool,
    next_id: u32,
    last_frame_ts: u64,
    in_graphics: bool,
    async_locked: bool,
    events: Vec<Event>,
}

struct Model(RefCell<State>);

impl Model {
    fn new(preload: Option<Shape>) -> Self {
        Model(RefCell::new(State {
            preload: preload.map(|shape| (1, shape)),
            textures_stale: true,
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
    fn in_graphics(&self) -> bool {
        self.0.borrow().in_graphics
    }
    fn unlocked(&self) -> bool {
        !self.0.borrow().async_locked
    }
    /// Inside the graphics context, outside `async_mutex`.
    fn graphics_step(&self) {
        assert!(self.in_graphics(), "runs inside the graphics context");
        assert!(self.unlocked(), "runs outside async_mutex");
    }
    fn preload(&self) -> Option<(u32, Shape)> {
        self.0.borrow().preload
    }
    fn preload_id(&self) -> u32 {
        self.preload().map_or(0, |(id, _)| id)
    }
    fn last_frame_ts(&self) -> u64 {
        self.0.borrow().last_frame_ts
    }
    fn texture_shape(&self) -> Option<Shape> {
        self.0.borrow().texture_shape
    }
    fn textures_stale(&self) -> bool {
        self.0.borrow().textures_stale
    }
    fn settled(&self) -> bool {
        !self.in_graphics() && self.unlocked()
    }
}

impl Host for Model {
    type Source = ();
    type Frame = Frame;
    fn null_source(&self, func: &'static CStr) {
        assert!(self.settled());
        self.push(Event::NullSource(func));
    }
    fn enter_graphics(&self) {
        assert!(self.settled());
        self.0.borrow_mut().in_graphics = true;
        self.push(Event::EnterGraphics);
    }
    fn preload_shape(&self, (): ()) -> Option<Shape> {
        self.graphics_step();
        self.push(Event::PreloadShape);
        self.preload().map(|(_, shape)| shape)
    }
    fn frame_shape(&self, frame: Frame) -> Shape {
        self.graphics_step();
        self.push(Event::FrameShape);
        frame.shape
    }
    fn destroy_preload(&self, (): ()) {
        self.graphics_step();
        let id = self.preload_id();
        self.push(Event::DestroyPreload(id));
        // The field still names the freed frame until the create.
    }
    fn create_preload(&self, (): (), frame: Frame) {
        self.graphics_step();
        let mut s = self.0.borrow_mut();
        let id = s.next_id;
        s.next_id += 1;
        s.preload = Some((id, frame.shape));
        s.events.push(Event::CreatePreload(id, frame.shape));
    }
    fn copy_to_preload(&self, (): (), frame: Frame) {
        self.graphics_step();
        let id = self.preload_id();
        self.0.borrow_mut().copied = Some((id, frame.timestamp));
        self.push(Event::CopyToPreload(id, frame.timestamp));
    }
    fn set_texture_size(&self, (): ()) {
        self.graphics_step();
        let id = self.preload_id();
        assert!(
            self.0.borrow().copied.is_some_and(|(c, _)| c == id),
            "sized from the preload frame after the copy"
        );
        let shape = self.preload().map(|(_, shape)| shape);
        self.0.borrow_mut().texture_shape = shape;
        self.push(Event::SetTextureSize(id));
    }
    fn update_textures(&self, (): ()) {
        self.graphics_step();
        assert_eq!(
            self.texture_shape(),
            self.preload().map(|(_, shape)| shape),
            "filled after the size"
        );
        let id = self.preload_id();
        self.push(Event::UpdateTextures(id));
    }
    fn mark_textures_fresh(&self, (): ()) {
        self.graphics_step();
        self.0.borrow_mut().textures_stale = false;
        self.push(Event::MarkTexturesFresh);
    }
    fn lock_async(&self, (): ()) {
        self.graphics_step();
        self.0.borrow_mut().async_locked = true;
        self.push(Event::LockAsync);
    }
    fn set_last_frame_ts(&self, (): (), frame: Frame) {
        assert!(self.in_graphics());
        assert!(
            self.0.borrow().async_locked,
            "last_frame_ts under async_mutex"
        );
        self.0.borrow_mut().last_frame_ts = frame.timestamp;
        self.push(Event::SetLastFrameTs(frame.timestamp));
    }
    fn unlock_async(&self, (): ()) {
        assert!(self.in_graphics());
        assert!(self.0.borrow().async_locked);
        self.0.borrow_mut().async_locked = false;
        self.push(Event::UnlockAsync);
    }
    fn leave_graphics(&self) {
        self.graphics_step();
        self.0.borrow_mut().in_graphics = false;
        self.push(Event::LeaveGraphics);
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

/// The events after the preload frame (id `id`) is settled.
fn tail(id: u32, f: Frame) -> [Event; 8] {
    [
        Event::CopyToPreload(id, f.timestamp),
        Event::SetTextureSize(id),
        Event::UpdateTextures(id),
        Event::MarkTexturesFresh,
        Event::LockAsync,
        Event::SetLastFrameTs(f.timestamp),
        Event::UnlockAsync,
        Event::LeaveGraphics,
    ]
}

/// The events of a call that makes a new preload frame (id `new`) after
/// destroying `old` (0 for none).
fn recreated(old: u32, new: u32, f: Frame) -> Vec<Event> {
    let mut events = vec![
        Event::EnterGraphics,
        Event::PreloadShape,
        Event::FrameShape,
        Event::DestroyPreload(old),
        Event::CreatePreload(new, f.shape),
    ];
    events.extend(tail(new, f));
    events
}

/// The events of a call that reuses preload frame `id`.
fn reused(id: u32, f: Frame) -> Vec<Event> {
    let mut events = vec![Event::EnterGraphics, Event::PreloadShape, Event::FrameShape];
    events.extend(tail(id, f));
    events
}

#[test]
fn null_source_logs_the_public_entry() {
    let model = Model::new(Some(BGRA_2X2));
    set_video_frame_internal(&model, None, Some(frame(BGRA_2X2, 100)));
    assert_eq!(
        model.events(),
        [Event::NullSource(c"obs_source_set_video_frame")]
    );
    assert_eq!(model.preload(), Some((1, BGRA_2X2)));
    assert_eq!(model.last_frame_ts(), 7);
    assert!(model.textures_stale());

    // A NULL frame too: the source is checked first.
    set_video_frame_internal(&model, None, None);
    assert_eq!(
        model.events(),
        [Event::NullSource(c"obs_source_set_video_frame")]
    );
}

/// A NULL frame returns before the graphics context is entered.
#[test]
fn null_frame_changes_nothing() {
    for preload in [None, Some(BGRA_2X2)] {
        let model = Model::new(preload);
        set_video_frame_internal(&model, Some(()), None);
        assert_eq!(model.events(), []);
        assert_eq!(model.preload(), preload.map(|s| (1, s)));
        assert_eq!(model.last_frame_ts(), 7);
        assert!(model.textures_stale());
        assert_eq!(model.texture_shape(), None);
    }
}

/// `first_frame_creates_preload`: the NULL preload frame is still passed to
/// the (NULL-safe) destroy; the textures follow the new preload frame.
#[test]
fn first_frame_creates_preload() {
    let model = Model::new(None);
    let f = frame(BGRA_2X2, 100);
    set_video_frame_internal(&model, Some(()), Some(f));
    assert_eq!(model.events(), recreated(0, 2, f));
    assert_eq!(model.preload(), Some((2, BGRA_2X2)));
    assert_eq!(model.last_frame_ts(), 100);
    assert_eq!(model.texture_shape(), Some(BGRA_2X2));
    assert!(!model.textures_stale());
    assert!(model.settled());
}

/// `same_shape_reuses_preload`.
#[test]
fn same_shape_reuses_preload() {
    let model = Model::new(Some(BGRA_2X2));
    let f = frame(BGRA_2X2, 200);
    set_video_frame_internal(&model, Some(()), Some(f));
    assert_eq!(model.events(), reused(1, f));
    assert_eq!(model.preload(), Some((1, BGRA_2X2)));
    assert_eq!(model.last_frame_ts(), 200);
    assert_eq!(model.texture_shape(), Some(BGRA_2X2));
    assert!(!model.textures_stale());
    assert!(model.settled());
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
        let model = Model::new(Some(BGRA_2X2));
        let f = frame(changed, 7);
        set_video_frame_internal(&model, Some(()), Some(f));
        assert_eq!(model.events(), recreated(1, 2, f), "{changed:?}");
        assert_eq!(model.preload(), Some((2, changed)));
        assert_eq!(model.texture_shape(), Some(changed));
        assert_eq!(model.last_frame_ts(), 7);
        assert!(model.settled());
    }
}

/// The cmocka case sequence on one source: create, reuse, then each change,
/// with the textures marked stale again between calls.
#[test]
fn sequence_tracks_the_preload_frame() {
    let model = Model::new(None);
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
        model.0.borrow_mut().textures_stale = true;
        set_video_frame_internal(&model, Some(()), Some(f));
        if new {
            assert_eq!(model.events(), recreated(id, id.max(1) + 1, f));
            id = id.max(1) + 1;
        } else {
            assert_eq!(model.events(), reused(id, f));
        }
        assert_eq!(model.preload(), Some((id, f.shape)));
        assert_eq!(model.texture_shape(), Some(f.shape));
        assert_eq!(model.last_frame_ts(), f.timestamp);
        assert!(!model.textures_stale());
        assert!(model.settled());
    }
    // A NULL frame after all that keeps the last one and touches nothing.
    model.0.borrow_mut().textures_stale = true;
    set_video_frame_internal(&model, Some(()), None);
    assert_eq!(model.events(), []);
    assert_eq!(model.preload(), Some((id, shape(BGRA, 2, 4))));
    assert_eq!(model.last_frame_ts(), 0);
    assert!(model.textures_stale());
}

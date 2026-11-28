//! Tier 1: `obs_source::output_video_internal` over a recording model of the
//! source, its async frame cache and queue and its two mutexes, mirroring
//! every `test_source_output_video_internal` case plus the worker's own NULL
//! source check and the refs-reach-0 branch (which the public entry never
//! reaches), the order of every step and the scope of each mutex.
use core::ffi::{CStr, c_long};
use obs_source::output_video_internal::{Host, output_video_internal};
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Event {
    NullSource(&'static CStr),
    LockAudioBuf,
    FrameTimestamp,
    SetLastOutputFrameTs(u64),
    UnlockAudioBuf,
    LockAsync,
    SetAsyncActive(bool),
    MarkTexturesStale,
    ClearLastFrameTs,
    FreeAsyncCache,
    UnlockAsync,
    FrameReceived,
    /// The frame (its timestamp) handed to the cache.
    CacheVideo(u64),
    /// A cached frame (its id) lost a reference.
    ReleaseRef(u32),
    DestroyFrame(u32),
    PushFrame(u32),
}

/// What the next `cache_video()` gives: `None` for an overflow, or the
/// references the cached frame holds with the one `cache_video()` added.
type CacheAnswer = Option<c_long>;

#[derive(Default)]
struct State {
    /// The cached frames: id and references.
    cache: Vec<(u32, c_long)>,
    /// The queued frames' ids.
    queue: Vec<u32>,
    answers: Vec<CacheAnswer>,
    next_id: u32,
    last_output_frame_ts: u64,
    last_frame_ts: u64,
    async_active: bool,
    textures_stale: bool,
    audio_buf_locked: bool,
    async_locked: bool,
    events: Vec<Event>,
}

struct Model(RefCell<State>);

impl Model {
    fn new() -> Self {
        Model(RefCell::new(State {
            next_id: 1,
            last_output_frame_ts: 9,
            last_frame_ts: 7,
            ..State::default()
        }))
    }
    fn answer(&self, answer: CacheAnswer) {
        self.0.borrow_mut().answers.push(answer);
    }
    fn events(&self) -> Vec<Event> {
        std::mem::take(&mut self.0.borrow_mut().events)
    }
    fn push(&self, event: Event) {
        self.0.borrow_mut().events.push(event);
    }
    fn audio_locked(&self) -> bool {
        self.0.borrow().audio_buf_locked
    }
    fn async_locked(&self) -> bool {
        self.0.borrow().async_locked
    }
    fn settled(&self) -> bool {
        !self.audio_locked() && !self.async_locked()
    }
    /// Under `async_mutex` alone.
    fn async_step(&self) {
        assert!(self.async_locked(), "runs under async_mutex");
        assert!(!self.audio_locked(), "runs outside audio_buf_mutex");
    }
    fn queue(&self) -> Vec<u32> {
        self.0.borrow().queue.clone()
    }
    fn last_output_frame_ts(&self) -> u64 {
        self.0.borrow().last_output_frame_ts
    }
    fn last_frame_ts(&self) -> u64 {
        self.0.borrow().last_frame_ts
    }
    fn async_active(&self) -> bool {
        self.0.borrow().async_active
    }
    fn textures_stale(&self) -> bool {
        self.0.borrow().textures_stale
    }
    fn refs(&self, id: u32) -> Option<c_long> {
        self.0
            .borrow()
            .cache
            .iter()
            .find(|&&(i, _)| i == id)
            .map(|&(_, refs)| refs)
    }
}

impl Host for Model {
    type Source = ();
    type Frame = u64;
    type Cached = u32;
    fn null_source(&self, func: &'static CStr) {
        assert!(self.settled());
        self.push(Event::NullSource(func));
    }
    fn lock_audio_buf(&self, (): ()) {
        assert!(self.settled(), "audio_buf_mutex is taken alone (#3747)");
        self.0.borrow_mut().audio_buf_locked = true;
        self.push(Event::LockAudioBuf);
    }
    fn frame_timestamp(&self, frame: u64) -> u64 {
        self.push(Event::FrameTimestamp);
        frame
    }
    fn set_last_output_frame_ts(&self, (): (), timestamp: u64) {
        assert!(
            self.audio_locked(),
            "last_output_frame_ts under audio_buf_mutex"
        );
        assert!(!self.async_locked());
        self.0.borrow_mut().last_output_frame_ts = timestamp;
        self.push(Event::SetLastOutputFrameTs(timestamp));
    }
    fn unlock_audio_buf(&self, (): ()) {
        assert!(self.audio_locked());
        self.0.borrow_mut().audio_buf_locked = false;
        self.push(Event::UnlockAudioBuf);
    }
    fn lock_async(&self, (): ()) {
        assert!(self.settled());
        self.0.borrow_mut().async_locked = true;
        self.push(Event::LockAsync);
    }
    fn set_async_active(&self, (): (), active: bool) {
        self.async_step();
        self.0.borrow_mut().async_active = active;
        self.push(Event::SetAsyncActive(active));
    }
    fn mark_textures_stale(&self, (): ()) {
        self.async_step();
        self.0.borrow_mut().textures_stale = true;
        self.push(Event::MarkTexturesStale);
    }
    fn clear_last_frame_ts(&self, (): ()) {
        self.async_step();
        self.0.borrow_mut().last_frame_ts = 0;
        self.push(Event::ClearLastFrameTs);
    }
    fn free_async_cache(&self, (): ()) {
        self.async_step();
        let mut s = self.0.borrow_mut();
        s.cache.clear();
        s.queue.clear();
        s.events.push(Event::FreeAsyncCache);
    }
    fn unlock_async(&self, (): ()) {
        assert!(self.async_locked());
        self.0.borrow_mut().async_locked = false;
        self.push(Event::UnlockAsync);
    }
    fn frame_received(&self, (): ()) {
        assert!(self.settled());
        self.push(Event::FrameReceived);
    }
    fn cache_video(&self, (): (), frame: u64) -> Option<u32> {
        // cache_video() takes async_mutex itself.
        assert!(self.settled(), "cache_video() runs outside both mutexes");
        let mut s = self.0.borrow_mut();
        s.events.push(Event::CacheVideo(frame));
        let answer = if s.answers.is_empty() {
            Some(2)
        } else {
            s.answers.remove(0)
        };
        let Some(refs) = answer else {
            s.cache.clear();
            s.queue.clear();
            return None;
        };
        let id = s.next_id;
        s.next_id += 1;
        s.cache.push((id, refs));
        Some(id)
    }
    fn release_ref(&self, cached: u32) -> c_long {
        self.async_step();
        self.push(Event::ReleaseRef(cached));
        let mut s = self.0.borrow_mut();
        let entry = s
            .cache
            .iter_mut()
            .find(|(id, _)| *id == cached)
            .expect("a cached frame");
        entry.1 -= 1;
        entry.1
    }
    fn destroy_frame(&self, cached: u32) {
        self.async_step();
        assert_eq!(self.refs(cached), Some(0), "destroyed at 0 only");
        let mut s = self.0.borrow_mut();
        s.cache.retain(|&(id, _)| id != cached);
        s.events.push(Event::DestroyFrame(cached));
    }
    fn push_frame(&self, (): (), cached: u32) {
        self.async_step();
        let mut s = self.0.borrow_mut();
        s.queue.push(cached);
        s.events.push(Event::PushFrame(cached));
    }
}

fn timestamp_step(ts: Option<u64>) -> Vec<Event> {
    let mut events = vec![Event::LockAudioBuf];
    if ts.is_some() {
        events.push(Event::FrameTimestamp);
    }
    events.push(Event::SetLastOutputFrameTs(ts.unwrap_or(0)));
    events.push(Event::UnlockAudioBuf);
    events
}

/// The events of a frame (timestamp `ts`) that the cache gives as `id`,
/// queued.
fn queued(ts: u64, id: u32) -> Vec<Event> {
    let mut events = timestamp_step(Some(ts));
    events.extend([
        Event::FrameReceived,
        Event::CacheVideo(ts),
        Event::LockAsync,
        Event::ReleaseRef(id),
        Event::PushFrame(id),
        Event::SetAsyncActive(true),
        Event::UnlockAsync,
    ]);
    events
}

fn null_frame() -> Vec<Event> {
    let mut events = timestamp_step(None);
    events.extend([
        Event::LockAsync,
        Event::SetAsyncActive(false),
        Event::MarkTexturesStale,
        Event::ClearLastFrameTs,
        Event::FreeAsyncCache,
        Event::UnlockAsync,
    ]);
    events
}

#[test]
fn null_source_logs_the_public_entry() {
    let model = Model::new();
    for frame in [Some(100), None] {
        output_video_internal(&model, None, frame);
        assert_eq!(
            model.events(),
            [Event::NullSource(c"obs_source_output_video")]
        );
        assert_eq!(model.last_output_frame_ts(), 9);
        assert_eq!(model.last_frame_ts(), 7);
        assert!(!model.async_active());
        assert!(!model.textures_stale());
    }
}

/// `frame_is_queued`: the cache's reference is dropped and the frame queued;
/// `last_frame_ts` and the stale flag stay.
#[test]
fn frame_is_queued() {
    let model = Model::new();
    output_video_internal(&model, Some(()), Some(100));
    assert_eq!(model.events(), queued(100, 1));
    assert_eq!(model.queue(), [1]);
    assert_eq!(model.refs(1), Some(1));
    assert_eq!(model.last_output_frame_ts(), 100);
    assert_eq!(model.last_frame_ts(), 7);
    assert!(model.async_active());
    assert!(!model.textures_stale());
    assert!(model.settled());
}

/// `null_frame_clears_the_queue` and `null_frame_on_an_idle_source`.
#[test]
fn null_frame_clears_the_queue() {
    let model = Model::new();
    output_video_internal(&model, Some(()), Some(100));
    output_video_internal(&model, Some(()), Some(200));
    model.events();
    output_video_internal(&model, Some(()), None);
    assert_eq!(model.events(), null_frame());
    assert!(model.queue().is_empty());
    assert_eq!(model.last_output_frame_ts(), 0);
    assert_eq!(model.last_frame_ts(), 0);
    assert!(!model.async_active());
    assert!(model.textures_stale());
    assert!(model.settled());

    let idle = Model::new();
    output_video_internal(&idle, Some(()), None);
    assert_eq!(idle.events(), null_frame());
    assert_eq!(idle.last_output_frame_ts(), 0);
}

/// `overflow_drops_the_queue`: no cached frame touches nothing under
/// `async_mutex` but the lock itself.
#[test]
fn overflow_touches_nothing_more() {
    let model = Model::new();
    output_video_internal(&model, Some(()), Some(100));
    model.events();
    model.answer(None);
    output_video_internal(&model, Some(()), Some(310));
    let mut events = timestamp_step(Some(310));
    events.extend([
        Event::FrameReceived,
        Event::CacheVideo(310),
        Event::LockAsync,
        Event::UnlockAsync,
    ]);
    assert_eq!(model.events(), events);
    assert!(model.queue().is_empty());
    assert_eq!(model.last_output_frame_ts(), 310);
    assert_eq!(model.last_frame_ts(), 7);
    assert!(model.async_active(), "async_active stays set");
    assert!(model.settled());
}

/// A cached frame whose last reference is the one `cache_video()` added
/// (the cache freed meanwhile) is destroyed, not queued, and `async_active`
/// is left alone.
#[test]
fn last_reference_destroys_the_frame() {
    let model = Model::new();
    model.answer(Some(1));
    output_video_internal(&model, Some(()), Some(100));
    let mut events = timestamp_step(Some(100));
    events.extend([
        Event::FrameReceived,
        Event::CacheVideo(100),
        Event::LockAsync,
        Event::ReleaseRef(1),
        Event::DestroyFrame(1),
        Event::UnlockAsync,
    ]);
    assert_eq!(model.events(), events);
    assert!(model.queue().is_empty());
    assert_eq!(model.refs(1), None);
    assert!(!model.async_active());
    assert!(model.settled());
}

/// Any count other than 0 left queues the frame, even one below 0.
#[test]
fn any_other_count_queues() {
    for refs in [2, 3, 0, -5, c_long::MAX, c_long::MIN + 1] {
        let model = Model::new();
        model.answer(Some(refs));
        output_video_internal(&model, Some(()), Some(5));
        assert_eq!(model.events(), queued(5, 1), "{refs}");
        assert_eq!(model.queue(), [1]);
        assert_eq!(model.refs(1), Some(refs - 1));
        assert!(model.async_active());
    }
}

/// The timestamp is passed through whole, 0 and the maximum included.
#[test]
fn timestamp_is_passed_whole() {
    for ts in [0, 1, u64::MAX] {
        let model = Model::new();
        output_video_internal(&model, Some(()), Some(ts));
        assert_eq!(model.events(), queued(ts, 1));
        assert_eq!(model.last_output_frame_ts(), ts);
    }
}

/// The cmocka case sequence on one source.
#[test]
fn sequence_tracks_the_queue() {
    let model = Model::new();
    output_video_internal(&model, Some(()), Some(100));
    output_video_internal(&model, Some(()), Some(200));
    assert_eq!(model.queue(), [1, 2]);
    model.answer(None);
    output_video_internal(&model, Some(()), Some(300));
    assert!(model.queue().is_empty());
    output_video_internal(&model, Some(()), Some(400));
    assert_eq!(model.queue(), [3]);
    output_video_internal(&model, Some(()), None);
    assert!(model.queue().is_empty());
    assert!(!model.async_active());
    output_video_internal(&model, Some(()), Some(500));
    assert_eq!(model.queue(), [4]);
    assert!(model.async_active());
    assert_eq!(model.last_output_frame_ts(), 500);
    assert_eq!(model.last_frame_ts(), 0);
    assert!(model.textures_stale());
    assert!(model.settled());
}

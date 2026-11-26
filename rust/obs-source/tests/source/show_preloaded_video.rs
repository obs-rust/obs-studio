//! Tier 1: `obs_source::show_preloaded_video` over a recording model of the
//! source, mirroring every `test_source_show_preloaded_video` case plus the
//! order of every step, the graphics context's scope and each mutex's.
use obs_source::show_preloaded_video::{Host, show_preloaded_video};
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Event {
    NullSource(&'static core::ffi::CStr),
    Destroying,
    HasPreload,
    EnterGraphics,
    SetTextureSize,
    UpdateTextures,
    MarkFresh,
    MarkActive,
    LeaveGraphics,
    LockAsync,
    LastFrameTs,
    UnlockAsync,
    LockAudio,
    MonitorOnly,
    Now,
    ResetTiming(u64, u64),
    ResetData(u64),
    UnlockAudio,
}

#[derive(Default)]
struct State {
    destroying: bool,
    preload: bool,
    monitor_only: bool,
    last_frame_ts: u64,
    now: u64,
    in_graphics: bool,
    async_locked: bool,
    audio_locked: bool,
    textures_stale: bool,
    active: bool,
    events: Vec<Event>,
}

struct Model(RefCell<State>);

impl Model {
    fn new(destroying: bool, preload: bool, monitor_only: bool) -> Self {
        Model(RefCell::new(State {
            destroying,
            preload,
            monitor_only,
            last_frame_ts: 5_000_000,
            now: 9_000_000,
            textures_stale: true,
            ..State::default()
        }))
    }
    fn events(&self) -> Vec<Event> {
        std::mem::take(&mut self.0.borrow_mut().events)
    }
    fn push(&self, event: Event) {
        self.0.borrow_mut().events.push(event);
    }
    /// No graphics context and no mutex held.
    fn idle(&self) -> bool {
        let s = self.0.borrow();
        !s.in_graphics && !s.async_locked && !s.audio_locked
    }
    /// Only the graphics context held.
    fn graphics_only(&self) -> bool {
        let s = self.0.borrow();
        s.in_graphics && !s.async_locked && !s.audio_locked
    }
    /// Only the audio buffer mutex held.
    fn audio_only(&self) -> bool {
        let s = self.0.borrow();
        !s.in_graphics && !s.async_locked && s.audio_locked
    }
}

impl Host for Model {
    type Source = ();
    fn null_source(&self, func: &'static core::ffi::CStr) {
        assert!(self.idle());
        self.push(Event::NullSource(func));
    }
    fn destroying(&self, (): ()) -> bool {
        assert!(self.idle());
        self.push(Event::Destroying);
        self.0.borrow().destroying
    }
    fn has_preload_frame(&self, (): ()) -> bool {
        assert!(self.idle());
        self.push(Event::HasPreload);
        self.0.borrow().preload
    }
    fn enter_graphics(&self) {
        assert!(self.idle(), "enters graphics once, holding no mutex");
        self.0.borrow_mut().in_graphics = true;
        self.push(Event::EnterGraphics);
    }
    fn set_texture_size(&self, (): ()) {
        assert!(self.graphics_only());
        self.push(Event::SetTextureSize);
    }
    fn update_textures(&self, (): ()) {
        assert!(self.graphics_only());
        self.push(Event::UpdateTextures);
    }
    fn mark_textures_fresh(&self, (): ()) {
        assert!(self.graphics_only());
        self.0.borrow_mut().textures_stale = false;
        self.push(Event::MarkFresh);
    }
    fn mark_active(&self, (): ()) {
        assert!(self.graphics_only());
        self.0.borrow_mut().active = true;
        self.push(Event::MarkActive);
    }
    fn leave_graphics(&self) {
        assert!(self.graphics_only());
        self.0.borrow_mut().in_graphics = false;
        self.push(Event::LeaveGraphics);
    }
    fn lock_async(&self, (): ()) {
        assert!(self.idle());
        self.0.borrow_mut().async_locked = true;
        self.push(Event::LockAsync);
    }
    fn last_frame_ts(&self, (): ()) -> u64 {
        let s = self.0.borrow();
        assert!(s.async_locked && !s.audio_locked && !s.in_graphics);
        drop(s);
        self.push(Event::LastFrameTs);
        self.0.borrow().last_frame_ts
    }
    fn unlock_async(&self, (): ()) {
        assert!(self.0.borrow().async_locked);
        self.0.borrow_mut().async_locked = false;
        self.push(Event::UnlockAsync);
    }
    fn lock_audio(&self, (): ()) {
        assert!(self.idle());
        self.0.borrow_mut().audio_locked = true;
        self.push(Event::LockAudio);
    }
    fn monitor_only(&self, (): ()) -> bool {
        assert!(self.audio_only());
        self.push(Event::MonitorOnly);
        self.0.borrow().monitor_only
    }
    fn now(&self) -> u64 {
        assert!(self.audio_only());
        self.push(Event::Now);
        self.0.borrow().now
    }
    fn reset_audio_timing(&self, (): (), timestamp: u64, os_time: u64) {
        assert!(self.audio_only());
        self.push(Event::ResetTiming(timestamp, os_time));
    }
    fn reset_audio_data(&self, (): (), os_time: u64) {
        assert!(self.audio_only());
        self.push(Event::ResetData(os_time));
    }
    fn unlock_audio(&self, (): ()) {
        assert!(self.audio_only());
        self.0.borrow_mut().audio_locked = false;
        self.push(Event::UnlockAudio);
    }
}

/// `test_source_show_preloaded_video::null_source`.
#[test]
fn null_source_logs_and_returns() {
    let model = Model::new(false, true, false);
    show_preloaded_video(&model, None);
    assert_eq!(
        model.events(),
        [Event::NullSource(c"obs_source_show_preloaded_video")]
    );
    assert!(model.idle());
}

/// `test_source_show_preloaded_video::destroying_source`: the preload frame
/// is not even looked at.
#[test]
fn destroying_source_is_left_alone() {
    let model = Model::new(true, true, false);
    show_preloaded_video(&model, Some(()));
    assert_eq!(model.events(), [Event::Destroying]);
    let s = model.0.borrow();
    assert!(s.textures_stale && !s.active);
}

/// `test_source_show_preloaded_video::without_preload_frame`.
#[test]
fn without_preload_frame_is_left_alone() {
    let model = Model::new(false, false, false);
    show_preloaded_video(&model, Some(()));
    assert_eq!(model.events(), [Event::Destroying, Event::HasPreload]);
    let s = model.0.borrow();
    assert!(s.textures_stale && !s.active);
}

fn shown(clock: &[Event], sys_ts: u64) -> Vec<Event> {
    let mut events = vec![
        Event::Destroying,
        Event::HasPreload,
        Event::EnterGraphics,
        Event::SetTextureSize,
        Event::UpdateTextures,
        Event::MarkFresh,
        Event::MarkActive,
        Event::LeaveGraphics,
        Event::LockAsync,
        Event::LastFrameTs,
        Event::UnlockAsync,
        Event::LockAudio,
        Event::MonitorOnly,
    ];
    events.extend_from_slice(clock);
    events.extend([
        Event::ResetTiming(5_000_000, sys_ts),
        Event::ResetData(sys_ts),
        Event::UnlockAudio,
    ]);
    events
}

/// `test_source_show_preloaded_video::shows_at_current_time` and
/// `shows_unmonitored_at_current_time`: one clock read, after the
/// monitoring type, under the audio mutex.
#[test]
fn shows_at_current_time() {
    let model = Model::new(false, true, false);
    show_preloaded_video(&model, Some(()));
    assert_eq!(model.events(), shown(&[Event::Now], 9_000_000));
    assert!(model.idle());
    let s = model.0.borrow();
    assert!(!s.textures_stale && s.active);
}

/// `test_source_show_preloaded_video::monitor_only_shows_at_time_zero`: no
/// clock read.
#[test]
fn monitor_only_shows_at_time_zero() {
    let model = Model::new(false, true, true);
    show_preloaded_video(&model, Some(()));
    assert_eq!(model.events(), shown(&[], 0));
    assert!(model.idle());
    let s = model.0.borrow();
    assert!(!s.textures_stale && s.active);
}

/// The timestamps pass through unchanged at their extremes: no arithmetic
/// happens here (`reset_audio_timing()` does the subtraction).
#[test]
fn timestamps_pass_through_unchanged() {
    for (last, now) in [(0, 0), (u64::MAX, 0), (0, u64::MAX), (u64::MAX, u64::MAX)] {
        let model = Model::new(false, true, false);
        {
            let mut s = model.0.borrow_mut();
            s.last_frame_ts = last;
            s.now = now;
        }
        show_preloaded_video(&model, Some(()));
        let events = model.events();
        assert!(events.contains(&Event::ResetTiming(last, now)));
        assert!(events.contains(&Event::ResetData(now)));
    }
}

/// Repeated calls do the whole sequence again: nothing is cached.
#[test]
fn repeated_calls_repeat_the_sequence() {
    let model = Model::new(false, true, false);
    show_preloaded_video(&model, Some(()));
    let first = model.events();
    show_preloaded_video(&model, Some(()));
    assert_eq!(model.events(), first);
}

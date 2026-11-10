//! Tier 1: `obs_source::deinterlace`, the rest of
//! `libobs/obs-source-deinterlace.c.h`: the frame take, the texture update,
//! the render and the mode and field-order setters.
//!
//! A fake source keeps the fields, frames (by id), textures, texrenders,
//! effects and parameters as small integers, answers the graphics calls
//! from its settings, and logs every lock, slot write, erase and call into
//! the rest of libobs in order. Each case pins the log and the fields left
//! behind: the walk's drops (each erased, then back to the cache), the
//! unbuffered pair and the flag on the front frame, the take of one frame
//! or a pair with its half duration and offset, the update's upload and
//! swap with the slots read under `async_mutex`, the render's lookups,
//! skip, sRGB switch, parameters and draw loop, and each mode change with
//! its lock order.

use std::cell::RefCell;
use std::ffi::CStr;

use obs_source::deinterlace::{
    self, FLIP_V, MAX_AV_PLANES, PARAM_DIMENSIONS, PARAM_FIELD_ORDER, PARAM_FRAME2, PARAM_IMAGE,
    PARAM_MULTIPLIER, PARAM_PREVIOUS_IMAGE, SET_FIELD_ORDER_LOG_NAME, SET_MODE_LOG_NAME, Source,
};
use obs_source::deinterlace_draw::{self, Effect};
use obs_source::deinterlace_pacing;

const MS: u64 = 1_000_000;
/// A frame timestamp on the source's clock.
const T: u64 = 1_000_000_000_000;
/// A field at 60i.
const FIELD: u64 = 16_666_667;
/// The canvas interval at 30 fps.
const INTERVAL: u64 = 33_333_333;
/// The video time.
const VIDEO_TIME: u64 = 9_000_000_000_000;

#[derive(Debug, Clone, PartialEq)]
enum Call {
    Valid(bool, &'static CStr),
    Lock,
    Unlock,
    Erase(u32),
    SetCur(Option<u32>),
    SetPrev(Option<u32>),
    SetFlag(u32, bool),
    Addref(u32),
    Remove(u32),
    SetAsyncTextureSize(u32),
    UpdatePrevTextures(u32, Option<u32>),
    Release(u32),
    SetDeinterlaceTextureSize,
    Load(Effect),
    Enter,
    Leave,
    Param(Option<u32>, &'static CStr),
    TexrenderTexture(u32),
    EnableFramebufferSrgb(bool),
    SetTextureSrgb(Option<u32>, Option<u32>),
    SetTexture(Option<u32>, Option<u32>),
    SetFloat(Option<u32>, f32),
    SetInt(Option<u32>, i32),
    SetVec2(Option<u32>, [f32; 2]),
    SetBool(Option<u32>, bool),
    Loop(Option<u32>, &'static CStr),
    Draw(u32, u32, u32),
    TextureDestroy(Option<u32>),
    TexrenderDestroy(Option<u32>),
}

#[derive(Debug, Clone, Default)]
struct State {
    queue: Vec<u32>,
    /// Each frame's timestamp and `prev_frame`, by id.
    frames: Vec<(u64, bool)>,
    refs: Vec<u32>,
    unbuffered: bool,
    last_sys_timestamp: u64,
    last_frame_ts: u64,
    offset: u64,
    frame_ts: u64,
    half_duration: u32,
    cur: Option<u32>,
    prev: Option<u32>,
    format: i32,
    trc: i32,
    width: u32,
    height: u32,
    flip: bool,
    textures: [Option<u32>; MAX_AV_PLANES],
    prev_textures: [Option<u32>; MAX_AV_PLANES],
    texrender: Option<u32>,
    prev_texrender: Option<u32>,
    rendered: bool,
    effect: Option<u32>,
    mode: i32,
    top_first: bool,
    space: i32,
    interval: u64,
    video_time: u64,
    /// What `set_async_texture_size()` answers.
    texture_size_ok: bool,
    /// The texture of texrender `n` is `n + 100`, or none.
    texrender_has_texture: bool,
    linear_srgb: bool,
    color_space: i32,
    framebuffer_srgb: bool,
    white_level: f32,
    /// How many times `gs_effect_loop()` answers true.
    loops: u32,
    /// Whether `gs_effect_get_param_by_name()` finds a parameter.
    params_found: bool,
}

struct Fake {
    state: RefCell<State>,
    log: RefCell<Vec<Call>>,
    locked: RefCell<bool>,
}

impl Fake {
    fn new(state: State) -> Self {
        Self {
            state: RefCell::new(state),
            log: RefCell::new(Vec::new()),
            locked: RefCell::new(false),
        }
    }

    fn call(&self, call: Call) {
        self.log.borrow_mut().push(call);
    }

    fn take_log(&self) -> Vec<Call> {
        std::mem::take(&mut *self.log.borrow_mut())
    }

    fn st(&self) -> std::cell::Ref<'_, State> {
        self.state.borrow()
    }

    fn st_mut(&self) -> std::cell::RefMut<'_, State> {
        self.state.borrow_mut()
    }

    fn frame_index(frame: u32) -> usize {
        usize::try_from(frame).unwrap()
    }
}

/// The effect id `load_effect()` gives: 10 plus the effect's index.
fn effect_id(effect: Effect) -> u32 {
    10 + effect as u32
}

impl Source for Fake {
    type Frame = u32;
    type Texture = u32;
    type Texrender = u32;
    type Effect = u32;
    type Param = u32;

    fn valid(source: Option<&Self>, func: &'static CStr) -> bool {
        if let Some(s) = source {
            s.call(Call::Valid(true, func));
        }
        source.is_some()
    }

    fn frames_num(&self) -> usize {
        self.st().queue.len()
    }
    fn frame(&self, idx: usize) -> u32 {
        self.st().queue[idx]
    }
    fn erase_front(&self) {
        let front = self.st_mut().queue.remove(0);
        self.call(Call::Erase(front));
    }
    fn lock(&self) {
        assert!(!*self.locked.borrow(), "async_mutex taken twice");
        *self.locked.borrow_mut() = true;
        self.call(Call::Lock);
    }
    fn unlock(&self) {
        assert!(*self.locked.borrow(), "async_mutex not held");
        *self.locked.borrow_mut() = false;
        self.call(Call::Unlock);
    }

    fn async_unbuffered(&self) -> bool {
        self.st().unbuffered
    }
    fn last_sys_timestamp(&self) -> u64 {
        self.st().last_sys_timestamp
    }
    fn last_frame_ts(&self) -> u64 {
        self.st().last_frame_ts
    }
    fn set_last_frame_ts(&self, value: u64) {
        self.st_mut().last_frame_ts = value;
    }
    fn offset(&self) -> u64 {
        self.st().offset
    }
    fn set_offset(&self, value: u64) {
        self.st_mut().offset = value;
    }
    fn frame_ts(&self) -> u64 {
        self.st().frame_ts
    }
    fn set_frame_ts(&self, value: u64) {
        self.st_mut().frame_ts = value;
    }
    fn half_duration(&self) -> u32 {
        self.st().half_duration
    }
    fn set_half_duration(&self, value: u32) {
        self.st_mut().half_duration = value;
    }

    fn cur_frame(&self) -> Option<u32> {
        self.st().cur
    }
    fn set_cur_frame(&self, value: Option<u32>) {
        self.st_mut().cur = value;
        self.call(Call::SetCur(value));
    }
    fn prev_frame(&self) -> Option<u32> {
        self.st().prev
    }
    fn set_prev_frame(&self, value: Option<u32>) {
        self.st_mut().prev = value;
        self.call(Call::SetPrev(value));
    }

    fn frame_timestamp(&self, frame: u32) -> u64 {
        self.st().frames[Self::frame_index(frame)].0
    }
    fn frame_prev_flag(&self, frame: u32) -> bool {
        self.st().frames[Self::frame_index(frame)].1
    }
    fn set_frame_prev_flag(&self, frame: u32, value: bool) {
        self.st_mut().frames[Self::frame_index(frame)].1 = value;
        self.call(Call::SetFlag(frame, value));
    }
    fn frame_addref(&self, frame: u32) {
        let mut st = self.st_mut();
        if st.refs.len() <= Self::frame_index(frame) {
            st.refs.resize(Self::frame_index(frame) + 1, 0);
        }
        st.refs[Self::frame_index(frame)] += 1;
        drop(st);
        self.call(Call::Addref(frame));
    }

    fn async_format(&self) -> i32 {
        self.st().format
    }
    fn async_trc(&self) -> i32 {
        self.st().trc
    }
    fn async_width(&self) -> u32 {
        self.st().width
    }
    fn async_height(&self) -> u32 {
        self.st().height
    }
    fn async_flip(&self) -> bool {
        self.st().flip
    }
    fn texture(&self, idx: usize) -> Option<u32> {
        self.st().textures[idx]
    }
    fn set_texture(&self, idx: usize, value: Option<u32>) {
        self.st_mut().textures[idx] = value;
    }
    fn prev_texture(&self, idx: usize) -> Option<u32> {
        self.st().prev_textures[idx]
    }
    fn set_prev_texture(&self, idx: usize, value: Option<u32>) {
        self.st_mut().prev_textures[idx] = value;
    }
    fn texrender(&self) -> Option<u32> {
        self.st().texrender
    }
    fn set_texrender(&self, value: Option<u32>) {
        self.st_mut().texrender = value;
    }
    fn prev_texrender(&self) -> Option<u32> {
        self.st().prev_texrender
    }
    fn set_prev_texrender(&self, value: Option<u32>) {
        self.st_mut().prev_texrender = value;
    }

    fn rendered(&self) -> bool {
        self.st().rendered
    }
    fn set_rendered(&self, value: bool) {
        self.st_mut().rendered = value;
    }
    fn effect(&self) -> Option<u32> {
        self.st().effect
    }
    fn set_effect(&self, value: Option<u32>) {
        self.st_mut().effect = value;
    }
    fn mode(&self) -> i32 {
        self.st().mode
    }
    fn set_mode(&self, value: i32) {
        self.st_mut().mode = value;
    }
    fn top_first(&self) -> bool {
        self.st().top_first
    }
    fn set_top_first(&self, value: bool) {
        self.st_mut().top_first = value;
    }

    fn convert_video_space(&self, format: i32, trc: i32) -> i32 {
        assert_eq!((format, trc), (self.st().format, self.st().trc));
        self.st().space
    }
    fn frame_interval_ns(&self) -> u64 {
        self.st().interval
    }
    fn video_time(&self) -> u64 {
        self.st().video_time
    }
    fn load_effect(&self, effect: Effect) -> Option<u32> {
        self.call(Call::Load(effect));
        Some(effect_id(effect))
    }

    fn remove_async_frame(&self, frame: u32) {
        self.call(Call::Remove(frame));
    }
    fn set_async_texture_size(&self, frame: u32) -> bool {
        self.call(Call::SetAsyncTextureSize(frame));
        self.st().texture_size_ok
    }
    fn update_prev_textures(&self, frame: u32, prev_texrender: Option<u32>) -> bool {
        self.call(Call::UpdatePrevTextures(frame, prev_texrender));
        true
    }
    fn release_frame(&self, frame: u32) {
        self.call(Call::Release(frame));
    }
    fn set_deinterlace_texture_size(&self) {
        self.call(Call::SetDeinterlaceTextureSize);
    }

    fn enter_graphics(&self) {
        self.call(Call::Enter);
    }
    fn leave_graphics(&self) {
        self.call(Call::Leave);
    }
    fn sdr_white_level(&self) -> f32 {
        self.st().white_level
    }

    fn param_by_name(&self, effect: Option<u32>, name: &'static CStr) -> Option<u32> {
        self.call(Call::Param(effect, name));
        let order = [
            PARAM_IMAGE,
            PARAM_PREVIOUS_IMAGE,
            PARAM_MULTIPLIER,
            PARAM_FIELD_ORDER,
            PARAM_FRAME2,
            PARAM_DIMENSIONS,
        ];
        let idx = order.iter().position(|n| *n == name).unwrap();
        (self.st().params_found && effect.is_some()).then(|| 200 + u32::try_from(idx).unwrap())
    }
    fn texrender_texture(&self, texrender: u32) -> Option<u32> {
        self.call(Call::TexrenderTexture(texrender));
        self.st().texrender_has_texture.then_some(texrender + 100)
    }
    fn linear_srgb(&self) -> bool {
        self.st().linear_srgb
    }
    fn color_space(&self) -> i32 {
        self.st().color_space
    }
    fn framebuffer_srgb_enabled(&self) -> bool {
        self.st().framebuffer_srgb
    }
    fn enable_framebuffer_srgb(&self, enable: bool) {
        self.st_mut().framebuffer_srgb = enable;
        self.call(Call::EnableFramebufferSrgb(enable));
    }
    fn set_param_texture_srgb(&self, param: Option<u32>, texture: Option<u32>) {
        self.call(Call::SetTextureSrgb(param, texture));
    }
    fn set_param_texture(&self, param: Option<u32>, texture: Option<u32>) {
        self.call(Call::SetTexture(param, texture));
    }
    fn set_param_float(&self, param: Option<u32>, value: f32) {
        self.call(Call::SetFloat(param, value));
    }
    fn set_param_int(&self, param: Option<u32>, value: i32) {
        self.call(Call::SetInt(param, value));
    }
    fn set_param_vec2(&self, param: Option<u32>, value: [f32; 2]) {
        self.call(Call::SetVec2(param, value));
    }
    fn set_param_bool(&self, param: Option<u32>, value: bool) {
        self.call(Call::SetBool(param, value));
    }
    fn effect_loop(&self, effect: Option<u32>, technique: &'static CStr) -> bool {
        self.call(Call::Loop(effect, technique));
        let mut st = self.st_mut();
        if st.loops == 0 {
            return false;
        }
        st.loops -= 1;
        true
    }
    fn draw_sprite(&self, flip: u32, width: u32, height: u32) {
        self.call(Call::Draw(flip, width, height));
    }
    fn texture_destroy(&self, texture: Option<u32>) {
        self.call(Call::TextureDestroy(texture));
    }
    fn texrender_destroy(&self, texrender: Option<u32>) {
        self.call(Call::TexrenderDestroy(texrender));
    }
}

/// A source with frames 0.. at `timestamps`, all queued.
fn queued(timestamps: &[u64]) -> State {
    State {
        queue: (0..u32::try_from(timestamps.len()).unwrap()).collect(),
        frames: timestamps.iter().map(|&ts| (ts, false)).collect(),
        interval: INTERVAL,
        video_time: VIDEO_TIME,
        ..State::default()
    }
}

// ---------------------------------------------------------------------------
// process_last_frame: the release and the take

#[test]
fn process_releases_prev_then_cur_with_an_empty_queue() {
    let mut st = queued(&[T, T + FIELD]);
    st.queue.clear();
    st.cur = Some(1);
    st.prev = Some(0);
    st.last_frame_ts = T;
    let s = Fake::new(st);
    deinterlace::process_last_frame(&s, T);
    assert_eq!(
        s.take_log(),
        [
            Call::Remove(0),
            Call::SetPrev(None),
            Call::Remove(1),
            Call::SetCur(None),
        ]
    );
    assert_eq!(s.st().last_frame_ts, T);
}

#[test]
fn process_with_nothing_held_and_nothing_queued_does_nothing() {
    let s = Fake::new(queued(&[]));
    deinterlace::process_last_frame(&s, T);
    assert!(s.take_log().is_empty());
}

#[test]
fn first_frame_alone_is_taken_and_starts_the_clock() {
    let s = Fake::new(queued(&[T]));
    deinterlace::process_last_frame(&s, 5 * MS);
    assert_eq!(
        s.take_log(),
        [Call::SetPrev(None), Call::SetCur(Some(0)), Call::Erase(0)]
    );
    let st = s.st();
    assert_eq!(st.cur, Some(0));
    assert_eq!(st.prev, None);
    assert!(st.queue.is_empty());
    assert_eq!(st.last_frame_ts, T);
    assert_eq!(st.frame_ts, T);
    assert_eq!(
        st.half_duration,
        deinterlace_pacing::half_duration(T, 0, 0, INTERVAL)
    );
    assert_eq!(
        st.offset,
        deinterlace_pacing::buffered_offset(VIDEO_TIME, T)
    );
}

#[test]
fn first_frame_with_two_queued_flags_the_front_and_takes_the_pair() {
    let s = Fake::new(queued(&[T, T + FIELD, T + 2 * FIELD]));
    deinterlace::process_last_frame(&s, 5 * MS);
    assert_eq!(
        s.take_log(),
        [
            Call::SetFlag(0, true),
            Call::SetPrev(None),
            Call::SetCur(Some(0)),
            Call::Erase(0),
            Call::SetPrev(Some(0)),
            Call::SetCur(Some(1)),
            Call::Erase(1),
        ]
    );
    let st = s.st();
    assert_eq!((st.prev, st.cur), (Some(0), Some(1)));
    assert_eq!(st.queue, [2]);
    assert_eq!(
        st.half_duration,
        deinterlace_pacing::half_duration(T + FIELD, T, 0, INTERVAL)
    );
    assert_eq!(st.last_frame_ts, T + FIELD);
    assert_eq!(st.frame_ts, T + FIELD);
}

#[test]
fn first_frame_with_exactly_two_queued_does_not_read_the_flag_into_a_pair() {
    // Two queued: the flag is set, but one remains after the take, and
    // take_reads_flag(1) reads it: the pair is taken.
    let s = Fake::new(queued(&[T, T + FIELD]));
    deinterlace::process_last_frame(&s, 5 * MS);
    let st = s.st();
    assert_eq!((st.prev, st.cur), (Some(0), Some(1)));
    assert!(st.queue.is_empty());
}

#[test]
fn unbuffered_hold_keeps_everything() {
    let mut st = queued(&[T, T + FIELD]);
    st.unbuffered = true;
    st.offset = 3 * MS;
    st.frame_ts = T;
    st.half_duration = 8 * MS as u32;
    st.last_frame_ts = T;
    let sys_time = T + 3 * MS + MS;
    assert!(deinterlace_pacing::hold(T, 3 * MS, 8_000_000, sys_time));
    let s = Fake::new(st);
    deinterlace::process_last_frame(&s, sys_time);
    assert!(s.take_log().is_empty());
    assert_eq!(s.st().queue, [0, 1]);
}

#[test]
fn unbuffered_walk_drops_all_but_two_then_pairs() {
    let ts = [T, T + FIELD, T + 2 * FIELD, T + 3 * FIELD, T + 4 * FIELD];
    let mut st = queued(&ts);
    st.unbuffered = true;
    st.last_frame_ts = T;
    st.offset = 0;
    let sys_time = T + 4 * FIELD;
    let s = Fake::new(st);
    deinterlace::process_last_frame(&s, sys_time);
    let pair = obs_source::deinterlace_select::unbuffered_pair(ts[3], ts[4], 0, sys_time);
    let log = s.take_log();
    assert_eq!(
        log[..7],
        [
            Call::Erase(0),
            Call::Remove(0),
            Call::Erase(1),
            Call::Remove(1),
            Call::Erase(2),
            Call::Remove(2),
            Call::SetFlag(3, pair.prev_frame),
        ]
    );
    let st = s.st();
    assert_eq!(st.last_frame_ts, T + 3 * FIELD);
    if pair.prev_frame {
        assert_eq!((st.prev, st.cur), (Some(3), Some(4)));
    } else {
        assert_eq!((st.prev, st.cur), (None, Some(3)));
    }
}

#[test]
fn buffered_walk_drops_frames_that_are_due_in_pairs() {
    // last_frame_ts T: the clock moves 5 fields between ticks, so the
    // first frames are all due.
    let ts: Vec<u64> = (0..8).map(|i| T + i * FIELD).collect();
    let mut st = queued(&ts);
    st.last_frame_ts = T;
    st.last_sys_timestamp = 100 * MS;
    let sys_time = 100 * MS + 5 * FIELD;
    let walk = obs_source::deinterlace_select::walk(
        &ts,
        sys_time,
        obs_source::deinterlace_select::State {
            last_frame_ts: T,
            last_sys_timestamp: 100 * MS,
            async_unbuffered: false,
        },
    );
    assert!(walk.drop_front > 0);
    let s = Fake::new(st);
    deinterlace::process_last_frame(&s, sys_time);
    let log = s.take_log();
    let mut want = Vec::new();
    for i in 0..u32::try_from(walk.drop_front).unwrap() {
        want.push(Call::Erase(i));
        want.push(Call::Remove(i));
    }
    assert_eq!(log[..want.len()], want[..]);
    assert_eq!(s.st().last_frame_ts, walk.last_frame_ts);
    assert_eq!(walk.ready, s.st().cur.is_some());
}

#[test]
fn buffered_time_jump_resets_the_offset_and_takes_the_frame() {
    let jumped = T + 10_000 * MS;
    let mut st = queued(&[jumped, jumped + FIELD, jumped + 2 * FIELD]);
    st.last_frame_ts = T;
    st.offset = 77;
    st.last_sys_timestamp = 100 * MS;
    let s = Fake::new(st);
    deinterlace::process_last_frame(&s, 100 * MS + FIELD);
    let st = s.st();
    assert_eq!((st.prev, st.cur), (None, Some(0)));
    assert_eq!(st.queue, [1, 2]);
    assert_eq!(st.last_frame_ts, jumped);
    assert_eq!(st.frame_ts, jumped);
    assert_eq!(
        st.offset,
        deinterlace_pacing::buffered_offset(VIDEO_TIME, jumped)
    );
}

#[test]
fn walk_reads_a_queue_longer_than_the_stack_copy() {
    let ts: Vec<u64> = (0..40).map(|i| T + i * FIELD).collect();
    let mut st = queued(&ts);
    st.unbuffered = true;
    st.last_frame_ts = T;
    let s = Fake::new(st);
    deinterlace::process_last_frame(&s, T + 39 * FIELD);
    let log = s.take_log();
    let erases = log.iter().filter(|c| matches!(c, Call::Erase(_))).count();
    assert!(erases >= 38);
    assert_eq!(log[0], Call::Erase(0));
    assert_eq!(log[1], Call::Remove(0));
    assert_eq!(s.st().last_frame_ts, T + 38 * FIELD);
}

#[test]
fn unbuffered_take_reanchors_the_offset() {
    let mut st = queued(&[T]);
    st.unbuffered = true;
    st.last_frame_ts = T - FIELD;
    st.offset = 0;
    let s = Fake::new(st);
    deinterlace::process_last_frame(&s, T);
    let st = s.st();
    assert_eq!(st.cur, Some(0));
    let half = deinterlace_pacing::half_duration(T, 0, 0, INTERVAL);
    assert_eq!(st.half_duration, half);
    assert_eq!(
        st.offset,
        deinterlace_pacing::reanchor(0, VIDEO_TIME, T, half, INTERVAL)
    );
}

// ---------------------------------------------------------------------------
// update_async_video

#[test]
fn update_runs_once_a_frame() {
    let mut st = queued(&[T]);
    st.rendered = true;
    st.prev = Some(0);
    let s = Fake::new(st);
    deinterlace::update_async_video(&s);
    assert!(s.take_log().is_empty());
    assert_eq!(s.st().prev, Some(0));
}

#[test]
fn update_uploads_the_previous_frame_outside_the_lock() {
    let mut st = queued(&[T, T + FIELD]);
    st.queue.clear();
    st.prev = Some(0);
    st.cur = Some(1);
    st.prev_texrender = Some(7);
    st.texture_size_ok = true;
    let s = Fake::new(st);
    deinterlace::update_async_video(&s);
    assert_eq!(
        s.take_log(),
        [
            Call::Lock,
            Call::SetPrev(None),
            Call::Unlock,
            Call::Addref(0),
            Call::SetAsyncTextureSize(0),
            Call::UpdatePrevTextures(0, Some(7)),
            Call::Release(0),
        ]
    );
    assert!(s.st().rendered);
    assert_eq!(s.st().refs[0], 1);
}

#[test]
fn update_skips_the_upload_when_the_size_fails() {
    let mut st = queued(&[T]);
    st.queue.clear();
    st.prev = Some(0);
    st.texture_size_ok = false;
    let s = Fake::new(st);
    deinterlace::update_async_video(&s);
    assert_eq!(
        s.take_log(),
        [
            Call::Lock,
            Call::SetPrev(None),
            Call::Unlock,
            Call::Addref(0),
            Call::SetAsyncTextureSize(0),
            Call::Release(0),
        ]
    );
}

#[test]
fn update_swaps_the_textures_and_the_texrenders() {
    let mut st = queued(&[T]);
    st.queue.clear();
    st.cur = Some(0);
    for c in 0..MAX_AV_PLANES {
        st.textures[c] = Some(u32::try_from(c).unwrap() + 1);
        st.prev_textures[c] = (c % 2 == 0).then_some(u32::try_from(c).unwrap() + 50);
    }
    st.texrender = Some(3);
    st.prev_texrender = Some(4);
    let s = Fake::new(st);
    deinterlace::update_async_video(&s);
    assert_eq!(
        s.take_log(),
        [Call::Lock, Call::SetPrev(None), Call::Unlock]
    );
    let st = s.st();
    for c in 0..MAX_AV_PLANES {
        assert_eq!(st.prev_textures[c], Some(u32::try_from(c).unwrap() + 1));
        assert_eq!(
            st.textures[c],
            (c % 2 == 0).then_some(u32::try_from(c).unwrap() + 50)
        );
    }
    assert_eq!((st.texrender, st.prev_texrender), (Some(4), Some(3)));
}

#[test]
fn update_swap_leaves_the_texrenders_without_a_current_one() {
    let mut st = queued(&[T]);
    st.queue.clear();
    st.cur = Some(0);
    st.textures[0] = Some(1);
    st.prev_texrender = Some(4);
    let s = Fake::new(st);
    deinterlace::update_async_video(&s);
    let st = s.st();
    assert_eq!((st.textures[0], st.prev_textures[0]), (None, Some(1)));
    assert_eq!((st.texrender, st.prev_texrender), (None, Some(4)));
}

#[test]
fn update_with_no_frame_does_nothing_but_the_flag() {
    let mut st = queued(&[]);
    st.textures[0] = Some(1);
    let s = Fake::new(st);
    deinterlace::update_async_video(&s);
    assert_eq!(
        s.take_log(),
        [Call::Lock, Call::SetPrev(None), Call::Unlock]
    );
    assert_eq!(s.st().textures[0], Some(1));
    assert!(s.st().rendered);
}

// ---------------------------------------------------------------------------
// render

fn lookups(effect: Option<u32>) -> Vec<Call> {
    [
        PARAM_IMAGE,
        PARAM_PREVIOUS_IMAGE,
        PARAM_MULTIPLIER,
        PARAM_FIELD_ORDER,
        PARAM_FRAME2,
        PARAM_DIMENSIONS,
    ]
    .into_iter()
    .map(|name| Call::Param(effect, name))
    .collect()
}

fn drawable() -> State {
    State {
        effect: Some(17),
        width: 64,
        height: 48,
        textures: [Some(1), None, None, None, None, None, None, None],
        prev_textures: [Some(2), None, None, None, None, None, None, None],
        mode: deinterlace_draw::MODE_YADIF,
        params_found: true,
        loops: 2,
        video_time: VIDEO_TIME,
        frame_ts: T,
        offset: 5,
        half_duration: 9,
        white_level: 300.0,
        ..State::default()
    }
}

#[test]
fn render_skips_without_a_current_texture() {
    let mut st = drawable();
    st.textures[0] = None;
    let s = Fake::new(st);
    deinterlace::render(&s);
    assert_eq!(s.take_log(), lookups(Some(17)));
}

#[test]
fn render_skips_without_a_previous_texture_or_a_size() {
    for (prev, width, height) in [(None, 64, 48), (Some(2), 0, 48), (Some(2), 64, 0)] {
        let mut st = drawable();
        st.prev_textures[0] = prev;
        st.width = width;
        st.height = height;
        let s = Fake::new(st);
        deinterlace::render(&s);
        assert_eq!(s.take_log(), lookups(Some(17)));
    }
}

#[test]
fn render_draws_with_the_textures_and_parameters() {
    let mut st = drawable();
    st.top_first = true;
    st.flip = true;
    let s = Fake::new(st);
    deinterlace::render(&s);
    let space = 0;
    let tech = deinterlace::technique_name(deinterlace_draw::technique(space, 0));
    let mut want = lookups(Some(17));
    want.extend([Call::EnableFramebufferSrgb(deinterlace_draw::linear_srgb(
        space,
        false,
        deinterlace_draw::MODE_YADIF,
    ))]);
    let linear = deinterlace_draw::linear_srgb(space, false, deinterlace_draw::MODE_YADIF);
    if linear {
        want.extend([
            Call::SetTextureSrgb(Some(200), Some(1)),
            Call::SetTextureSrgb(Some(201), Some(2)),
        ]);
    } else {
        want.extend([
            Call::SetTexture(Some(200), Some(1)),
            Call::SetTexture(Some(201), Some(2)),
        ]);
    }
    want.extend([
        Call::SetFloat(Some(202), 1.0),
        Call::SetInt(Some(203), 1),
        Call::SetVec2(Some(205), [64.0, 48.0]),
        Call::SetBool(Some(204), deinterlace_draw::frame2(VIDEO_TIME, T, 5, 9)),
        Call::Loop(Some(17), tech),
        Call::Draw(FLIP_V, 64, 48),
        Call::Loop(Some(17), tech),
        Call::Draw(FLIP_V, 64, 48),
        Call::Loop(Some(17), tech),
        Call::EnableFramebufferSrgb(false),
    ]);
    assert_eq!(s.take_log(), want);
}

#[test]
fn render_linear_mode_uses_srgb_textures_and_restores_the_framebuffer() {
    let mut st = drawable();
    st.mode = deinterlace_draw::MODE_LINEAR;
    st.linear_srgb = true;
    st.framebuffer_srgb = true;
    st.loops = 0;
    let s = Fake::new(st);
    deinterlace::render(&s);
    let log = s.take_log();
    let linear = deinterlace_draw::linear_srgb(
        0,
        deinterlace_draw::reads_linear_srgb(0),
        deinterlace_draw::MODE_LINEAR,
    );
    assert!(linear);
    assert_eq!(log[6], Call::EnableFramebufferSrgb(true));
    assert_eq!(log[7], Call::SetTextureSrgb(Some(200), Some(1)));
    assert_eq!(log[8], Call::SetTextureSrgb(Some(201), Some(2)));
    assert_eq!(log.last(), Some(&Call::EnableFramebufferSrgb(true)));
}

#[test]
fn render_takes_the_texrender_textures() {
    let mut st = drawable();
    st.texrender = Some(5);
    st.prev_texrender = Some(6);
    st.texrender_has_texture = true;
    st.loops = 0;
    let s = Fake::new(st);
    deinterlace::render(&s);
    let log = s.take_log();
    assert_eq!(log[6], Call::TexrenderTexture(5));
    assert_eq!(log[7], Call::TexrenderTexture(6));
    let linear = deinterlace_draw::linear_srgb(0, false, deinterlace_draw::MODE_YADIF);
    if linear {
        assert_eq!(log[9], Call::SetTextureSrgb(Some(200), Some(105)));
        assert_eq!(log[10], Call::SetTextureSrgb(Some(201), Some(106)));
    } else {
        assert_eq!(log[9], Call::SetTexture(Some(200), Some(105)));
        assert_eq!(log[10], Call::SetTexture(Some(201), Some(106)));
    }
}

#[test]
fn render_skips_when_a_texrender_has_no_texture() {
    let mut st = drawable();
    st.texrender = Some(5);
    st.texrender_has_texture = false;
    let s = Fake::new(st);
    deinterlace::render(&s);
    let mut want = lookups(Some(17));
    want.push(Call::TexrenderTexture(5));
    assert_eq!(s.take_log(), want);
}

#[test]
fn render_multiplies_by_the_white_level_into_scrgb() {
    let mut st = drawable();
    st.color_space = deinterlace_draw::GS_CS_709_SCRGB;
    st.loops = 0;
    let s = Fake::new(st);
    deinterlace::render(&s);
    let want = deinterlace_draw::multiplier(0, deinterlace_draw::GS_CS_709_SCRGB, 300.0);
    assert!(s.take_log().contains(&Call::SetFloat(Some(202), want)));
}

#[test]
fn render_without_an_effect_passes_nulls() {
    let mut st = drawable();
    st.effect = None;
    st.loops = 0;
    let s = Fake::new(st);
    deinterlace::render(&s);
    let log = s.take_log();
    assert_eq!(log[..6], lookups(None)[..]);
    assert!(log.contains(&Call::SetFloat(None, 1.0)));
}

// ---------------------------------------------------------------------------
// set_mode and set_field_order

#[test]
fn set_mode_on_no_source_does_nothing() {
    deinterlace::set_mode::<Fake>(None, deinterlace_draw::MODE_BLEND);
    deinterlace::set_field_order::<Fake>(None, 0);
}

#[test]
fn set_mode_same_does_nothing_more() {
    let mut st = queued(&[]);
    st.mode = deinterlace_draw::MODE_BLEND;
    let s = Fake::new(st);
    deinterlace::set_mode(Some(&s), deinterlace_draw::MODE_BLEND);
    assert_eq!(s.take_log(), [Call::Valid(true, SET_MODE_LOG_NAME)]);
}

#[test]
fn enable_without_a_frame_loads_the_effect_and_releases_the_previous_frame() {
    let mut st = queued(&[T]);
    st.queue.clear();
    st.prev = Some(0);
    let s = Fake::new(st);
    deinterlace::set_mode(Some(&s), deinterlace_draw::MODE_BLEND);
    assert_eq!(
        s.take_log(),
        [
            Call::Valid(true, SET_MODE_LOG_NAME),
            Call::Enter,
            Call::Load(Effect::Blend),
            Call::Lock,
            Call::Remove(0),
            Call::SetPrev(None),
            Call::Unlock,
            Call::Leave,
        ]
    );
    assert_eq!(s.st().mode, deinterlace_draw::MODE_BLEND);
    assert_eq!(s.st().effect, Some(effect_id(Effect::Blend)));
}

#[test]
fn enable_with_a_frame_sizes_the_previous_textures() {
    for (format, width, height, sized) in [
        (3, 64, 48, true),
        (0, 64, 48, false),
        (3, 0, 48, false),
        (3, 64, 0, false),
    ] {
        let mut st = queued(&[]);
        st.format = format;
        st.width = width;
        st.height = height;
        let s = Fake::new(st);
        deinterlace::set_mode(Some(&s), deinterlace_draw::MODE_YADIF_2X);
        let mut want = vec![Call::Valid(true, SET_MODE_LOG_NAME), Call::Enter];
        if sized {
            want.push(Call::SetDeinterlaceTextureSize);
        }
        want.extend([
            Call::Load(Effect::Yadif2x),
            Call::Lock,
            Call::Unlock,
            Call::Leave,
        ]);
        assert_eq!(s.take_log(), want);
    }
}

#[test]
fn each_mode_loads_its_effect() {
    for (mode, effect) in [
        (deinterlace_draw::MODE_DISCARD, Effect::Discard),
        (deinterlace_draw::MODE_RETRO, Effect::Discard2x),
        (deinterlace_draw::MODE_BLEND, Effect::Blend),
        (deinterlace_draw::MODE_BLEND_2X, Effect::Blend2x),
        (deinterlace_draw::MODE_LINEAR, Effect::Linear),
        (deinterlace_draw::MODE_LINEAR_2X, Effect::Linear2x),
        (deinterlace_draw::MODE_YADIF, Effect::Yadif),
        (deinterlace_draw::MODE_YADIF_2X, Effect::Yadif2x),
    ] {
        let s = Fake::new(queued(&[]));
        deinterlace::set_mode(Some(&s), mode);
        assert!(s.take_log().contains(&Call::Load(effect)));
        assert_eq!(s.st().effect, Some(effect_id(effect)));
        assert_eq!(s.st().mode, mode);
    }
}

#[test]
fn switch_stores_the_mode_and_effect_in_the_graphics_context() {
    let mut st = queued(&[]);
    st.mode = deinterlace_draw::MODE_BLEND;
    st.effect = Some(1);
    st.prev_textures[0] = Some(9);
    let s = Fake::new(st);
    deinterlace::set_mode(Some(&s), deinterlace_draw::MODE_YADIF);
    assert_eq!(
        s.take_log(),
        [
            Call::Valid(true, SET_MODE_LOG_NAME),
            Call::Enter,
            Call::Load(Effect::Yadif),
            Call::Leave,
        ]
    );
    let st = s.st();
    assert_eq!(st.mode, deinterlace_draw::MODE_YADIF);
    assert_eq!(st.effect, Some(effect_id(Effect::Yadif)));
    assert_eq!(st.prev_textures[0], Some(9));
}

#[test]
fn disable_destroys_three_previous_planes_and_the_texrender() {
    for mode in [deinterlace_draw::MODE_DISABLE, 99, -1] {
        let mut st = queued(&[]);
        st.mode = deinterlace_draw::MODE_YADIF;
        st.effect = Some(1);
        st.prev_textures = [Some(1), Some(2), Some(3), Some(4), None, None, None, None];
        st.prev_texrender = Some(8);
        st.textures[0] = Some(5);
        let s = Fake::new(st);
        deinterlace::set_mode(Some(&s), mode);
        assert_eq!(
            s.take_log(),
            [
                Call::Valid(true, SET_MODE_LOG_NAME),
                Call::Enter,
                Call::TextureDestroy(Some(1)),
                Call::TextureDestroy(Some(2)),
                Call::TextureDestroy(Some(3)),
                Call::TexrenderDestroy(Some(8)),
                Call::Leave,
            ]
        );
        let st = s.st();
        assert_eq!(st.mode, deinterlace_draw::MODE_DISABLE);
        assert_eq!(st.effect, Some(1));
        assert_eq!(st.prev_textures[..4], [None, None, None, Some(4)]);
        assert_eq!(st.prev_texrender, None);
        assert_eq!(st.textures[0], Some(5));
    }
}

#[test]
fn out_of_range_mode_on_a_disabled_source_does_nothing() {
    let s = Fake::new(queued(&[]));
    deinterlace::set_mode(Some(&s), 99);
    assert_eq!(s.take_log(), [Call::Valid(true, SET_MODE_LOG_NAME)]);
    assert_eq!(s.st().mode, deinterlace_draw::MODE_DISABLE);
}

#[test]
fn field_order_is_top_first_only_for_top() {
    for (order, top_first) in [(0, true), (1, false), (7, false), (-1, false)] {
        let mut st = queued(&[]);
        st.top_first = !top_first;
        let s = Fake::new(st);
        deinterlace::set_field_order(Some(&s), order);
        assert_eq!(s.take_log(), [Call::Valid(true, SET_FIELD_ORDER_LOG_NAME)]);
        assert_eq!(s.st().top_first, top_first);
    }
}

//! Tier 2 layout and Tier 3 parity: the shims against the original
//! `libobs/obs-output-interleave.c.h` (oracle/output_interleave.c).
//!
//! `test_output_interleave` compiles the C into itself, so it never sees the
//! Rust swap; this file is what compares the two.
//!
//! Both sides run on a test output ([`Fixture`]) through the same stand-ins
//! defined here: the accessors `obs_output_interleave_fields`,
//! `obs_output_caption_track_fields`, `obs_encoder_interleave_info` and
//! `obs_encoder_group_num_started`, the `obs-output.c` wrappers (`active`,
//! the two first-encoder-index helpers, `add_caption`), the output's
//! `encoded_packet`, its packet callbacks and the two name getters. The
//! packet copy and release are the delay parity's (one definition per test
//! binary), which track every held copy. Each stand-in records its call,
//! with whether `interleaved_mutex`, `pkt_callbacks_mutex` and the caption
//! mutex were held, into the output it was given. `add_caption` and the
//! packet callbacks change the packet (and the callbacks the packet time),
//! so a side that passed a copy instead of its packet would show. The C
//! reads its clock from `oracle_output_interleave_clock_ns`, the shim from
//! the clock it is given; both are the same test counter, set to the same
//! value before each side runs.
//!
//! Each run configures one to two video and audio encoders (time bases,
//! frame sizes), then feeds one sequence of steps to a C output and a Rust
//! output: packets of random tracks with random keyframes, timestamps
//! (running per track, with jitter and steps back) and packet times;
//! changes of `active`, the delay (the packet is then the delay's held
//! copy), the batch limit, the captions (head, times, raw data), the
//! `add_caption` result, the packet callbacks and the encoder group; a
//! restart (what `reset_packet_data()` does, then `free_packets`);
//! `get_encoder_index`; and the file's static functions called directly
//! where their C preconditions hold. After every step it compares the
//! return value, the recorded calls, the log line, the held copies made and
//! released, the caller's packet, every queued packet field by field (its
//! encoder and group as an index), the packet times and keyframe group
//! entries, the darray capacities, and every interleaver field. The runs
//! that compare log lines swap the process-wide log handler, so they run in
//! a child process.
//!
//! Intentional differences: none. Nothing is excluded.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use std::cell::{Cell, RefCell};
use std::ffi::{CString, c_char, c_int, c_void};
use std::mem::{align_of, offset_of, size_of};
use std::ptr;

use obs_c_oracle::output_delay::{
    oracle_output_delay_mutex_create, oracle_output_delay_mutex_destroy,
    oracle_output_delay_mutex_held,
};
use obs_c_oracle::output_interleave::{
    LAYOUT_NAMES, VALUE_NAMES, oracle_free_packets, oracle_get_encoder_index,
    oracle_interleave_packets, oracle_output_interleave_apply_ept_offsets,
    oracle_output_interleave_check_group_keyframe, oracle_output_interleave_count_streamable,
    oracle_output_interleave_discard_unused_audio, oracle_output_interleave_initialize,
    oracle_output_interleave_layout, oracle_output_interleave_prune,
    oracle_output_interleave_prune_premature, oracle_output_interleave_resort,
    oracle_output_interleave_send, oracle_output_interleave_start_idx,
    oracle_output_interleave_value,
};
use obs_output::ffi::output_interleave::{
    KEYFRAME_TRACK_STATUS_NOT_SEEN, KEYFRAME_TRACK_STATUS_SEEN, KEYFRAME_TRACK_STATUS_SKIPPED,
    LOG_DEBUG, LOG_WARNING, OBS_ENCODER_AUDIO, OBS_ENCODER_VIDEO, caption_text, encoded_packet_t,
    encoder_packet, encoder_packet_time, free_packets, get_encoder_index, interleave_packets,
    interleave_packets_with_clock, keyframe_group_data, obs_encoder_interleave_info,
    obs_output_caption_track_fields, obs_output_interleave_fields, packet_callback, statics,
};
use obs_output::interleave::{MAX_OUTPUT_AUDIO_ENCODERS, MAX_OUTPUT_VIDEO_ENCODERS};
use obs_util::ffi::darray::{darray, darray_free, darray_push_back};
use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};

use super::delay_parity::{
    Capture, LogLine, is_held, live, obs_encoder_packet_create_instance, obs_encoder_packet_release,
};
use super::isolated;

const V: usize = MAX_OUTPUT_VIDEO_ENCODERS;
const A: usize = MAX_OUTPUT_AUDIO_ENCODERS;
const T: i64 = 1_000_000;

// ---------------------------------------------------------------------------
// Tier 2: layouts and values

#[test]
fn values_match_c() {
    let rust: [i64; 9] = [
        OBS_ENCODER_AUDIO.into(),
        OBS_ENCODER_VIDEO.into(),
        KEYFRAME_TRACK_STATUS_NOT_SEEN.into(),
        KEYFRAME_TRACK_STATUS_SEEN.into(),
        KEYFRAME_TRACK_STATUS_SKIPPED.into(),
        LOG_WARNING.into(),
        LOG_DEBUG.into(),
        V as i64,
        A as i64,
    ];
    for (i, (name, value)) in VALUE_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a plain getter; `i` is in range.
        let c = unsafe { oracle_output_interleave_value(i) };
        assert_eq!(value, c, "{name}");
    }
}

#[test]
fn layout_matches_c() {
    type K = keyframe_group_data;
    type C = packet_callback;
    type X = caption_text;
    type D = darray;
    type F = obs_output_interleave_fields;
    type R = obs_output_caption_track_fields;
    type I = obs_encoder_interleave_info;
    let rust = [
        size_of::<K>(),
        align_of::<K>(),
        offset_of!(K, group_id),
        offset_of!(K, pts),
        offset_of!(K, required_tracks),
        offset_of!(K, seen_on_track),
        size_of::<c_int>(),
        size_of::<C>(),
        align_of::<C>(),
        offset_of!(C, packet_cb),
        offset_of!(C, param),
        size_of::<X>(),
        align_of::<X>(),
        offset_of!(X, display_duration),
        offset_of!(X, next),
        offset_of!(X, text),
        size_of::<D>(),
        align_of::<D>(),
        offset_of!(D, array),
        offset_of!(D, num),
        offset_of!(D, capacity),
        size_of::<D>(),
        size_of::<D>(),
        size_of::<D>(),
        size_of::<D>(),
        size_of::<F>(),
        align_of::<F>(),
        offset_of!(F, interleaved_mutex),
        offset_of!(F, interleaved_packets),
        offset_of!(F, received_video),
        offset_of!(F, received_audio),
        offset_of!(F, video_offsets),
        offset_of!(F, audio_offsets),
        offset_of!(F, highest_audio_ts),
        offset_of!(F, highest_video_ts),
        offset_of!(F, video_encoders),
        offset_of!(F, audio_encoders),
        offset_of!(F, encoder_packet_times),
        offset_of!(F, keyframe_group_tracking),
        offset_of!(F, pkt_callbacks_mutex),
        offset_of!(F, pkt_callbacks),
        offset_of!(F, total_frames),
        offset_of!(F, interleaver_max_batch_size),
        offset_of!(F, active_delay_ns),
        offset_of!(F, encoded_packet),
        offset_of!(F, data),
        offset_of!(F, name),
        size_of::<R>(),
        align_of::<R>(),
        offset_of!(R, caption_mutex),
        offset_of!(R, caption_head),
        offset_of!(R, caption_timestamp),
        offset_of!(R, last_caption_timestamp),
        offset_of!(R, caption_data_size),
        size_of::<I>(),
        align_of::<I>(),
        offset_of!(I, timebase_num),
        offset_of!(I, timebase_den),
        offset_of!(I, framesize),
        offset_of!(I, group),
        size_of::<encoder_packet>(),
        align_of::<encoder_packet>(),
        size_of::<encoder_packet_time>(),
        align_of::<encoder_packet_time>(),
        size_of::<c_int>(),
        size_of::<usize>(),
        size_of::<bool>(),
    ];
    assert_eq!(rust.len(), LAYOUT_NAMES.len());
    for (i, (name, value)) in LAYOUT_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a plain getter; `i` is in range.
        let c = unsafe { oracle_output_interleave_layout(i) };
        assert_eq!(value, c, "{name}");
    }
}

// ---------------------------------------------------------------------------
// the test output and the stand-ins both sides call

/// A packet's fields and data, its encoder as an index into the fixture's
/// encoders, and whether the data is a held copy.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PView {
    bytes: Vec<u8>,
    held_copy: bool,
    size: usize,
    pts: i64,
    dts: i64,
    timebase: (i32, i32),
    kind: c_int,
    keyframe: bool,
    dts_usec: i64,
    sys_dts_usec: i64,
    priority: (c_int, c_int),
    track_idx: usize,
    encoder: Option<usize>,
}

/// One keyframe group entry, its group as an index.
#[derive(Debug, Clone, PartialEq, Eq)]
struct GView {
    group: Option<usize>,
    pts: i64,
    required_tracks: u32,
    seen_on_track: [c_int; V],
}

/// What a stand-in recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Event {
    Encoded {
        locked: bool,
        callbacks_locked: bool,
        packet: PView,
    },
    Callback {
        idx: usize,
        locked: bool,
        callbacks_locked: bool,
        packet: PView,
        time: Option<encoder_packet_time>,
    },
    AddCaption {
        locked: bool,
        caption_locked: bool,
        packet: PView,
    },
    NumStarted {
        group: usize,
        locked: bool,
    },
}

/// `struct caption_text` with room for its text.
#[repr(C)]
struct CaptionHead {
    display_duration: f64,
    next: *mut c_void,
    text: [u8; 8],
}

/// `struct caption_track_data`, as far as the accessor reads it.
struct CaptionTrack {
    mutex: *mut c_void,
    head: *mut caption_text,
    timestamp: f64,
    last_timestamp: f64,
    data_size: usize,
    storage: Box<CaptionHead>,
}

/// An encoder: what `obs_encoder_interleave_info` returns, and its name.
struct Encoder {
    info: obs_encoder_interleave_info,
    name: CString,
}

/// An encoder group: its started count.
struct Group {
    owner: *mut Fixture,
    num_started: u32,
}

/// The encoders of a run.
#[derive(Debug, Clone, Copy)]
struct Setup {
    videos: usize,
    audios: usize,
    video_timebase: (u32, u32),
    audio_den: u32,
    framesize: usize,
}

impl Setup {
    /// The C test's: video 1/30, audio 1/48000 with 1024-sample frames.
    fn c_test(videos: usize, audios: usize) -> Self {
        Self {
            videos,
            audios,
            video_timebase: (1, 30),
            audio_den: 48000,
            framesize: 1024,
        }
    }
}

/// The fields of `struct obs_output` the interleaver reads, its encoders,
/// the stub results and the recorded calls.
struct Fixture {
    mutex: *mut c_void,
    callbacks_mutex: *mut c_void,
    queue: darray,
    received_video: [bool; V],
    received_audio: bool,
    video_offsets: [i64; V],
    audio_offsets: [i64; A],
    highest_audio_ts: i64,
    highest_video_ts: [i64; V],
    video_encoders: [*mut c_void; V],
    audio_encoders: [*mut c_void; A],
    times: [darray; V],
    groups: darray,
    callbacks: darray,
    total_frames: c_int,
    max_batch: usize,
    active_delay_ns: u64,
    encoded_packet: encoded_packet_t,
    data: *mut c_void,
    name: *mut c_char,
    captions: Vec<CaptionTrack>,
    /// Video encoders, then audio encoders.
    /// Never grown after the fixture is made, so their addresses hold.
    encoders: Vec<Encoder>,
    group: Box<Group>,
    active: bool,
    add_caption_result: bool,
    add_caption_consumes: bool,
    events: Vec<Event>,
}

const ZEROED_DARRAY: darray = darray {
    array: ptr::null_mut(),
    num: 0,
    capacity: 0,
};

static NAME: &std::ffi::CStr = c"interleaved output";

fn mutex() -> *mut c_void {
    // SAFETY: creates a mutex for this output alone.
    let m = unsafe { oracle_output_delay_mutex_create() };
    assert!(!m.is_null());
    m
}

fn held(m: *mut c_void) -> bool {
    // SAFETY: one of the fixture's mutexes.
    unsafe { oracle_output_delay_mutex_held(m) }
}

impl Fixture {
    /// The C test's `setup()`, with the run's encoders.
    fn new(setup: Setup) -> Box<Self> {
        let mut encoders = Vec::new();
        for i in 0..setup.videos {
            encoders.push(Encoder {
                info: obs_encoder_interleave_info {
                    timebase_num: setup.video_timebase.0,
                    timebase_den: setup.video_timebase.1,
                    framesize: 0,
                    group: ptr::null_mut(),
                },
                name: CString::new(format!("video {i}")).unwrap(),
            });
        }
        for i in 0..setup.audios {
            encoders.push(Encoder {
                info: obs_encoder_interleave_info {
                    timebase_num: 1,
                    timebase_den: setup.audio_den,
                    framesize: setup.framesize,
                    group: ptr::null_mut(),
                },
                name: CString::new(format!("audio {i}")).unwrap(),
            });
        }
        let captions = (0..V)
            .map(|_| CaptionTrack {
                mutex: mutex(),
                head: ptr::null_mut(),
                timestamp: 0.0,
                last_timestamp: 0.0,
                data_size: 0,
                storage: Box::new(CaptionHead {
                    display_duration: 0.0,
                    next: ptr::null_mut(),
                    text: *b"caption\0",
                }),
            })
            .collect();
        let mut f = Box::new(Self {
            mutex: mutex(),
            callbacks_mutex: mutex(),
            queue: ZEROED_DARRAY,
            received_video: [false; V],
            received_audio: false,
            video_offsets: [0; V],
            audio_offsets: [0; A],
            highest_audio_ts: 0,
            highest_video_ts: [i64::MIN; V],
            video_encoders: [ptr::null_mut(); V],
            audio_encoders: [ptr::null_mut(); A],
            times: [ZEROED_DARRAY; V],
            groups: ZEROED_DARRAY,
            callbacks: ZEROED_DARRAY,
            total_frames: 0,
            max_batch: 8,
            active_delay_ns: 0,
            encoded_packet: Some(encoded_packet),
            data: ptr::null_mut(),
            name: NAME.as_ptr().cast_mut(),
            captions,
            encoders,
            group: Box::new(Group {
                owner: ptr::null_mut(),
                num_started: 2,
            }),
            active: true,
            add_caption_result: true,
            add_caption_consumes: false,
            events: Vec::new(),
        });
        let me: *mut Fixture = &raw mut *f;
        f.data = me.cast();
        f.group.owner = me;
        for i in 0..setup.videos {
            f.video_encoders[i] = (&raw mut f.encoders[i]).cast();
        }
        for i in 0..setup.audios {
            f.audio_encoders[i] = (&raw mut f.encoders[setup.videos + i]).cast();
        }
        f.set_callbacks(1);
        f
    }

    fn ptr(&mut self) -> *mut c_void {
        ptr::from_mut(self).cast()
    }

    fn set_callbacks(&mut self, n: usize) {
        // SAFETY: the fixture's darray of packet callbacks.
        unsafe {
            darray_free(&raw mut self.callbacks);
            for i in 0..n {
                let cb = packet_callback {
                    packet_cb: Some(packet_cb),
                    param: i as *mut c_void,
                };
                darray_push_back(
                    size_of::<packet_callback>(),
                    &raw mut self.callbacks,
                    (&raw const cb).cast(),
                );
            }
        }
    }

    fn set_group(&mut self, members: [bool; 2], num_started: u32) {
        let group: *mut c_void = (&raw mut *self.group).cast();
        for (i, member) in members.iter().enumerate() {
            if let Some(e) = self.encoder_mut(true, i) {
                e.info.group = if *member { group } else { ptr::null_mut() };
            }
        }
        self.group.num_started = num_started;
    }

    fn encoder_mut(&mut self, video: bool, track: usize) -> Option<&mut Encoder> {
        let p = if video {
            self.video_encoders[track]
        } else {
            self.audio_encoders[track]
        };
        // SAFETY: one of the fixture's boxed encoders, or null.
        unsafe { p.cast::<Encoder>().as_mut() }
    }

    fn encoder_index(&self, p: *mut c_void) -> Option<usize> {
        if p.is_null() {
            return None;
        }
        Some(
            self.encoders
                .iter()
                .position(|e| ptr::eq(e, p.cast::<Encoder>()))
                .unwrap_or(usize::MAX),
        )
    }

    fn view(&self, p: &encoder_packet) -> PView {
        let bytes = if p.data.is_null() || p.size == 0 {
            Vec::new()
        } else {
            // SAFETY: a packet's data is valid for `size` bytes.
            unsafe { std::slice::from_raw_parts(p.data, p.size) }.to_vec()
        };
        PView {
            bytes,
            ..self.view_fields(p)
        }
    }

    /// [`Fixture::view`] without reading the data.
    fn view_fields(&self, p: &encoder_packet) -> PView {
        PView {
            bytes: Vec::new(),
            held_copy: is_held(p.data),
            size: p.size,
            pts: p.pts,
            dts: p.dts,
            timebase: (p.timebase_num, p.timebase_den),
            kind: p.r#type,
            keyframe: p.keyframe,
            dts_usec: p.dts_usec,
            sys_dts_usec: p.sys_dts_usec,
            priority: (p.priority, p.drop_priority),
            track_idx: p.track_idx,
            encoder: self.encoder_index(p.encoder),
        }
    }

    fn items<X: Copy>(d: &darray) -> Vec<X> {
        if d.num == 0 {
            return Vec::new();
        }
        // SAFETY: a darray of `X`.
        unsafe { std::slice::from_raw_parts(d.array.cast::<X>(), d.num) }.to_vec()
    }

    fn packets(&self) -> Vec<encoder_packet> {
        Self::items(&self.queue)
    }

    fn snapshot(
        &mut self,
        ret: Ret,
        caller: Option<PView>,
        live_delta: i64,
        log: LogLine,
    ) -> Snapshot {
        let group: *mut c_void = (&raw mut *self.group).cast();
        Snapshot {
            ret,
            events: std::mem::take(&mut self.events),
            queue: self.packets().iter().map(|p| self.view(p)).collect(),
            queue_capacity: self.queue.capacity,
            times: self
                .times
                .iter()
                .map(|d| (Self::items::<encoder_packet_time>(d), d.capacity))
                .collect(),
            groups: Self::items::<keyframe_group_data>(&self.groups)
                .iter()
                .map(|g| GView {
                    group: (g.group_id != 0).then_some(if g.group_id == group as usize {
                        0
                    } else {
                        usize::MAX
                    }),
                    pts: g.pts,
                    required_tracks: g.required_tracks,
                    seen_on_track: g.seen_on_track,
                })
                .collect(),
            groups_capacity: self.groups.capacity,
            fields: Fields {
                received_video: self.received_video,
                received_audio: self.received_audio,
                video_offsets: self.video_offsets,
                audio_offsets: self.audio_offsets,
                highest_audio_ts: self.highest_audio_ts,
                highest_video_ts: self.highest_video_ts,
                total_frames: self.total_frames,
            },
            captions: self
                .captions
                .iter()
                .map(|c| {
                    (
                        c.timestamp.to_bits(),
                        c.last_timestamp.to_bits(),
                        c.data_size,
                        c.head.is_null(),
                    )
                })
                .collect(),
            caller,
            live_delta,
            log,
        }
    }

    /// What `reset_packet_data()` in `obs-output.c` does before it calls
    /// `free_packets` (which the step then calls on its side).
    fn reset_fields(&mut self) {
        self.received_audio = false;
        self.highest_audio_ts = 0;
        for t in &mut self.times {
            t.num = 0;
        }
        self.received_video = [false; V];
        self.video_offsets = [0; V];
        self.highest_video_ts = [i64::MIN; V];
        self.audio_offsets = [0; A];
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // The C test's teardown, without its leak check (`live_delta` per
        // step is the check): release what is queued, free the arrays.
        for mut p in self.packets() {
            // SAFETY: a held copy.
            unsafe { obs_encoder_packet_release(&raw mut p) };
        }
        // SAFETY: the fixture's darrays and mutexes.
        unsafe {
            darray_free(&raw mut self.queue);
            for t in &mut self.times {
                darray_free(t);
            }
            darray_free(&raw mut self.groups);
            darray_free(&raw mut self.callbacks);
            for c in &self.captions {
                oracle_output_delay_mutex_destroy(c.mutex);
            }
            oracle_output_delay_mutex_destroy(self.mutex);
            oracle_output_delay_mutex_destroy(self.callbacks_mutex);
        }
    }
}

unsafe extern "C" {
    fn os_gettime_ns() -> u64;
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Fields {
    received_video: [bool; V],
    received_audio: bool,
    video_offsets: [i64; V],
    audio_offsets: [i64; A],
    highest_audio_ts: i64,
    highest_video_ts: [i64; V],
    total_frames: c_int,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ret {
    None,
    Bool(bool),
    Size(usize),
    Int(c_int),
}

#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    ret: Ret,
    events: Vec<Event>,
    queue: Vec<PView>,
    queue_capacity: usize,
    times: Vec<(Vec<encoder_packet_time>, usize)>,
    groups: Vec<GView>,
    groups_capacity: usize,
    fields: Fields,
    /// Per track: the caption time, the last raw caption time (as bits),
    /// the raw data size, and whether there is no head.
    captions: Vec<(u64, u64, usize, bool)>,
    /// The caller's packet after the call.
    caller: Option<PView>,
    live_delta: i64,
    log: LogLine,
}

thread_local! {
    /// The clock both sides read.
    static CLOCK: Cell<u64> = const { Cell::new(0) };
}

fn tick() -> u64 {
    CLOCK.with(|c| {
        let now = c.get();
        c.set(now + 1);
        now
    })
}

/// The C file's `os_gettime_ns`.
#[unsafe(no_mangle)]
pub extern "C" fn oracle_output_interleave_clock_ns() -> u64 {
    tick()
}

/// # Safety
///
/// `output` is a live [`Fixture`].
unsafe fn fixture<'a>(output: *const c_void) -> &'a mut Fixture {
    // SAFETY: caller contract; the call is the only one using it.
    unsafe { &mut *output.cast::<Fixture>().cast_mut() }
}

/// libobs's `obs_output_interleave_fields`, over a [`Fixture`].
///
/// # Safety
///
/// `output` is a live [`Fixture`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_interleave_fields(
    output: *const c_void,
) -> obs_output_interleave_fields {
    let o = output.cast::<Fixture>().cast_mut();
    // SAFETY: field addresses of a live fixture.
    unsafe {
        obs_output_interleave_fields {
            interleaved_mutex: (*o).mutex,
            interleaved_packets: &raw mut (*o).queue,
            received_video: (&raw mut (*o).received_video).cast(),
            received_audio: &raw mut (*o).received_audio,
            video_offsets: (&raw mut (*o).video_offsets).cast(),
            audio_offsets: (&raw mut (*o).audio_offsets).cast(),
            highest_audio_ts: &raw mut (*o).highest_audio_ts,
            highest_video_ts: (&raw mut (*o).highest_video_ts).cast(),
            video_encoders: (&raw const (*o).video_encoders).cast(),
            audio_encoders: (&raw const (*o).audio_encoders).cast(),
            encoder_packet_times: (&raw mut (*o).times).cast(),
            keyframe_group_tracking: &raw mut (*o).groups,
            pkt_callbacks_mutex: (*o).callbacks_mutex,
            pkt_callbacks: &raw mut (*o).callbacks,
            total_frames: &raw mut (*o).total_frames,
            interleaver_max_batch_size: &raw const (*o).max_batch,
            active_delay_ns: &raw const (*o).active_delay_ns,
            encoded_packet: &raw const (*o).encoded_packet,
            data: &raw const (*o).data,
            name: &raw const (*o).name,
        }
    }
}

/// libobs's `obs_output_caption_track_fields`, over a [`Fixture`].
///
/// # Safety
///
/// `output` is a live [`Fixture`] and `track_idx` a video track.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_caption_track_fields(
    output: *mut c_void,
    track_idx: usize,
) -> obs_output_caption_track_fields {
    let o = output.cast::<Fixture>();
    // SAFETY: field addresses of a live fixture's caption track.
    unsafe {
        let c = (&raw mut (*o).captions)
            .as_mut()
            .unwrap()
            .as_mut_ptr()
            .add(track_idx);
        obs_output_caption_track_fields {
            caption_mutex: (*c).mutex,
            caption_head: &raw const (*c).head,
            caption_timestamp: &raw mut (*c).timestamp,
            last_caption_timestamp: &raw mut (*c).last_timestamp,
            caption_data_size: &raw const (*c).data_size,
        }
    }
}

/// `active()` of `obs-output.c`.
///
/// # Safety
///
/// `output` is a live [`Fixture`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_interleave_active(output: *const c_void) -> bool {
    // SAFETY: caller contract.
    unsafe { fixture(output) }.active
}

/// A copy of `get_first_audio_encoder_index()`.
///
/// # Safety
///
/// `output` is a live [`Fixture`], `index` null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_interleave_first_audio_encoder_index(
    output: *const c_void,
    index: *mut usize,
) -> bool {
    // SAFETY: caller contract.
    unsafe {
        if index.is_null() {
            return false;
        }
        match fixture(output)
            .audio_encoders
            .iter()
            .position(|e| !e.is_null())
        {
            Some(i) => {
                *index = i;
                true
            }
            None => false,
        }
    }
}

/// A copy of `get_first_video_encoder_index()`.
///
/// # Safety
///
/// As [`obs_output_interleave_first_audio_encoder_index`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_interleave_first_video_encoder_index(
    output: *const c_void,
    index: *mut usize,
) -> bool {
    // SAFETY: caller contract.
    unsafe {
        if index.is_null() {
            return false;
        }
        match fixture(output)
            .video_encoders
            .iter()
            .position(|e| !e.is_null())
        {
            Some(i) => {
                *index = i;
                true
            }
            None => false,
        }
    }
}

/// `add_caption()`, recorded. It changes the packet's priority (as the
/// real one rewrites its data) and may take the raw caption data.
///
/// # Safety
///
/// `output` is a live [`Fixture`] and `out` a valid packet.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_interleave_add_caption(
    output: *mut c_void,
    out: *mut encoder_packet,
) -> bool {
    // SAFETY: caller contract.
    unsafe {
        let o = fixture(output);
        let track = (*out).track_idx;
        let event = Event::AddCaption {
            locked: held(o.mutex),
            caption_locked: held(o.captions[track].mutex),
            packet: o.view(&*out),
        };
        o.events.push(event);
        (*out).priority += 1;
        if o.add_caption_consumes {
            o.captions[track].data_size = 0;
        }
        o.add_caption_result
    }
}

/// `obs_encoder_interleave_info`, over a fixture's encoder.
///
/// # Safety
///
/// `encoder` is one of a live fixture's encoders.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_interleave_info(
    encoder: *const c_void,
) -> obs_encoder_interleave_info {
    // SAFETY: caller contract.
    unsafe { (*encoder.cast::<Encoder>()).info }
}

/// `obs_encoder_group_num_started`, recorded.
///
/// # Safety
///
/// `group` is a live fixture's group.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_group_num_started(group: *mut c_void) -> u32 {
    // SAFETY: caller contract; the owner is the fixture the step runs on.
    unsafe {
        let g = &*group.cast::<Group>();
        let o = &mut *g.owner;
        let locked = held(o.mutex);
        o.events.push(Event::NumStarted { group: 0, locked });
        g.num_started
    }
}

/// `obs_output_get_name`.
///
/// # Safety
///
/// `output` is a live [`Fixture`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_get_name(output: *const c_void) -> *const c_char {
    // SAFETY: caller contract.
    unsafe { fixture(output) }.name
}

/// `obs_encoder_get_name`.
///
/// # Safety
///
/// `encoder` is one of a live fixture's encoders.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_get_name(encoder: *const c_void) -> *const c_char {
    // SAFETY: caller contract.
    unsafe { (*encoder.cast::<Encoder>()).name.as_ptr() }
}

/// The output's `encoded_packet()`: it does not own the packet.
unsafe extern "C" fn encoded_packet(data: *mut c_void, packet: *mut encoder_packet) {
    // SAFETY: the fixture the interleaver runs on, and the packet it sends.
    unsafe {
        let o = fixture(data);
        let event = Event::Encoded {
            locked: held(o.mutex),
            callbacks_locked: held(o.callbacks_mutex),
            packet: o.view(&*packet),
        };
        o.events.push(event);
    }
}

/// A packet callback: records, then changes the packet and its time, as a
/// callback may.
unsafe extern "C" fn packet_cb(
    output: *mut c_void,
    packet: *mut encoder_packet,
    time: *mut encoder_packet_time,
    param: *mut c_void,
) {
    // SAFETY: the fixture the interleaver runs on, the packet it sends and
    // its packet time (or null).
    unsafe {
        let o = fixture(output);
        let event = Event::Callback {
            idx: param as usize,
            locked: held(o.mutex),
            callbacks_locked: held(o.callbacks_mutex),
            packet: o.view(&*packet),
            time: time.as_ref().copied(),
        };
        o.events.push(event);
        (*packet).sys_dts_usec += 1;
        if let Some(t) = time.as_mut() {
            t.fer += 1;
        }
    }
}

// ---------------------------------------------------------------------------
// the steps

#[derive(Debug, Clone, Copy)]
enum Static {
    StartIdx,
    PrunePremature,
    Prune,
    Initialize,
    Resort,
    Discard(i64),
    Send,
    CheckGroup {
        track: usize,
        pts: i64,
        keyframe: bool,
    },
    CountStreamable,
    ApplyEptOffsets,
}

#[derive(Debug, Clone)]
enum Op {
    /// A packet of track `track % count`; the track's dts and dts_usec move
    /// by the steps.
    Packet {
        video: bool,
        track: usize,
        usec_step: i64,
        dts_step: i64,
        pts_extra: i64,
        keyframe: bool,
        with_time: bool,
        size: usize,
    },
    /// A packet with these exact timestamps (the fixed scenarios).
    Exact {
        video: bool,
        track: usize,
        dts: i64,
        pts: i64,
        dts_usec: i64,
        keyframe: bool,
        with_time: bool,
    },
    SetActive(bool),
    SetDelay(bool),
    SetBatch(usize),
    Reset,
    FreePackets,
    Caption {
        track: usize,
        head: Option<f64>,
        timestamp: f64,
        last: f64,
        data_size: usize,
    },
    AddCaption {
        result: bool,
        consume: bool,
    },
    Callbacks(usize),
    Group {
        members: [bool; 2],
        num_started: u32,
    },
    EncoderIndex {
        video: bool,
        track: usize,
    },
    Static(Static),
}

/// The running timestamps of each track.
struct Clocks {
    video: [(i64, i64); 2],
    audio: [(i64, i64); 2],
    tag: u8,
    cts: u64,
}

impl Clocks {
    fn new() -> Self {
        Self {
            video: [(30, T); 2],
            audio: [(48000, T); 2],
            tag: 0,
            cts: 0,
        }
    }
}

/// The source packet of a step, built for each side with its own encoder.
#[derive(Debug, Clone)]
struct Src {
    video: bool,
    track: usize,
    dts: i64,
    pts: i64,
    dts_usec: i64,
    keyframe: bool,
    bytes: Vec<u8>,
    timebase: (i32, i32),
    time: Option<encoder_packet_time>,
}

fn has_audio_at_or_after_start(f: &Fixture) -> bool {
    let packets = f.packets();
    let video0 = packets
        .iter()
        .any(|p| p.r#type == OBS_ENCODER_VIDEO && p.track_idx == 0);
    video0 && packets.iter().any(|p| p.r#type == OBS_ENCODER_AUDIO)
}

/// The step on one side: `c` picks the oracle or the shim. Returns the
/// return value and the caller's packet after the call.
#[expect(
    clippy::too_many_lines,
    reason = "parity table: one block per ported C case"
)]
fn run_side(f: &mut Fixture, c: bool, op: &Op, src: Option<&Src>) -> (Ret, Option<PView>) {
    let o = f.ptr();
    match op {
        Op::Packet { .. } | Op::Exact { .. } => {
            let src = src.expect("a packet step has a source");
            let encoder = if src.video {
                f.video_encoders[src.track]
            } else {
                f.audio_encoders[src.track]
            };
            let mut bytes = src.bytes.clone();
            let mut pkt = encoder_packet {
                data: bytes.as_mut_ptr(),
                size: bytes.len(),
                pts: src.pts,
                dts: src.dts,
                timebase_num: src.timebase.0,
                timebase_den: src.timebase.1,
                r#type: if src.video {
                    OBS_ENCODER_VIDEO
                } else {
                    OBS_ENCODER_AUDIO
                },
                keyframe: src.keyframe,
                dts_usec: src.dts_usec,
                sys_dts_usec: src.dts_usec,
                priority: 0,
                drop_priority: 0,
                track_idx: 99,
                encoder,
            };
            if f.active_delay_ns != 0 {
                // the delay hands the callback a held copy it owns
                let mut held = encoder_packet::default();
                // SAFETY: a valid packet and a writable copy.
                unsafe { obs_encoder_packet_create_instance(&raw mut held, &raw const pkt) };
                pkt = held;
            }
            let mut time = src.time;
            let time_ptr = time.as_mut().map_or(ptr::null_mut(), ptr::from_mut);
            // SAFETY: a live fixture, a valid packet of one of its encoders
            // and its time.
            unsafe {
                if c {
                    oracle_interleave_packets(o, (&raw mut pkt).cast(), time_ptr.cast());
                } else {
                    interleave_packets_with_clock(o, &raw mut pkt, time_ptr, tick);
                }
            }
            // With a delay the caller's packet shares its data with the
            // queued one, which may already be sent and released: its bytes
            // are read only while they are still held.
            let mut view = f.view_fields(&pkt);
            if f.active_delay_ns == 0 || is_held(pkt.data) {
                view = f.view(&pkt);
            }
            if f.active_delay_ns != 0 && !f.active {
                // Characterized, not endorsed: not released; the delay's
                // caller would leak it, the test releases it.
                // SAFETY: the held copy made above.
                unsafe { obs_encoder_packet_release(&raw mut pkt) };
            }
            drop(bytes);
            (Ret::None, Some(view))
        }
        Op::SetActive(a) => {
            f.active = *a;
            (Ret::None, None)
        }
        Op::SetDelay(d) => {
            f.active_delay_ns = u64::from(*d);
            (Ret::None, None)
        }
        Op::SetBatch(n) => {
            f.max_batch = *n;
            (Ret::None, None)
        }
        Op::Reset | Op::FreePackets => {
            if matches!(op, Op::Reset) {
                f.reset_fields();
            }
            // SAFETY: a live fixture.
            unsafe {
                if c {
                    oracle_free_packets(o);
                } else {
                    free_packets(o);
                }
            }
            (Ret::None, None)
        }
        Op::Caption {
            track,
            head,
            timestamp,
            last,
            data_size,
        } => {
            let ct = &mut f.captions[*track];
            match head {
                Some(d) => {
                    ct.storage.display_duration = *d;
                    ct.head = (&raw mut *ct.storage).cast();
                }
                None => ct.head = ptr::null_mut(),
            }
            ct.timestamp = *timestamp;
            ct.last_timestamp = *last;
            ct.data_size = *data_size;
            (Ret::None, None)
        }
        Op::AddCaption { result, consume } => {
            f.add_caption_result = *result;
            f.add_caption_consumes = *consume;
            (Ret::None, None)
        }
        Op::Callbacks(n) => {
            f.set_callbacks(*n);
            (Ret::None, None)
        }
        Op::Group {
            members,
            num_started,
        } => {
            f.set_group(*members, *num_started);
            (Ret::None, None)
        }
        Op::EncoderIndex { video, track } => {
            let mut pkt = encoder_packet {
                r#type: if *video {
                    OBS_ENCODER_VIDEO
                } else {
                    OBS_ENCODER_AUDIO
                },
                encoder: if *video {
                    f.video_encoders[*track]
                } else {
                    f.audio_encoders[*track]
                },
                ..encoder_packet::default()
            };
            // SAFETY: a live fixture and a packet of one of its encoders.
            let idx = unsafe {
                if c {
                    oracle_get_encoder_index(o, (&raw mut pkt).cast())
                } else {
                    get_encoder_index(o, &raw mut pkt)
                }
            };
            (Ret::Size(idx), None)
        }
        Op::Static(s) => run_static(f, c, *s),
    }
}

fn run_static(f: &mut Fixture, c: bool, s: Static) -> (Ret, Option<PView>) {
    let o = f.ptr();
    // SAFETY: a live fixture; the callers check each function's C
    // precondition first.
    unsafe {
        match s {
            Static::StartIdx => (
                Ret::Size(if c {
                    oracle_output_interleave_start_idx(o)
                } else {
                    statics::get_interleaved_start_idx(o)
                }),
                None,
            ),
            Static::PrunePremature => (
                Ret::Int(if c {
                    oracle_output_interleave_prune_premature(o)
                } else {
                    statics::prune_premature_packets(o)
                }),
                None,
            ),
            Static::Prune => (
                Ret::Bool(if c {
                    oracle_output_interleave_prune(o)
                } else {
                    statics::prune_interleaved_packets(o)
                }),
                None,
            ),
            Static::Initialize => (
                Ret::Bool(if c {
                    oracle_output_interleave_initialize(o)
                } else {
                    statics::initialize_interleaved_packets(o)
                }),
                None,
            ),
            Static::Resort => {
                if c {
                    oracle_output_interleave_resort(o);
                } else {
                    statics::resort_interleaved_packets(o);
                }
                (Ret::None, None)
            }
            Static::Discard(dts_usec) => {
                if c {
                    oracle_output_interleave_discard_unused_audio(o, dts_usec);
                } else {
                    statics::discard_unused_audio_packets(o, dts_usec);
                }
                (Ret::None, None)
            }
            Static::Send => {
                if c {
                    oracle_output_interleave_send(o);
                } else {
                    statics::send_interleaved(o, tick);
                }
                (Ret::None, None)
            }
            Static::CheckGroup {
                track,
                pts,
                keyframe,
            } => {
                let mut pkt = encoder_packet {
                    r#type: OBS_ENCODER_VIDEO,
                    keyframe,
                    pts,
                    dts: pts,
                    track_idx: track,
                    encoder: f.video_encoders[track],
                    ..encoder_packet::default()
                };
                if c {
                    oracle_output_interleave_check_group_keyframe(o, (&raw mut pkt).cast());
                } else {
                    statics::check_encoder_group_keyframe_alignment(o, &raw const pkt);
                }
                (Ret::None, None)
            }
            Static::CountStreamable => (
                Ret::Size(if c {
                    oracle_output_interleave_count_streamable(o)
                } else {
                    statics::count_streamable_frames(o)
                }),
                None,
            ),
            Static::ApplyEptOffsets => {
                if c {
                    oracle_output_interleave_apply_ept_offsets(o);
                } else {
                    statics::apply_ept_offsets(o);
                }
                (Ret::None, None)
            }
        }
    }
}

/// Builds the step's source packet from the running timestamps, and
/// clamps the step's tracks to the run's encoders. `None` for a step whose
/// C precondition does not hold (it is skipped on both sides).
#[expect(
    clippy::too_many_lines,
    reason = "parity table: one block per ported C case"
)]
fn prepare(op: &Op, setup: Setup, clocks: &mut Clocks, f: &Fixture) -> Option<(Op, Option<Src>)> {
    let video_tb = (
        i32::try_from(setup.video_timebase.0).unwrap(),
        i32::try_from(setup.video_timebase.1).unwrap(),
    );
    let audio_tb = (1, i32::try_from(setup.audio_den).unwrap());
    match *op {
        Op::Packet {
            video,
            track,
            usec_step,
            dts_step,
            pts_extra,
            keyframe,
            with_time,
            size,
        } => {
            let track = track % if video { setup.videos } else { setup.audios };
            let (dts, usec) = if video {
                &mut clocks.video[track]
            } else {
                &mut clocks.audio[track]
            };
            *dts += dts_step;
            *usec += usec_step;
            let (dts, dts_usec) = (*dts, *usec);
            clocks.tag = clocks.tag.wrapping_add(1);
            clocks.cts += 1;
            let pts = if video { dts + pts_extra } else { dts };
            let src = Src {
                video,
                track,
                dts,
                pts,
                dts_usec,
                keyframe: video && keyframe,
                bytes: (0..size)
                    .map(|i| clocks.tag.wrapping_add(i as u8))
                    .collect(),
                timebase: if video { video_tb } else { audio_tb },
                time: (video && with_time).then_some(encoder_packet_time {
                    pts,
                    cts: clocks.cts,
                    fer: clocks.cts * 2,
                    ferc: clocks.cts * 3,
                    pir: 0,
                }),
            };
            Some((op.clone(), Some(src)))
        }
        Op::Exact {
            video,
            track,
            dts,
            pts,
            dts_usec,
            keyframe,
            with_time,
        } => {
            clocks.tag = clocks.tag.wrapping_add(1);
            clocks.cts += 1;
            let src = Src {
                video,
                track,
                dts,
                pts,
                dts_usec,
                keyframe,
                bytes: vec![clocks.tag],
                timebase: if video { video_tb } else { audio_tb },
                time: with_time.then_some(encoder_packet_time {
                    pts,
                    cts: clocks.cts,
                    ..encoder_packet_time::default()
                }),
            };
            Some((op.clone(), Some(src)))
        }
        Op::EncoderIndex { video, track } => Some((
            Op::EncoderIndex {
                video,
                track: track % if video { setup.videos } else { setup.audios },
            },
            None,
        )),
        Op::Caption {
            track,
            head,
            timestamp,
            last,
            data_size,
        } => Some((
            Op::Caption {
                track: track % setup.videos,
                head,
                timestamp,
                last,
                data_size,
            },
            None,
        )),
        Op::Static(Static::StartIdx) => has_audio_at_or_after_start(f).then(|| (op.clone(), None)),
        Op::Static(Static::Send) => (f.queue.num != 0).then(|| (op.clone(), None)),
        Op::Static(Static::CheckGroup {
            track,
            pts,
            keyframe,
        }) => Some((
            Op::Static(Static::CheckGroup {
                track: track % setup.videos,
                pts,
                keyframe,
            }),
            None,
        )),
        _ => Some((op.clone(), None)),
    }
}

/// Runs `ops` on a C output and a Rust output and compares them after
/// every step. With `log`, the log lines are compared too.
fn run_sequence(setup: Setup, ops: &[Op], mut log: Option<&mut Capture>) {
    let mut c = Fixture::new(setup);
    let mut r = Fixture::new(setup);
    let mut clocks = Clocks::new();
    for (i, op) in ops.iter().enumerate() {
        let Some((op, src)) = prepare(op, setup, &mut clocks, &c) else {
            continue;
        };
        let base = 1000 * (i as u64 + 1);

        let side = |f: &mut Fixture, is_c: bool, log: &mut Option<&mut Capture>| {
            if let Some(l) = log.as_deref_mut() {
                l.take();
            }
            CLOCK.with(|k| k.set(base));
            let before = live();
            let (ret, caller) = run_side(f, is_c, &op, src.as_ref());
            let delta = live() - before;
            let line = log
                .as_deref_mut()
                .map_or((0, -1, String::new()), Capture::take);
            f.snapshot(ret, caller, delta, line)
        };

        let cs = side(&mut c, true, &mut log);
        let rs = side(&mut r, false, &mut log);
        assert_eq!(cs, rs, "step {i}: {op:?}\nsrc: {src:?}\nops: {ops:?}");
    }
}

// ---------------------------------------------------------------------------
// fixed scenarios: the characterization cases, step by step

fn vid(track: usize, dts: i64, pts: i64, dts_usec: i64, keyframe: bool) -> Op {
    Op::Exact {
        video: true,
        track,
        dts,
        pts,
        dts_usec,
        keyframe,
        with_time: true,
    }
}

fn aud(track: usize, dts: i64, dts_usec: i64) -> Op {
    Op::Exact {
        video: false,
        track,
        dts,
        pts: dts,
        dts_usec,
        keyframe: false,
        with_time: false,
    }
}

const F: i64 = 33333;

#[expect(
    clippy::too_many_lines,
    reason = "parity table: one block per ported C case"
)]
fn scenarios() -> Vec<(Setup, Vec<Op>)> {
    vec![
        // video waits for a keyframe
        (
            Setup::c_test(1, 1),
            vec![
                aud(0, 48000, T - 20000),
                aud(0, 49024, T + 1333),
                vid(0, 30, 30, T, false),
                vid(0, 31, 31, T + F, true),
                vid(0, 32, 32, T + 2 * F, false),
                aud(0, 50048, T + 70000),
                vid(0, 33, 33, T + 3 * F, false),
            ],
        ),
        // the keyframe wait discards other tracks
        (
            Setup::c_test(2, 1),
            vec![
                vid(1, 30, 30, T, true),
                aud(0, 48000, T + 1000),
                vid(0, 31, 31, T + 2000, false),
            ],
        ),
        // late audio drops the first keyframe
        (
            Setup::c_test(1, 1),
            vec![
                vid(0, 30, 30, T, true),
                aud(0, 48000, T + 40000),
                vid(0, 31, 31, T + F, false),
                aud(0, 49024, T + 61333),
                vid(0, 32, 32, T + 2 * F, false),
                vid(0, 60, 60, T + 1_000_000, true),
                aud(0, 96000, T + 1_010_000),
                vid(0, 61, 61, T + 1_000_000 + F, false),
                aud(0, 97024, T + 1_031_333),
            ],
        ),
        // the premature prune can start on a non-keyframe
        (
            Setup::c_test(2, 1),
            vec![
                vid(0, 30, 30, T, true),
                aud(0, 48000, T + 40000),
                vid(0, 31, 31, T + F, false),
                aud(0, 49024, T + 50000),
                vid(0, 32, 32, T + 2 * F, false),
                aud(0, 50048, T + 70000),
                vid(1, 30, 30, T + 2 * F, true),
            ],
        ),
        // the start waits for every track; then offsets and resort
        (
            Setup::c_test(2, 2),
            vec![
                vid(0, 30, 30, T, true),
                vid(1, 30, 30, T, true),
                aud(0, 48000, T + 1000),
                aud(1, 48000, T + 2000),
                vid(0, 31, 31, T + F, false),
                vid(1, 31, 31, T + F, false),
                aud(0, 49024, T + 22333),
                aud(1, 49024, T + 23333),
                aud(0, 50048, T + 43666),
                aud(1, 50048, T + 44666),
            ],
        ),
        // start offsets and resort with a B-frame stream
        (
            Setup::c_test(1, 2),
            vec![
                aud(0, 48000, T - 1000),
                aud(1, 96000, T - 3000),
                vid(0, 29, 30, T, true),
                aud(0, 49024, T + 20333),
                aud(1, 97024, T + 18333),
                vid(0, 30, 31, T + F, false),
            ],
        ),
        // priming audio
        (
            Setup::c_test(1, 1),
            vec![
                aud(0, -1024, T - 21333),
                vid(0, 30, 30, T, true),
                aud(0, 0, T),
                aud(0, 1024, T + 21333),
                vid(0, 31, 31, T + F, false),
            ],
        ),
        (
            Setup::c_test(1, 1),
            vec![
                aud(0, -2048, T - 10000),
                vid(0, 30, 30, T, true),
                aud(0, -1024, T + 11333),
            ],
        ),
        // audio past the first video
        (
            Setup::c_test(1, 1),
            vec![
                aud(0, 48000, T - 10000),
                vid(0, 30, 30, T, true),
                aud(0, 49024, T + 11333),
            ],
        ),
        // the catch-up limit, with several callbacks and captions
        (
            Setup::c_test(1, 1),
            vec![
                Op::Callbacks(3),
                Op::Caption {
                    track: 0,
                    head: Some(2.0),
                    timestamp: 0.0,
                    last: 0.0,
                    data_size: 3,
                },
                Op::SetBatch(0),
                aud(0, 48000, T),
                vid(0, 30, 30, T, true),
                aud(0, 49024, T + 21333),
                aud(0, 50048, T + 42666),
                aud(0, 51072, T + 64000),
                vid(0, 31, 31, T + F, false),
                vid(0, 32, 32, T + 2 * F, false),
                Op::AddCaption {
                    result: false,
                    consume: true,
                },
                vid(0, 33, 33, T + 3 * F, false),
                aud(0, 52096, T + 85333),
                vid(0, 34, 34, T + 4 * F, false),
            ],
        ),
        // a delayed output: packets are the delay's, a waiting one released,
        // an inactive one not
        (
            Setup::c_test(1, 1),
            vec![
                Op::SetDelay(true),
                aud(0, 48000, T),
                vid(0, 30, 30, T + 1000, false),
                vid(0, 31, 31, T + F, true),
                aud(0, 49024, T + 40000),
                Op::SetActive(false),
                vid(0, 32, 32, T + 2 * F, false),
                Op::SetActive(true),
                Op::SetDelay(false),
                vid(0, 33, 33, T + 3 * F, false),
            ],
        ),
        // encoder group keyframes
        (
            Setup::c_test(2, 1),
            vec![
                Op::Group {
                    members: [true, true],
                    num_started: 2,
                },
                Op::Static(Static::CheckGroup {
                    track: 0,
                    pts: 10,
                    keyframe: true,
                }),
                Op::Static(Static::CheckGroup {
                    track: 1,
                    pts: 10,
                    keyframe: true,
                }),
                Op::Static(Static::CheckGroup {
                    track: 0,
                    pts: 20,
                    keyframe: true,
                }),
                Op::Static(Static::CheckGroup {
                    track: 0,
                    pts: 30,
                    keyframe: true,
                }),
                Op::Static(Static::CheckGroup {
                    track: 1,
                    pts: 30,
                    keyframe: true,
                }),
                Op::Static(Static::CheckGroup {
                    track: 0,
                    pts: 50,
                    keyframe: true,
                }),
                Op::Static(Static::CheckGroup {
                    track: 0,
                    pts: 40,
                    keyframe: true,
                }),
                Op::Static(Static::CheckGroup {
                    track: 0,
                    pts: 40,
                    keyframe: true,
                }),
                Op::Static(Static::CheckGroup {
                    track: 1,
                    pts: 60,
                    keyframe: false,
                }),
                Op::Static(Static::CheckGroup {
                    track: 1,
                    pts: 60,
                    keyframe: true,
                }),
            ],
        ),
        // restart: reset_packet_data and free_packets, then a new start
        (
            Setup::c_test(1, 1),
            vec![
                aud(0, 48000, T),
                vid(0, 30, 30, T, true),
                aud(0, 49024, T + 21333),
                Op::Reset,
                aud(0, 96000, T + 2_000_000),
                vid(0, 90, 90, T + 2_000_000, true),
                Op::FreePackets,
                Op::EncoderIndex {
                    video: true,
                    track: 0,
                },
                Op::EncoderIndex {
                    video: false,
                    track: 0,
                },
            ],
        ),
    ]
}

#[test]
fn parity_fixed_sequences() {
    if !isolated(concat!(module_path!(), "::parity_fixed_sequences")) {
        return;
    }
    let mut log = Capture::arm();
    for (setup, ops) in scenarios() {
        run_sequence(setup, &ops, Some(&mut log));
    }
}

fn static_call() -> impl Strategy<Value = Static> {
    prop_oneof![
        Just(Static::StartIdx),
        Just(Static::PrunePremature),
        Just(Static::Prune),
        Just(Static::Initialize),
        Just(Static::Resort),
        (-50_000_i64..2_000_000).prop_map(Static::Discard),
        Just(Static::Send),
        (0_usize..2, -2_i64..60, any::<bool>()).prop_map(|(track, pts, keyframe)| {
            Static::CheckGroup {
                track,
                pts,
                keyframe,
            }
        }),
        Just(Static::CountStreamable),
        Just(Static::ApplyEptOffsets),
    ]
}

fn op() -> impl Strategy<Value = Op> {
    let packet = (
        any::<bool>(),
        0_usize..2,
        -20_000_i64..60_000,
        any::<bool>(),
        0_i64..3,
        prop::bool::weighted(0.3),
        prop::bool::weighted(0.85),
        1_usize..4,
    )
        .prop_map(
            |(video, track, usec_step, big_step, pts_extra, keyframe, with_time, size)| {
                Op::Packet {
                    video,
                    track,
                    usec_step,
                    dts_step: match (video, big_step) {
                        (true, false) => 1,
                        (true, true) => -1,
                        (false, false) => 1024,
                        (false, true) => -1024,
                    },
                    pts_extra,
                    keyframe,
                    with_time,
                    size,
                }
            },
        );
    prop_oneof![
        30 => packet,
        1 => any::<bool>().prop_map(Op::SetActive),
        1 => any::<bool>().prop_map(Op::SetDelay),
        1 => (0_usize..4).prop_map(Op::SetBatch),
        1 => Just(Op::Reset),
        1 => Just(Op::FreePackets),
        1 => (
            0_usize..2,
            prop::option::of(prop_oneof![Just(0.5), Just(2.0)]),
            prop_oneof![Just(0.0), Just(0.5), Just(1.0), Just(30.0)],
            prop_oneof![Just(0.0), Just(1.0), Just(30.0)],
            0_usize..3,
        )
            .prop_map(|(track, head, timestamp, last, data_size)| Op::Caption {
                track,
                head,
                timestamp,
                last,
                data_size,
            }),
        1 => (any::<bool>(), any::<bool>())
            .prop_map(|(result, consume)| Op::AddCaption { result, consume }),
        1 => (0_usize..3).prop_map(Op::Callbacks),
        1 => (any::<[bool; 2]>(), 0_u32..3).prop_map(|(members, num_started)| Op::Group {
            members,
            num_started,
        }),
        1 => (any::<bool>(), 0_usize..2)
            .prop_map(|(video, track)| Op::EncoderIndex { video, track }),
        4 => static_call().prop_map(Op::Static),
    ]
}

fn setup() -> impl Strategy<Value = Setup> {
    (
        1_usize..=2,
        1_usize..=2,
        prop_oneof![Just((1, 30)), Just((1, 60)), Just((1001, 30000))],
        prop_oneof![Just(48000), Just(44100)],
        prop_oneof![Just(1024), Just(960)],
    )
        .prop_map(
            |(videos, audios, video_timebase, audio_den, framesize)| Setup {
                videos,
                audios,
                video_timebase,
                audio_den,
                framesize,
            },
        )
}

#[test]
fn parity_random_sequences() {
    if !isolated(concat!(module_path!(), "::parity_random_sequences")) {
        return;
    }
    let log = RefCell::new(Capture::arm());
    let mut runner = TestRunner::new(Config {
        cases: 1024,
        failure_persistence: None,
        ..Config::default()
    });
    runner
        .run(
            &(setup(), prop::collection::vec(op(), 0..80)),
            |(setup, ops)| {
                run_sequence(setup, &ops, Some(&mut log.borrow_mut()));
                Ok(())
            },
        )
        .unwrap();
}

/// The exported entry point reads the real clock for each packet callback:
/// the packet time it hands over carries a `pir` between two readings.
#[test]
fn exported_entry_point_reads_the_clock() {
    let mut f = Fixture::new(Setup::c_test(1, 1));
    let o = f.ptr();
    let audio = f.audio_encoders[0];
    let video = f.video_encoders[0];
    let mut a = [1_u8];
    let mut v = [2_u8];
    // SAFETY: a live fixture and valid packets of its encoders.
    let (before, after) = unsafe {
        let before = os_gettime_ns();
        let mut pkt = encoder_packet {
            data: a.as_mut_ptr(),
            size: 1,
            dts: 48000,
            pts: 48000,
            timebase_num: 1,
            timebase_den: 48000,
            r#type: OBS_ENCODER_AUDIO,
            dts_usec: T,
            encoder: audio,
            ..encoder_packet::default()
        };
        interleave_packets(o, &raw mut pkt, ptr::null_mut());
        let mut pkt = encoder_packet {
            data: v.as_mut_ptr(),
            size: 1,
            dts: 30,
            pts: 30,
            timebase_num: 1,
            timebase_den: 30,
            r#type: OBS_ENCODER_VIDEO,
            keyframe: true,
            dts_usec: T,
            encoder: video,
            ..encoder_packet::default()
        };
        let mut time = encoder_packet_time {
            pts: 30,
            ..encoder_packet_time::default()
        };
        interleave_packets(o, &raw mut pkt, &raw mut time);
        (before, os_gettime_ns())
    };
    let events = std::mem::take(&mut f.events);
    let pir = events
        .iter()
        .find_map(|e| match e {
            Event::Callback {
                time: Some(time), ..
            } => Some(time.pir),
            _ => None,
        })
        .expect("the video packet went out with its time");
    assert!(
        (before..=after).contains(&pir),
        "{pir} in {before}..={after}"
    );
    assert!(matches!(events.last(), Some(Event::Encoded { packet, .. }) if packet.bytes == [2]));
}

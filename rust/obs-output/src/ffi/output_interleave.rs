//! C ABI for `libobs/obs-output-interleave.c.h`.
//!
//! `interleave_packets`, `get_encoder_index` and `free_packets` are
//! declared in `obs-internal.h` without `EXPORT`, so they stay hidden
//! (`rust-hidden.txt`): `obs-output.c` calls them inside libobs, and passes
//! `interleave_packets` to `obs_encoder_start()` / `obs_encoder_stop()` as
//! the encoded packet callback (or to the delay as its `delay_callback`).
//! The encoder finds a callback again by comparing the function pointer,
//! which works because `obs-output.c` names the symbol, and that symbol is
//! this function in `libobs-with-rust`.
//!
//! The output is `struct obs_output` and the encoders `struct obs_encoder`,
//! private and platform-sized, so this never reads them: libobs's hidden
//! `obs_output_interleave_fields()`, `obs_output_caption_track_fields()`,
//! `obs_encoder_interleave_info()` and `obs_encoder_group_num_started()`
//! (#1599) give the fields, and the `obs_output_interleave_*` wrappers call
//! `active()`, the two first-encoder-index helpers and `add_caption()` of
//! `obs-output.c`. Each field is read and written through those pointers
//! where the C reads and writes it, and `interleaved_mutex`,
//! `pkt_callbacks_mutex` and the track's `caption_mutex` are taken where
//! the C takes them, with libobs's `pthread_mutex_lock` on the opaque
//! pointer.
//!
//! `interleaved_packets`, `encoder_packet_times[]` and
//! `keyframe_group_tracking` are the C darrays `obs-output.c` resets and
//! frees. They are changed in place with the Rust `util/darray.h`
//! (`obs_util::ffi::darray`, #1596 and #1600), whose buffers are `bmalloc`
//! memory either side can grow or free. Queued packets are libobs's
//! reference-counted copies (`obs_encoder_packet_create_instance`, hidden in
//! libobs) or the delay's copy, and go back through
//! `obs_encoder_packet_release`.
//!
//! The clock is read once per packet callback call, as the C does. The
//! `*_with_clock` and `*_of` functions are Rust-mangled (hidden by `_ZN*`):
//! the Tier 3 test drives the entry point and the C file's static
//! functions through them, with a test clock, as the oracle renames the C
//! clock.
//!
//! No intentional differences from the C. Where nothing can see it: a
//! packet type other than `OBS_ENCODER_VIDEO` reads as audio everywhere (a
//! packet carries its encoder's type, one of the two); a released queue
//! entry is zeroed in a copy rather than in the buffer slot that is erased
//! next; and an empty queue, a missing first video packet of track 0 or a
//! null callback abort where the C reads through a null pointer or past the
//! end (the interleaver never reaches those).

use core::ffi::{c_char, c_int, c_longlong, c_void};
use core::mem::{offset_of, size_of};
use core::ptr;

use obs_util::ffi::darray::{
    darray, darray_erase, darray_erase_range, darray_free, darray_insert, darray_pop_front,
    darray_push_back_new,
};

use crate::interleave::{
    self, EncoderInfo, InterleaveOutput, KeyframeGroup, Kind, MAX_OUTPUT_AUDIO_ENCODERS,
    MAX_OUTPUT_VIDEO_ENCODERS, Packet, PacketTime, PacketView, TrackStatus,
};

pub use super::output_delay::{encoder_packet, encoder_packet_time};

/// `LOG_WARNING` (`util/base.h`).
pub const LOG_WARNING: c_int = 200;
/// `LOG_DEBUG` (`util/base.h`).
pub const LOG_DEBUG: c_int = 400;

/// `OBS_ENCODER_AUDIO` (`enum obs_encoder_type`).
pub const OBS_ENCODER_AUDIO: c_int = 0;
/// `OBS_ENCODER_VIDEO`.
pub const OBS_ENCODER_VIDEO: c_int = 1;

/// `KEYFRAME_TRACK_STATUS_NOT_SEEN` (`enum keyframe_group_track_status`).
pub const KEYFRAME_TRACK_STATUS_NOT_SEEN: c_int = 0;
/// `KEYFRAME_TRACK_STATUS_SEEN`.
pub const KEYFRAME_TRACK_STATUS_SEEN: c_int = 1;
/// `KEYFRAME_TRACK_STATUS_SKIPPED`.
pub const KEYFRAME_TRACK_STATUS_SKIPPED: c_int = 2;

/// `struct keyframe_group_data` (`obs-internal.h`).
#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct keyframe_group_data {
    pub group_id: usize,
    pub pts: i64,
    pub required_tracks: u32,
    /// `enum keyframe_group_track_status`.
    pub seen_on_track: [c_int; MAX_OUTPUT_VIDEO_ENCODERS],
}

/// `packet_cb` of `struct packet_callback`.
#[allow(non_camel_case_types)]
pub type packet_cb_t = Option<
    unsafe extern "C" fn(*mut c_void, *mut encoder_packet, *mut encoder_packet_time, *mut c_void),
>;

/// `struct packet_callback` (`obs-internal.h`).
#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy)]
pub struct packet_callback {
    pub packet_cb: packet_cb_t,
    pub param: *mut c_void,
}

/// `struct caption_text` (`obs-internal.h`): the fields the interleaver
/// reads, then the text (a flexible array member).
#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Debug)]
pub struct caption_text {
    pub display_duration: f64,
    pub next: *mut caption_text,
    pub text: [c_char; 0],
}

/// `info.encoded_packet` of `struct obs_output_info`.
#[allow(non_camel_case_types)]
pub type encoded_packet_t = Option<unsafe extern "C" fn(*mut c_void, *mut encoder_packet)>;

/// `struct obs_output_interleave_fields` (`obs-internal.h`): pointers into
/// one `struct obs_output`. The arrays point at their first element:
/// `received_video`, `video_offsets`, `highest_video_ts`, `video_encoders`
/// and `encoder_packet_times` have [`MAX_OUTPUT_VIDEO_ENCODERS`] entries,
/// `audio_offsets` and `audio_encoders` [`MAX_OUTPUT_AUDIO_ENCODERS`].
#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy)]
pub struct obs_output_interleave_fields {
    /// `pthread_mutex_t *`.
    pub interleaved_mutex: *mut c_void,
    /// `obs_encoder_packet_array_t *`.
    pub interleaved_packets: *mut darray,
    pub received_video: *mut bool,
    pub received_audio: *mut bool,
    pub video_offsets: *mut i64,
    pub audio_offsets: *mut i64,
    pub highest_audio_ts: *mut i64,
    pub highest_video_ts: *mut i64,
    /// `obs_encoder_t *const *`.
    pub video_encoders: *const *mut c_void,
    pub audio_encoders: *const *mut c_void,
    /// `obs_encoder_packet_time_array_t *`.
    pub encoder_packet_times: *mut darray,
    /// `obs_keyframe_group_array_t *`.
    pub keyframe_group_tracking: *mut darray,
    pub pkt_callbacks_mutex: *mut c_void,
    /// `obs_packet_callback_array_t *`.
    pub pkt_callbacks: *mut darray,
    pub total_frames: *mut c_int,
    pub interleaver_max_batch_size: *const usize,
    pub active_delay_ns: *const u64,
    pub encoded_packet: *const encoded_packet_t,
    pub data: *const *mut c_void,
    pub name: *const *mut c_char,
}

/// `struct obs_output_caption_track_fields` (`obs-internal.h`).
#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy)]
pub struct obs_output_caption_track_fields {
    /// `pthread_mutex_t *`.
    pub caption_mutex: *mut c_void,
    pub caption_head: *const *mut caption_text,
    pub caption_timestamp: *mut f64,
    pub last_caption_timestamp: *mut f64,
    pub caption_data_size: *const usize,
}

/// `struct obs_encoder_interleave_info` (`obs-internal.h`).
#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy)]
pub struct obs_encoder_interleave_info {
    pub timebase_num: u32,
    pub timebase_den: u32,
    pub framesize: usize,
    /// `struct obs_encoder_group *`.
    pub group: *mut c_void,
}

unsafe extern "C" {
    // obs-output.c and obs-encoder.c, hidden in libobs (obs-internal.h).
    fn obs_output_interleave_fields(output: *const c_void) -> obs_output_interleave_fields;
    fn obs_output_caption_track_fields(
        output: *mut c_void,
        track_idx: usize,
    ) -> obs_output_caption_track_fields;
    fn obs_output_interleave_active(output: *const c_void) -> bool;
    fn obs_output_interleave_first_audio_encoder_index(
        output: *const c_void,
        index: *mut usize,
    ) -> bool;
    fn obs_output_interleave_first_video_encoder_index(
        output: *const c_void,
        index: *mut usize,
    ) -> bool;
    fn obs_output_interleave_add_caption(output: *mut c_void, out: *mut encoder_packet) -> bool;
    fn obs_encoder_interleave_info(encoder: *const c_void) -> obs_encoder_interleave_info;
    fn obs_encoder_group_num_started(group: *mut c_void) -> u32;
    fn obs_encoder_packet_create_instance(dst: *mut encoder_packet, src: *const encoder_packet);
    // EXPORT.
    fn obs_encoder_packet_release(packet: *mut encoder_packet);
    fn obs_output_get_name(output: *const c_void) -> *const c_char;
    fn obs_encoder_get_name(encoder: *const c_void) -> *const c_char;
    // libobs's pthread (w32-pthreads on Windows); the mutexes are opaque here.
    fn pthread_mutex_lock(mutex: *mut c_void) -> c_int;
    fn pthread_mutex_unlock(mutex: *mut c_void) -> c_int;
    fn os_gettime_ns() -> u64;
    fn blog(level: c_int, format: *const c_char, ...);
}

impl PacketView for encoder_packet {
    fn kind(&self) -> Kind {
        if self.r#type == OBS_ENCODER_VIDEO {
            Kind::Video
        } else {
            Kind::Audio
        }
    }
    fn track(&self) -> usize {
        self.track_idx
    }
    fn dts(&self) -> i64 {
        self.dts
    }
    fn pts(&self) -> i64 {
        self.pts
    }
    fn dts_usec(&self) -> i64 {
        self.dts_usec
    }
    fn keyframe(&self) -> bool {
        self.keyframe
    }
    fn timebase_num(&self) -> i32 {
        self.timebase_num
    }
    fn timebase_den(&self) -> i32 {
        self.timebase_den
    }
    fn encoder(&self) -> usize {
        self.encoder as usize
    }
}

impl Packet for encoder_packet {
    fn set_track(&mut self, track: usize) {
        self.track_idx = track;
    }
    fn set_dts(&mut self, dts: i64) {
        self.dts = dts;
    }
    fn set_pts(&mut self, pts: i64) {
        self.pts = pts;
    }
    fn set_dts_usec(&mut self, dts_usec: i64) {
        self.dts_usec = dts_usec;
    }
}

impl PacketTime for encoder_packet_time {
    fn pts(&self) -> i64 {
        self.pts
    }
    fn set_pts(&mut self, pts: i64) {
        self.pts = pts;
    }
    fn set_pir(&mut self, pir: u64) {
        self.pir = pir;
    }
}

fn status_of(raw: c_int) -> TrackStatus {
    match raw {
        KEYFRAME_TRACK_STATUS_NOT_SEEN => TrackStatus::NotSeen,
        KEYFRAME_TRACK_STATUS_SEEN => TrackStatus::Seen,
        _ => TrackStatus::Skipped,
    }
}

impl KeyframeGroup for keyframe_group_data {
    fn new(group_id: usize, pts: i64, track: usize, required_tracks: u32) -> Self {
        let mut seen_on_track = [KEYFRAME_TRACK_STATUS_NOT_SEEN; MAX_OUTPUT_VIDEO_ENCODERS];
        seen_on_track[track] = KEYFRAME_TRACK_STATUS_SEEN;
        Self {
            group_id,
            pts,
            required_tracks,
            seen_on_track,
        }
    }
    fn group_id(&self) -> usize {
        self.group_id
    }
    fn pts(&self) -> i64 {
        self.pts
    }
    fn required_tracks(&self) -> u32 {
        self.required_tracks
    }
    fn seen(&self, track: usize) -> TrackStatus {
        status_of(self.seen_on_track[track])
    }
    fn set_seen(&mut self, track: usize, status: TrackStatus) {
        self.seen_on_track[track] = match status {
            TrackStatus::NotSeen => KEYFRAME_TRACK_STATUS_NOT_SEEN,
            TrackStatus::Seen => KEYFRAME_TRACK_STATUS_SEEN,
            TrackStatus::Skipped => KEYFRAME_TRACK_STATUS_SKIPPED,
        };
    }
}

/// The items of a darray of `T`.
///
/// # Safety
///
/// `d` is a live darray of `T` that nothing changes while the slice lives.
unsafe fn items<'a, T>(d: *const darray) -> &'a [T] {
    // SAFETY: caller contract; an empty darray may have a null buffer.
    unsafe {
        if (*d).num == 0 {
            &[]
        } else {
            core::slice::from_raw_parts((*d).array.cast::<T>(), (*d).num)
        }
    }
}

/// [`items`], mutable.
///
/// # Safety
///
/// As [`items`], and nothing else reads it while the slice lives.
unsafe fn items_mut<'a, T>(d: *mut darray) -> &'a mut [T] {
    // SAFETY: caller contract.
    unsafe {
        if (*d).num == 0 {
            &mut []
        } else {
            core::slice::from_raw_parts_mut((*d).array.cast::<T>(), (*d).num)
        }
    }
}

const ZEROED_DARRAY: darray = darray {
    array: ptr::null_mut(),
    num: 0,
    capacity: 0,
};

/// The queue's old buffer while it is resorted.
pub struct Detached(darray);

impl AsRef<[encoder_packet]> for Detached {
    fn as_ref(&self) -> &[encoder_packet] {
        // SAFETY: the detached darray is owned here until it is freed.
        unsafe { items(&raw const self.0) }
    }
}

/// `os_gettime_ns()`.
fn libobs_clock() -> u64 {
    // SAFETY: a plain libobs call.
    unsafe { os_gettime_ns() }
}

/// One output, seen through its interleaver fields.
struct COutput {
    output: *mut c_void,
    f: obs_output_interleave_fields,
    /// The caption fields of the track whose mutex is held.
    caption: Option<obs_output_caption_track_fields>,
    clock: fn() -> u64,
}

impl COutput {
    /// # Safety
    ///
    /// `output` is a live `obs_output_t`.
    unsafe fn new(output: *const c_void, clock: fn() -> u64) -> Self {
        Self {
            output: output.cast_mut(),
            // SAFETY: a live output (caller contract).
            f: unsafe { obs_output_interleave_fields(output) },
            caption: None,
            clock,
        }
    }

    fn times(&self, track: usize) -> *mut darray {
        assert!(track < MAX_OUTPUT_VIDEO_ENCODERS);
        // SAFETY: one of the MAX_OUTPUT_VIDEO_ENCODERS darrays.
        unsafe { self.f.encoder_packet_times.add(track) }
    }

    fn caption_fields(&self) -> &obs_output_caption_track_fields {
        self.caption
            .as_ref()
            .expect("the caption fields are read under the caption mutex")
    }
}

const PACKET: usize = size_of::<encoder_packet>();
const TIME: usize = size_of::<encoder_packet_time>();
const GROUP: usize = size_of::<keyframe_group_data>();

impl InterleaveOutput for COutput {
    type Packet = encoder_packet;
    type Time = encoder_packet_time;
    type Group = keyframe_group_data;
    type Detached = Detached;

    fn active(&self) -> bool {
        // SAFETY: a live output.
        unsafe { obs_output_interleave_active(self.output) }
    }

    fn first_audio_encoder_index(&self) -> Option<usize> {
        let mut index = 0;
        // SAFETY: a live output and a writable index.
        unsafe { obs_output_interleave_first_audio_encoder_index(self.output, &raw mut index) }
            .then_some(index)
    }

    fn first_video_encoder_index(&self) -> Option<usize> {
        let mut index = 0;
        // SAFETY: as above.
        unsafe { obs_output_interleave_first_video_encoder_index(self.output, &raw mut index) }
            .then_some(index)
    }

    fn add_caption(&mut self, packet: &mut encoder_packet) -> bool {
        // SAFETY: a live output and the packet being sent.
        unsafe { obs_output_interleave_add_caption(self.output, packet) }
    }

    fn video_encoder(&self, slot: usize) -> Option<usize> {
        assert!(slot < MAX_OUTPUT_VIDEO_ENCODERS);
        // SAFETY: one of the MAX_OUTPUT_VIDEO_ENCODERS slots.
        let encoder = unsafe { *self.f.video_encoders.add(slot) };
        (!encoder.is_null()).then_some(encoder as usize)
    }

    fn audio_encoder(&self, slot: usize) -> Option<usize> {
        assert!(slot < MAX_OUTPUT_AUDIO_ENCODERS);
        // SAFETY: one of the MAX_OUTPUT_AUDIO_ENCODERS slots.
        let encoder = unsafe { *self.f.audio_encoders.add(slot) };
        (!encoder.is_null()).then_some(encoder as usize)
    }

    fn encoder_info(&self, encoder: usize) -> EncoderInfo {
        // SAFETY: the encoder of a packet the output was given.
        let info = unsafe { obs_encoder_interleave_info(encoder as *const c_void) };
        EncoderInfo {
            timebase_num: info.timebase_num,
            timebase_den: info.timebase_den,
            framesize: info.framesize,
            group: (!info.group.is_null()).then_some(info.group as usize),
        }
    }

    fn group_num_started(&self, group: usize) -> u32 {
        // SAFETY: the group of a live encoder (`encoder_info`).
        unsafe { obs_encoder_group_num_started(group as *mut c_void) }
    }

    fn lock(&mut self) {
        // SAFETY: the output's initialized `interleaved_mutex`.
        unsafe { pthread_mutex_lock(self.f.interleaved_mutex) };
    }

    fn unlock(&mut self) {
        // SAFETY: held by this thread (`lock`).
        unsafe { pthread_mutex_unlock(self.f.interleaved_mutex) };
    }

    fn received_video(&self, track: usize) -> bool {
        assert!(track < MAX_OUTPUT_VIDEO_ENCODERS);
        // SAFETY: one of the flags.
        unsafe { *self.f.received_video.add(track) }
    }

    fn set_received_video(&mut self, track: usize, received: bool) {
        assert!(track < MAX_OUTPUT_VIDEO_ENCODERS);
        // SAFETY: one of the flags.
        unsafe { *self.f.received_video.add(track) = received };
    }

    fn received_audio(&self) -> bool {
        // SAFETY: a live field.
        unsafe { *self.f.received_audio }
    }

    fn set_received_audio(&mut self, received: bool) {
        // SAFETY: a live field.
        unsafe { *self.f.received_audio = received };
    }

    fn video_offset(&self, track: usize) -> i64 {
        assert!(track < MAX_OUTPUT_VIDEO_ENCODERS);
        // SAFETY: one of the offsets.
        unsafe { *self.f.video_offsets.add(track) }
    }

    fn set_video_offset(&mut self, track: usize, offset: i64) {
        assert!(track < MAX_OUTPUT_VIDEO_ENCODERS);
        // SAFETY: one of the offsets.
        unsafe { *self.f.video_offsets.add(track) = offset };
    }

    fn audio_offset(&self, track: usize) -> i64 {
        assert!(track < MAX_OUTPUT_AUDIO_ENCODERS);
        // SAFETY: one of the offsets.
        unsafe { *self.f.audio_offsets.add(track) }
    }

    fn set_audio_offset(&mut self, track: usize, offset: i64) {
        assert!(track < MAX_OUTPUT_AUDIO_ENCODERS);
        // SAFETY: one of the offsets.
        unsafe { *self.f.audio_offsets.add(track) = offset };
    }

    fn highest_audio_ts(&self) -> i64 {
        // SAFETY: a live field.
        unsafe { *self.f.highest_audio_ts }
    }

    fn set_highest_audio_ts(&mut self, ts: i64) {
        // SAFETY: a live field.
        unsafe { *self.f.highest_audio_ts = ts };
    }

    fn highest_video_ts(&self, track: usize) -> i64 {
        assert!(track < MAX_OUTPUT_VIDEO_ENCODERS);
        // SAFETY: one of the timestamps.
        unsafe { *self.f.highest_video_ts.add(track) }
    }

    fn set_highest_video_ts(&mut self, track: usize, ts: i64) {
        assert!(track < MAX_OUTPUT_VIDEO_ENCODERS);
        // SAFETY: one of the timestamps.
        unsafe { *self.f.highest_video_ts.add(track) = ts };
    }

    fn active_delay_ns(&self) -> u64 {
        // SAFETY: a live field.
        unsafe { *self.f.active_delay_ns }
    }

    fn max_batch_size(&self) -> usize {
        // SAFETY: a live field.
        unsafe { *self.f.interleaver_max_batch_size }
    }

    fn inc_total_frames(&mut self) {
        // SAFETY: a live field; `int` arithmetic wraps as it does in practice.
        unsafe { *self.f.total_frames = (*self.f.total_frames).wrapping_add(1) };
    }

    fn packets(&self) -> &[encoder_packet] {
        // SAFETY: the queue, under `interleaved_mutex`.
        unsafe { items(self.f.interleaved_packets) }
    }

    fn packets_mut(&mut self) -> &mut [encoder_packet] {
        // SAFETY: as above.
        unsafe { items_mut(self.f.interleaved_packets) }
    }

    fn insert_packet(&mut self, idx: usize, packet: encoder_packet) {
        // SAFETY: the queue, a darray of packets; `packet` is one.
        unsafe {
            darray_insert(
                PACKET,
                self.f.interleaved_packets,
                idx,
                (&raw const packet).cast(),
            );
        }
    }

    fn erase_front(&mut self, end: usize) {
        // SAFETY: the queue (the core checks `end`).
        unsafe { darray_erase_range(PACKET, self.f.interleaved_packets, 0, end) };
    }

    fn detach_packets(&mut self) -> Detached {
        // SAFETY: the queue; its header is moved out and zeroed, as the C
        // copies `.da` and memsets the array.
        unsafe { Detached(ptr::replace(self.f.interleaved_packets, ZEROED_DARRAY)) }
    }

    fn free_detached(&mut self, old: Detached) {
        let mut old = old;
        // SAFETY: the old buffer, which nothing else points at.
        unsafe { darray_free(&raw mut old.0) };
    }

    fn free_queue(&mut self) {
        // SAFETY: the queue.
        unsafe { darray_free(self.f.interleaved_packets) };
    }

    fn packet_times(&self, track: usize) -> &[encoder_packet_time] {
        // SAFETY: the track's darray of packet times.
        unsafe { items(self.times(track)) }
    }

    fn packet_times_mut(&mut self, track: usize) -> &mut [encoder_packet_time] {
        // SAFETY: as above.
        unsafe { items_mut(self.times(track)) }
    }

    fn push_packet_time(&mut self, track: usize, time: encoder_packet_time) {
        // SAFETY: as above; the new item is one zeroed packet time.
        unsafe {
            let new = darray_push_back_new(TIME, self.times(track));
            *new.cast::<encoder_packet_time>() = time;
        }
    }

    fn erase_packet_time(&mut self, track: usize, idx: usize) {
        // SAFETY: as above.
        unsafe { darray_erase(TIME, self.times(track), idx) };
    }

    fn pop_packet_time(&mut self, track: usize) {
        // SAFETY: as above.
        unsafe { darray_pop_front(TIME, self.times(track)) };
    }

    fn groups(&self) -> &[keyframe_group_data] {
        // SAFETY: `keyframe_group_tracking`, under `interleaved_mutex`.
        unsafe { items(self.f.keyframe_group_tracking) }
    }

    fn groups_mut(&mut self) -> &mut [keyframe_group_data] {
        // SAFETY: as above.
        unsafe { items_mut(self.f.keyframe_group_tracking) }
    }

    fn erase_group(&mut self, idx: usize) {
        // SAFETY: as above.
        unsafe { darray_erase(GROUP, self.f.keyframe_group_tracking, idx) };
    }

    fn insert_group(&mut self, idx: usize, group: keyframe_group_data) {
        // SAFETY: as above; `group` is one entry.
        unsafe {
            darray_insert(
                GROUP,
                self.f.keyframe_group_tracking,
                idx,
                (&raw const group).cast(),
            );
        }
    }

    fn lock_caption(&mut self, track: usize) {
        // SAFETY: a live output and one of its video tracks.
        let fields = unsafe { obs_output_caption_track_fields(self.output, track) };
        // SAFETY: the track's initialized `caption_mutex`.
        unsafe { pthread_mutex_lock(fields.caption_mutex) };
        self.caption = Some(fields);
    }

    fn unlock_caption(&mut self, _track: usize) {
        let fields = self.caption.take().expect("the caption mutex is held");
        // SAFETY: held by this thread (`lock_caption`).
        unsafe { pthread_mutex_unlock(fields.caption_mutex) };
    }

    fn caption_head_duration(&self, _track: usize) -> Option<f64> {
        // SAFETY: the track's caption fields, under its mutex; a non-null
        // head is a live `struct caption_text`.
        unsafe {
            let head = *self.caption_fields().caption_head;
            (!head.is_null()).then(|| (*head).display_duration)
        }
    }

    fn caption_timestamp(&self, _track: usize) -> f64 {
        // SAFETY: as above.
        unsafe { *self.caption_fields().caption_timestamp }
    }

    fn set_caption_timestamp(&mut self, _track: usize, ts: f64) {
        // SAFETY: as above.
        unsafe { *self.caption_fields().caption_timestamp = ts };
    }

    fn last_caption_timestamp(&self, _track: usize) -> f64 {
        // SAFETY: as above.
        unsafe { *self.caption_fields().last_caption_timestamp }
    }

    fn set_last_caption_timestamp(&mut self, _track: usize, ts: f64) {
        // SAFETY: as above.
        unsafe { *self.caption_fields().last_caption_timestamp = ts };
    }

    fn caption_data_size(&self, _track: usize) -> usize {
        // SAFETY: as above.
        unsafe { *self.caption_fields().caption_data_size }
    }

    fn lock_callbacks(&mut self) {
        // SAFETY: the output's initialized `pkt_callbacks_mutex`.
        unsafe { pthread_mutex_lock(self.f.pkt_callbacks_mutex) };
    }

    fn unlock_callbacks(&mut self) {
        // SAFETY: held by this thread (`lock_callbacks`).
        unsafe { pthread_mutex_unlock(self.f.pkt_callbacks_mutex) };
    }

    fn callback_count(&self) -> usize {
        // SAFETY: `pkt_callbacks`, under its mutex.
        unsafe { (*self.f.pkt_callbacks).num }
    }

    fn call_callback(
        &mut self,
        idx: usize,
        packet: &mut encoder_packet,
        time: Option<&mut encoder_packet_time>,
    ) {
        // SAFETY: `pkt_callbacks`, under its mutex; `idx` is below its size.
        let callback = unsafe { items::<packet_callback>(self.f.pkt_callbacks)[idx] };
        let time = time.map_or(ptr::null_mut(), ptr::from_mut);
        let cb = callback.packet_cb.expect("a registered packet callback");
        // SAFETY: a callback libobs registered, with its parameter.
        unsafe { cb(self.output, packet, time, callback.param) };
    }

    fn encoded_packet(&mut self, packet: &mut encoder_packet) {
        // SAFETY: the output's `info.encoded_packet` and `context.data`.
        unsafe {
            let encoded = (*self.f.encoded_packet).expect("an encoded output's encoded_packet");
            encoded(*self.f.data, packet);
        }
    }

    fn now(&self) -> u64 {
        (self.clock)()
    }

    fn copy_packet(&mut self, src: &encoder_packet) -> encoder_packet {
        let mut dst = encoder_packet::default();
        // SAFETY: `src` is the encoder's packet, valid for `size` bytes of
        // data; `dst` is written whole.
        unsafe { obs_encoder_packet_create_instance(&raw mut dst, src) };
        dst
    }

    fn release(&mut self, packet: &mut encoder_packet) {
        // SAFETY: a held copy (or the delay's copy).
        unsafe { obs_encoder_packet_release(packet) };
    }

    fn log_sending_caption(&self, _track: usize, frame_timestamp: f64) {
        // SAFETY: the head is a live caption, with its text after its two
        // fields (a NUL-terminated flexible array member).
        unsafe {
            let head = *self.caption_fields().caption_head;
            let text = head
                .cast::<u8>()
                .add(offset_of!(caption_text, text))
                .cast::<c_char>();
            blog(
                LOG_DEBUG,
                c"Sending caption: %f \"%s\"".as_ptr(),
                frame_timestamp,
                text,
            );
        }
    }

    fn log_packet_time_not_found(&self, track: usize, pts: i64) {
        // SAFETY: the format and its arguments, as the C passes them (the
        // track as a size_t for `%lu`).
        unsafe {
            blog(
                LOG_DEBUG,
                c"%s: Track %lu encoder packet timing for PTS%lld not found.".as_ptr(),
                c"send_interleaved".as_ptr(),
                track,
                pts as c_longlong,
            );
        }
    }

    fn log_packet_times_empty(&self, track: usize) {
        // SAFETY: as above.
        unsafe {
            blog(
                LOG_DEBUG,
                c"%s: Track %lu encoder packet timing array empty.".as_ptr(),
                c"send_interleaved".as_ptr(),
                track,
            );
        }
    }

    fn warn_missing_keyframe(&self, pts: i64, encoder: usize, track: usize) {
        // SAFETY: a live output and encoder; their names are C strings (or
        // null, which C's printf prints as C does).
        unsafe {
            blog(
                LOG_WARNING,
                c"obs-output '%s': Missing keyframe with pts %lld for encoder '%s' (track: %zu)"
                    .as_ptr(),
                obs_output_get_name(self.output),
                pts as c_longlong,
                obs_encoder_get_name(encoder as *const c_void),
                track,
            );
        }
    }
}

/// Same contract as C `interleave_packets`, the encoded packet callback of
/// an output with video and audio.
///
/// # Safety
///
/// `data` is a live `obs_output_t`, `packet` a valid packet of one of its
/// encoders and `packet_time` null or valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn interleave_packets(
    data: *mut c_void,
    packet: *mut encoder_packet,
    packet_time: *mut encoder_packet_time,
) {
    // SAFETY: forwarded caller contract.
    unsafe { interleave_packets_with_clock(data, packet, packet_time, libobs_clock) };
}

/// [`interleave_packets`] with `clock` for `os_gettime_ns()`.
///
/// # Safety
///
/// As [`interleave_packets`].
pub unsafe fn interleave_packets_with_clock(
    data: *mut c_void,
    packet: *mut encoder_packet,
    packet_time: *mut encoder_packet_time,
    clock: fn() -> u64,
) {
    // SAFETY: forwarded caller contract.
    unsafe {
        let mut o = COutput::new(data, clock);
        let time = packet_time.as_ref();
        interleave::interleave_packets(&mut o, &mut *packet, time);
    }
}

/// Same contract as C `get_encoder_index`.
///
/// # Safety
///
/// `output` is a live `obs_output_t` and `pkt` a valid packet.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn get_encoder_index(
    output: *const c_void,
    pkt: *mut encoder_packet,
) -> usize {
    // SAFETY: forwarded caller contract.
    unsafe {
        let o = COutput::new(output, libobs_clock);
        interleave::get_encoder_index(&o, &*pkt)
    }
}

/// Same contract as C `free_packets`.
///
/// # Safety
///
/// `output` is a live `obs_output_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn free_packets(output: *mut c_void) {
    // SAFETY: forwarded caller contract.
    let mut o = unsafe { COutput::new(output, libobs_clock) };
    interleave::free_packets(&mut o);
}

/// The static functions of the C file, on a live output, for the Tier 3
/// test (the oracle wraps the C ones the same way).
pub mod statics {
    use core::ffi::{c_int, c_void};

    use super::{COutput, encoder_packet, libobs_clock};
    use crate::interleave;

    /// `get_interleaved_start_idx()`.
    ///
    /// # Safety
    ///
    /// `output` is a live `obs_output_t` whose queue holds a video packet
    /// of track 0 and audio at or after the start.
    pub unsafe fn get_interleaved_start_idx(output: *mut c_void) -> usize {
        // SAFETY: caller contract.
        let o = unsafe { COutput::new(output, libobs_clock) };
        interleave::get_interleaved_start_idx(&o)
    }

    /// `prune_premature_packets()`, with its C return value.
    ///
    /// # Safety
    ///
    /// `output` is a live `obs_output_t`.
    pub unsafe fn prune_premature_packets(output: *mut c_void) -> c_int {
        // SAFETY: caller contract.
        let mut o = unsafe { COutput::new(output, libobs_clock) };
        let prune = interleave::prune_premature_packets(&mut o).c_value();
        c_int::try_from(prune).expect("a queue index fits a C int")
    }

    /// `prune_interleaved_packets()`.
    ///
    /// # Safety
    ///
    /// `output` is a live `obs_output_t`.
    pub unsafe fn prune_interleaved_packets(output: *mut c_void) -> bool {
        // SAFETY: caller contract.
        let mut o = unsafe { COutput::new(output, libobs_clock) };
        interleave::prune_interleaved_packets(&mut o)
    }

    /// `initialize_interleaved_packets()`.
    ///
    /// # Safety
    ///
    /// `output` is a live `obs_output_t`.
    pub unsafe fn initialize_interleaved_packets(output: *mut c_void) -> bool {
        // SAFETY: caller contract.
        let mut o = unsafe { COutput::new(output, libobs_clock) };
        interleave::initialize_interleaved_packets(&mut o)
    }

    /// `resort_interleaved_packets()`.
    ///
    /// # Safety
    ///
    /// `output` is a live `obs_output_t`.
    pub unsafe fn resort_interleaved_packets(output: *mut c_void) {
        // SAFETY: caller contract.
        let mut o = unsafe { COutput::new(output, libobs_clock) };
        interleave::resort_interleaved_packets(&mut o);
    }

    /// `discard_unused_audio_packets()`.
    ///
    /// # Safety
    ///
    /// `output` is a live `obs_output_t`.
    pub unsafe fn discard_unused_audio_packets(output: *mut c_void, dts_usec: i64) {
        // SAFETY: caller contract.
        let mut o = unsafe { COutput::new(output, libobs_clock) };
        interleave::discard_unused_audio_packets(&mut o, dts_usec);
    }

    /// `send_interleaved()`, with `clock` for `os_gettime_ns()`.
    ///
    /// # Safety
    ///
    /// `output` is a live `obs_output_t` with a packet queued.
    pub unsafe fn send_interleaved(output: *mut c_void, clock: fn() -> u64) {
        // SAFETY: caller contract.
        let mut o = unsafe { COutput::new(output, clock) };
        interleave::send_interleaved(&mut o);
    }

    /// `check_encoder_group_keyframe_alignment()`.
    ///
    /// # Safety
    ///
    /// `output` is a live `obs_output_t` and `packet` a valid packet of one
    /// of its encoders.
    pub unsafe fn check_encoder_group_keyframe_alignment(
        output: *mut c_void,
        packet: *const encoder_packet,
    ) {
        // SAFETY: caller contract.
        unsafe {
            let mut o = COutput::new(output, libobs_clock);
            interleave::check_encoder_group_keyframe_alignment(&mut o, &*packet);
        }
    }

    /// `count_streamable_frames()`.
    ///
    /// # Safety
    ///
    /// `output` is a live `obs_output_t`.
    pub unsafe fn count_streamable_frames(output: *mut c_void) -> usize {
        // SAFETY: caller contract.
        let o = unsafe { COutput::new(output, libobs_clock) };
        interleave::count_streamable_frames(&o)
    }

    /// `apply_ept_offsets()`.
    ///
    /// # Safety
    ///
    /// `output` is a live `obs_output_t`.
    pub unsafe fn apply_ept_offsets(output: *mut c_void) {
        // SAFETY: caller contract.
        let mut o = unsafe { COutput::new(output, libobs_clock) };
        interleave::apply_ept_offsets(&mut o);
    }
}

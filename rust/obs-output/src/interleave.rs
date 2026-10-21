//! Safe core of `libobs/obs-output-interleave.c.h`: the encoded packet
//! interleaver.
//!
//! An output with both video and audio gets its encoders' packets through
//! `interleave_packets`. It holds them in one queue sorted by `dts_usec`
//! until every track has started, picks the start point where the first
//! video and audio packets are closest (dropping what comes before it),
//! shifts every track to start at timestamp 0, and from then on sends the
//! front of the queue once a packet of the opposing type is ahead of it, so
//! the output sees monotonic timestamps. It also hands each sent video
//! packet its encoder timing, decides when a caption goes into a video
//! packet, and checks that grouped encoders emit their keyframes together.
//!
//! The pure parts work on a slice of packets ([`PacketView`]): the first
//! and last packet of a track, the start index, the premature prune, the
//! insertion position, the offset arithmetic, the streamable count and the
//! catch-up rule, the discard count, the caption timing and the packet-time
//! lookup. The functions named after the C ones are generic over the output
//! ([`InterleaveOutput`]): its fields, the queue and the other arrays, the
//! locks, the encoders and the libobs calls. The C ABI shim runs them on the
//! real `struct obs_output` through libobs's hidden accessors; the tests run
//! them on a model.
//!
//! Characterized, not endorsed (`test_output_interleave`), and kept:
//! - audio that starts more than one video frame after the first keyframe
//!   drops that keyframe, and the output waits for the next one;
//! - the premature prune decides on the difference of the last audio track
//!   checked, not the largest one;
//! - [`get_encoder_duration`] truncates the per-sample time before it
//!   multiplies by the frame size;
//! - the start index follows video track 0 only, and can drop another audio
//!   track's only packet;
//! - a premature prune with more video queued starts a track on a frame that
//!   is not a keyframe;
//! - a video track waiting for its keyframe discards every queued packet
//!   below it, other tracks' keyframes included;
//! - an audio offset is taken only from a dts > 0, so priming audio keeps
//!   its own timestamps;
//! - a delayed packet that reaches an inactive output is not released;
//! - a later keyframe of the same track marks an older group entry skipped
//!   without a warning.
//!
//! Integer arithmetic wraps where the C's would overflow; libobs's
//! timestamps never get near that.

/// `MAX_OUTPUT_VIDEO_ENCODERS` (`obs-output.h`).
pub const MAX_OUTPUT_VIDEO_ENCODERS: usize = 10;
/// `MAX_OUTPUT_AUDIO_ENCODERS` (`obs-output.h`).
pub const MAX_OUTPUT_AUDIO_ENCODERS: usize = 6;

/// `enum obs_encoder_type`, as far as a packet can have it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Audio,
    Video,
}

/// The fields of a `struct encoder_packet` the interleaver reads.
pub trait PacketView {
    fn kind(&self) -> Kind;
    /// `track_idx`.
    fn track(&self) -> usize;
    fn dts(&self) -> i64;
    fn pts(&self) -> i64;
    fn dts_usec(&self) -> i64;
    fn keyframe(&self) -> bool;
    fn timebase_num(&self) -> i32;
    fn timebase_den(&self) -> i32;
    /// The encoder that made the packet, as an identity (its address).
    fn encoder(&self) -> usize;
}

/// A packet the interleaver can write: a struct copy shares the data, as a
/// C struct copy does.
pub trait Packet: PacketView + Clone {
    fn set_track(&mut self, track: usize);
    fn set_dts(&mut self, dts: i64);
    fn set_pts(&mut self, pts: i64);
    fn set_dts_usec(&mut self, dts_usec: i64);
}

/// `struct encoder_packet_time`: an encoder's timing of one video frame.
/// `Default` is the zeroed struct.
pub trait PacketTime: Clone + Default {
    fn pts(&self) -> i64;
    fn set_pts(&mut self, pts: i64);
    /// `pir`, the packet interleave request time.
    fn set_pir(&mut self, pir: u64);
}

/// `enum keyframe_group_track_status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackStatus {
    NotSeen,
    Seen,
    Skipped,
}

/// `struct keyframe_group_data`: one keyframe pts of an encoder group, and
/// which video tracks have had it.
pub trait KeyframeGroup {
    /// The entry the C inserts: zeroed, then the group, the pts, `track`
    /// seen and the group's started encoder count.
    fn new(group_id: usize, pts: i64, track: usize, required_tracks: u32) -> Self;
    fn group_id(&self) -> usize;
    fn pts(&self) -> i64;
    fn required_tracks(&self) -> u32;
    fn seen(&self, track: usize) -> TrackStatus;
    fn set_seen(&mut self, track: usize, status: TrackStatus);
}

/// What the interleaver reads from an encoder
/// (`obs_encoder_interleave_info`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncoderInfo {
    pub timebase_num: u32,
    pub timebase_den: u32,
    pub framesize: usize,
    /// The encoder group, as an identity (its address); `None` without one.
    pub group: Option<usize>,
}

/// An output the interleaver runs on: the interleaver fields of
/// `struct obs_output`, its arrays (C darrays, changed in place), its
/// locks, its encoders and the libobs calls the C file makes.
///
/// Track numbers are below [`MAX_OUTPUT_VIDEO_ENCODERS`] for video and
/// [`MAX_OUTPUT_AUDIO_ENCODERS`] for audio.
pub trait InterleaveOutput {
    /// `struct encoder_packet`.
    type Packet: Packet;
    /// `struct encoder_packet_time`.
    type Time: PacketTime;
    /// `struct keyframe_group_data`.
    type Group: KeyframeGroup;
    /// The queue's old buffer while it is resorted.
    type Detached: AsRef<[Self::Packet]>;

    /// `active()` of `obs-output.c`.
    fn active(&self) -> bool;
    /// `get_first_audio_encoder_index()`: the first slot with an encoder.
    fn first_audio_encoder_index(&self) -> Option<usize>;
    /// `get_first_video_encoder_index()`.
    fn first_video_encoder_index(&self) -> Option<usize>;
    /// `add_caption()` of `obs-output.c`, which may rewrite the packet.
    fn add_caption(&mut self, packet: &mut Self::Packet) -> bool;

    /// The encoder in a video slot, as an identity.
    fn video_encoder(&self, slot: usize) -> Option<usize>;
    /// The encoder in an audio slot, as an identity.
    fn audio_encoder(&self, slot: usize) -> Option<usize>;
    /// `obs_encoder_interleave_info()`.
    fn encoder_info(&self, encoder: usize) -> EncoderInfo;
    /// `obs_encoder_group_num_started()`.
    fn group_num_started(&self, group: usize) -> u32;

    /// Takes `interleaved_mutex`.
    fn lock(&mut self);
    /// Releases `interleaved_mutex`.
    fn unlock(&mut self);

    fn received_video(&self, track: usize) -> bool;
    fn set_received_video(&mut self, track: usize, received: bool);
    fn received_audio(&self) -> bool;
    fn set_received_audio(&mut self, received: bool);
    fn video_offset(&self, track: usize) -> i64;
    fn set_video_offset(&mut self, track: usize, offset: i64);
    fn audio_offset(&self, track: usize) -> i64;
    fn set_audio_offset(&mut self, track: usize, offset: i64);
    fn highest_audio_ts(&self) -> i64;
    fn set_highest_audio_ts(&mut self, ts: i64);
    fn highest_video_ts(&self, track: usize) -> i64;
    fn set_highest_video_ts(&mut self, track: usize, ts: i64);
    fn active_delay_ns(&self) -> u64;
    /// `interleaver_max_batch_size`.
    fn max_batch_size(&self) -> usize;
    /// `total_frames++`.
    fn inc_total_frames(&mut self);

    /// `interleaved_packets`.
    fn packets(&self) -> &[Self::Packet];
    fn packets_mut(&mut self) -> &mut [Self::Packet];
    /// `da_insert`.
    fn insert_packet(&mut self, idx: usize, packet: Self::Packet);
    /// `da_erase_range(.., 0, end)` (`da_erase(.., 0)` for one).
    fn erase_front(&mut self, end: usize);
    /// Moves the queue out and leaves it zeroed (no buffer).
    fn detach_packets(&mut self) -> Self::Detached;
    /// `da_free` of a detached queue.
    fn free_detached(&mut self, old: Self::Detached);
    /// `da_free` of the queue.
    fn free_queue(&mut self);

    /// `encoder_packet_times[track]`.
    fn packet_times(&self, track: usize) -> &[Self::Time];
    fn packet_times_mut(&mut self, track: usize) -> &mut [Self::Time];
    /// `da_push_back_new`, then a copy of `time` into it.
    fn push_packet_time(&mut self, track: usize, time: Self::Time);
    /// `da_erase`.
    fn erase_packet_time(&mut self, track: usize, idx: usize);
    /// `da_pop_front`.
    fn pop_packet_time(&mut self, track: usize);

    /// `keyframe_group_tracking`.
    fn groups(&self) -> &[Self::Group];
    fn groups_mut(&mut self) -> &mut [Self::Group];
    /// `da_erase`.
    fn erase_group(&mut self, idx: usize);
    /// `da_insert`.
    fn insert_group(&mut self, idx: usize, group: Self::Group);

    /// Takes the video track's `caption_mutex`.
    fn lock_caption(&mut self, track: usize);
    fn unlock_caption(&mut self, track: usize);
    /// `display_duration` of `caption_head`, if there is one.
    fn caption_head_duration(&self, track: usize) -> Option<f64>;
    fn caption_timestamp(&self, track: usize) -> f64;
    fn set_caption_timestamp(&mut self, track: usize, ts: f64);
    fn last_caption_timestamp(&self, track: usize) -> f64;
    fn set_last_caption_timestamp(&mut self, track: usize, ts: f64);
    /// `caption_data.size`, the raw CEA-608 bytes waiting.
    fn caption_data_size(&self, track: usize) -> usize;

    /// Takes `pkt_callbacks_mutex`.
    fn lock_callbacks(&mut self);
    fn unlock_callbacks(&mut self);
    /// `pkt_callbacks.num`.
    fn callback_count(&self) -> usize;
    /// Calls packet callback `idx`.
    fn call_callback(
        &mut self,
        idx: usize,
        packet: &mut Self::Packet,
        time: Option<&mut Self::Time>,
    );
    /// `info.encoded_packet(context.data, packet)`.
    fn encoded_packet(&mut self, packet: &mut Self::Packet);
    /// `os_gettime_ns()`.
    fn now(&self) -> u64;

    /// `obs_encoder_packet_create_instance`: a held copy of `src`.
    fn copy_packet(&mut self, src: &Self::Packet) -> Self::Packet;
    /// `obs_encoder_packet_release`, in place (it zeroes the packet).
    fn release(&mut self, packet: &mut Self::Packet);

    /// The debug line `"Sending caption: %f \"%s\""` with the head's text.
    fn log_sending_caption(&self, track: usize, frame_timestamp: f64);
    /// The debug line for a video packet whose timing is not queued.
    fn log_packet_time_not_found(&self, track: usize, pts: i64);
    /// The debug line for a video packet whose track has no timing queued.
    fn log_packet_times_empty(&self, track: usize);
    /// The warning for a grouped encoder that missed a keyframe.
    fn warn_missing_keyframe(&self, pts: i64, encoder: usize, track: usize);
}

// ---------------------------------------------------------------------------
// pure parts, over a slice

/// `packet_dts_usec()` (`obs-internal.h`): `dts` in microseconds, in units
/// of `1 / den` (the numerator is not used).
#[must_use]
pub fn packet_dts_usec(dts: i64, timebase_den: i32) -> i64 {
    todo!()
}

/// `find_first_packet_type_idx()`: the first packet of a track.
#[must_use]
pub fn find_first_packet_type_idx<P: PacketView>(
    packets: &[P],
    kind: Kind,
    track: usize,
) -> Option<usize> {
    todo!()
}

/// `find_last_packet_type_idx()`: the last packet of a track.
#[must_use]
pub fn find_last_packet_type_idx<P: PacketView>(
    packets: &[P],
    kind: Kind,
    track: usize,
) -> Option<usize> {
    todo!()
}

/// `get_interleaved_start_idx()`: where audio and video are closest.
///
/// The audio packet closest in `dts_usec` to the first video packet of
/// track 0 (the earlier one on a tie), or that video packet if it comes
/// first. If the first audio packet at or after that index is a priming
/// packet (`pts <= 0`), the start moves back to the first packet of every
/// audio track that is earlier.
///
/// # Panics
///
/// Without a video packet of track 0 and an audio packet at or after the
/// start, where the C reads through a null or past the end. The
/// interleaver calls it only after the premature prune found both.
#[must_use]
pub fn interleaved_start_idx<P: PacketView>(packets: &[P]) -> usize {
    todo!()
}

/// `get_encoder_duration()`: one audio frame in microseconds.
///
/// Characterized, not endorsed: the time of one sample is truncated to
/// whole microseconds before it is multiplied by the frame size (20480 us,
/// not 21333 us, for 1024 samples at 48 kHz). The product is unsigned in C
/// (`size_t`), then read back as signed.
#[must_use]
pub fn get_encoder_duration(info: EncoderInfo) -> i64 {
    todo!()
}

/// What `prune_premature_packets()` decides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prune {
    /// No video packet of track 0 yet (-1).
    NoVideo,
    /// An audio track with an encoder has no packet yet (-1); the caller
    /// clears `received_audio`.
    MissingAudio,
    /// Start at the start index (0).
    Keep,
    /// Audio starts too late: drop everything before this index (the later
    /// of the first video and the first audio packets, plus one).
    Through(usize),
}

impl Prune {
    /// The C return value: -1, 0 or the index.
    #[must_use]
    pub fn c_value(self) -> i64 {
        todo!()
    }
}

/// The premature prune of `prune_premature_packets()`.
///
/// `audio_encoder(slot)` is the encoder of an audio slot, and
/// `audio_duration(encoder)` its frame duration ([`get_encoder_duration`]);
/// both are asked in slot order, up to the first audio track with no packet.
/// The limit is one frame of the first video packet of track 0, widened to
/// the longest audio frame when more than one audio track runs. It is
/// compared with the difference of the last audio track checked
/// (characterized, not endorsed), strictly.
pub fn premature_prune<P: PacketView>(
    packets: &[P],
    audio_encoder: impl Fn(usize) -> Option<usize>,
    audio_duration: impl Fn(usize) -> i64,
) -> Prune {
    todo!()
}

/// `insert_interleaved_packet()`'s position: after every packet with a
/// lower `dts_usec`. At an equal one, video goes before audio and after
/// video of a lower track; audio goes after everything.
#[must_use]
pub fn insert_position<P: PacketView>(packets: &[P], out: &impl PacketView) -> usize {
    todo!()
}

/// `apply_interleaved_packet_offset()` on the packet: `dts` and `pts` less
/// `offset`, and `dts_usec` recomputed from the new `dts`.
pub fn apply_offset(packet: &mut impl Packet, offset: i64) {
    todo!()
}

/// The highest timestamps `has_higher_opposing_ts()` compares with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Highest {
    /// `highest_video_ts` of each video track with an encoder.
    pub video: [Option<i64>; MAX_OUTPUT_VIDEO_ENCODERS],
    pub audio: i64,
}

impl Highest {
    /// `has_higher_opposing_ts()`: every other video track with an encoder
    /// is past the packet, and for video also the audio. Equal is not past.
    #[must_use]
    pub fn has_higher_opposing(&self, packet: &impl PacketView) -> bool {
        todo!()
    }

    /// `count_streamable_frames()`: the packets from the front that have
    /// the opposing type ahead of them.
    #[must_use]
    pub fn streamable<P: PacketView>(&self, packets: &[P]) -> usize {
        todo!()
    }
}

/// How many packets one call sends once started: none without a
/// streamable one, else one, and a second when more than `max_batch_size`
/// would be left.
#[must_use]
pub fn sends_for(streamable: usize, max_batch_size: usize) -> usize {
    todo!()
}

/// `discard_unused_audio_packets()`'s count: the packets from the front
/// below `dts_usec`, whatever their type.
#[must_use]
pub fn discard_count<P: PacketView>(packets: &[P], dts_usec: i64) -> usize {
    todo!()
}

/// A video packet's time in seconds for the captions:
/// `(pts * num) / (double)den`, the product in integers.
#[must_use]
pub fn frame_timestamp(pts: i64, timebase_num: i32, timebase_den: i32) -> f64 {
    todo!()
}

/// Whether a caption at `caption_timestamp` goes into a frame at
/// `frame_timestamp` (there is one, and it is due).
#[must_use]
pub fn caption_due(has_head: bool, caption_timestamp: f64, frame_timestamp: f64) -> bool {
    todo!()
}

/// Whether raw caption data goes into a frame: there is some, and the
/// frame is later than the last one that got it.
#[must_use]
pub fn raw_caption_due(data_size: usize, last_timestamp: f64, frame_timestamp: f64) -> bool {
    todo!()
}

/// The packet time of a video packet: the first one with its `pts`.
#[must_use]
pub fn find_packet_time<T: PacketTime>(times: &[T], pts: i64) -> Option<usize> {
    todo!()
}

/// The video slot (or audio slot) of the packet's encoder.
#[must_use]
pub fn encoder_slot(
    packet: &impl PacketView,
    video_encoder: impl Fn(usize) -> Option<usize>,
    audio_encoder: impl Fn(usize) -> Option<usize>,
) -> Option<usize> {
    todo!()
}

/// Whether every track of a group entry has had its keyframe (seen or
/// skipped).
#[must_use]
pub fn group_complete(group: &impl KeyframeGroup) -> bool {
    todo!()
}

// ---------------------------------------------------------------------------
// the C functions, on an output

/// `get_encoder_index()`: the packet's encoder slot. A packet of an
/// encoder the output does not have is an `assert(false)` in C, then slot
/// 0 in a build without asserts; here a `debug_assert`, then 0.
pub fn get_encoder_index<O: InterleaveOutput>(o: &O, packet: &impl PacketView) -> usize {
    todo!()
}

/// `check_received()`: the packet's track has started (video per track,
/// one flag for all audio).
pub fn check_received<O: InterleaveOutput>(o: &mut O, packet: &impl PacketView) {
    todo!()
}

/// The offset of the packet's track.
pub fn interleaved_packet_offset<O: InterleaveOutput>(o: &O, packet: &impl PacketView) -> i64 {
    todo!()
}

/// `apply_interleaved_packet_offset()`: the track's offset off the packet
/// (and its packet time), and `dts_usec` from the new `dts`.
pub fn apply_interleaved_packet_offset<O: InterleaveOutput>(
    o: &O,
    packet: &mut O::Packet,
    time: Option<&mut O::Time>,
) {
    todo!()
}

/// The highest timestamps of the output.
pub fn highest<O: InterleaveOutput>(o: &O) -> Highest {
    todo!()
}

/// `has_higher_opposing_ts()`.
pub fn has_higher_opposing_ts<O: InterleaveOutput>(o: &O, packet: &impl PacketView) -> bool {
    todo!()
}

/// `set_higher_ts()`: raises the packet's highest timestamp to it.
pub fn set_higher_ts<O: InterleaveOutput>(o: &mut O, packet: &impl PacketView) {
    todo!()
}

/// `send_interleaved()`: sends the front of the queue. A video packet
/// counts a frame, gets a caption when one is due, and takes its packet
/// time; then the packet callbacks and the output's `encoded_packet` see
/// it, and it is released.
///
/// # Panics
///
/// On an empty queue, where the C reads past the end. The interleaver
/// sends only a packet it counted.
pub fn send_interleaved<O: InterleaveOutput>(o: &mut O) {
    todo!()
}

/// `get_interleaved_start_idx()` on the output's queue.
pub fn get_interleaved_start_idx<O: InterleaveOutput>(o: &O) -> usize {
    todo!()
}

/// `prune_premature_packets()`: [`premature_prune`] on the output's queue
/// and audio encoders; an audio track with no packet clears
/// `received_audio`.
pub fn prune_premature_packets<O: InterleaveOutput>(o: &mut O) -> Prune {
    todo!()
}

/// `discard_to_idx()`: releases the packets before `idx`, with the oldest
/// packet time of each video one's track, and erases them.
///
/// # Panics
///
/// Unless `0 < idx <= len`, where the C asserts.
pub fn discard_to_idx<O: InterleaveOutput>(o: &mut O, idx: usize) {
    todo!()
}

/// `prune_interleaved_packets()`: drops the packets before the start (the
/// premature prune's index, else the start index). False when the prune
/// says to wait.
pub fn prune_interleaved_packets<O: InterleaveOutput>(o: &mut O) -> bool {
    todo!()
}

/// The first packet of each track with an encoder.
type FirstPackets = (
    [Option<usize>; MAX_OUTPUT_VIDEO_ENCODERS],
    [Option<usize>; MAX_OUTPUT_AUDIO_ENCODERS],
);

/// `get_audio_and_video_packets()`: the first packet of each track with an
/// encoder. A video track without one clears its received flag, an audio
/// track without one `received_audio`; `None` then, or with no video
/// encoder at all.
pub fn get_audio_and_video_packets<O: InterleaveOutput>(o: &mut O) -> Option<FirstPackets> {
    todo!()
}

/// `initialize_interleaved_packets()`: the start. Every audio track must
/// reach the first video packet (else `received_audio` is cleared); the
/// packets before the start index go; each video track's offset becomes
/// its first `pts`, each audio track's its first `dts` when that is > 0;
/// `highest_audio_ts` drops by the first audio track's first `dts_usec`;
/// and every queued packet is shifted by its offset.
pub fn initialize_interleaved_packets<O: InterleaveOutput>(o: &mut O) -> bool {
    todo!()
}

/// `insert_interleaved_packet()`: inserts a copy of the packet at its
/// [`insert_position`].
pub fn insert_interleaved_packet<O: InterleaveOutput>(o: &mut O, packet: O::Packet) {
    todo!()
}

/// `resort_interleaved_packets()`: reinserts every packet into a new
/// queue, raising the highest timestamps on the way, and frees the old
/// buffer.
pub fn resort_interleaved_packets<O: InterleaveOutput>(o: &mut O) {
    todo!()
}

/// `discard_unused_audio_packets()`: drops the packets below `dts_usec`
/// ([`discard_count`]).
pub fn discard_unused_audio_packets<O: InterleaveOutput>(o: &mut O, dts_usec: i64) {
    todo!()
}

/// `purge_encoder_group_keyframe_data()`: erases group entry `idx` once
/// every track has had its keyframe ([`group_complete`]).
pub fn purge_encoder_group_keyframe_data<O: InterleaveOutput>(o: &mut O, idx: usize) -> bool {
    todo!()
}

/// `check_encoder_group_keyframe_alignment()`: tracks a grouped video
/// encoder's keyframe. An older entry of its group is marked skipped for
/// the packet's track (with a warning if the track had not seen it), the
/// entry with its pts seen, and without one a new entry is inserted in pts
/// order. Complete entries are erased.
pub fn check_encoder_group_keyframe_alignment<O: InterleaveOutput>(
    o: &mut O,
    packet: &impl PacketView,
) {
    todo!()
}

/// `apply_ept_offsets()`: each video track's offset off its queued packet
/// times.
pub fn apply_ept_offsets<O: InterleaveOutput>(o: &mut O) {
    todo!()
}

/// `count_streamable_frames()`.
pub fn count_streamable_frames<O: InterleaveOutput>(o: &O) -> usize {
    todo!()
}

/// Whether every video track with an encoder has started.
fn all_video_received<O: InterleaveOutput>(o: &O) -> bool {
    todo!()
}

/// `interleave_packets()`: the encoded packet callback.
///
/// An inactive output ignores the packet (characterized, not endorsed: a
/// delayed packet is then not released). The packet's track index is
/// written into it. Under `interleaved_mutex`: a video track that has not
/// had its keyframe drops a non-keyframe and the packets below it
/// (releasing a delayed packet after the unlock); otherwise the keyframe
/// group is checked, the packet is queued (the delay's copy as it is, else
/// a held copy) with its packet time, and once every track has started the
/// output starts (prune, start, resort) or sends what is streamable.
pub fn interleave_packets<O: InterleaveOutput>(
    o: &mut O,
    packet: &mut O::Packet,
    packet_time: Option<&O::Time>,
) {
    todo!()
}

/// `free_packets()`: releases every queued packet and frees the queue.
pub fn free_packets<O: InterleaveOutput>(o: &mut O) {
    todo!()
}

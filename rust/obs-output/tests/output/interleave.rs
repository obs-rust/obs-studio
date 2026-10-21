//! Tier 1 mirror of `test/cmocka/test_output_interleave.c`.
//!
//! The thirty C cases keep their names and their assertions, in order,
//! against the safe core run on a model output ([`Out`]) that records what
//! the C test's stubs record: each sent packet with whether
//! `interleaved_mutex` was held, each packet callback call with its packet
//! time and whether `pkt_callbacks_mutex` was held, and each `add_caption`
//! call with whether the track's caption mutex was held. The edge cases
//! after them are new.
//!
//! The C test's fixture is [`Out::setup`], and its teardown, which frees
//! the queue and checks that every held packet was released
//! (`bnum_allocs()`), is [`Out::teardown`]. A packet's data is one tag
//! byte, as in the C test; a held copy is told from the encoder's packet
//! by its copy number, where the C test compares data pointers. Where the
//! C test reads the struct, this reads the model.
//!
//! The model formats the log lines the way the shim does, so the text
//! assertions say which line the core asked for and with which values;
//! the levels and the `blog` formats themselves are the shim's, and Tier 3
//! compares them with the C.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;

use obs_output::interleave::{
    self, EncoderInfo, Highest, InterleaveOutput, KeyframeGroup, Kind, MAX_OUTPUT_AUDIO_ENCODERS,
    MAX_OUTPUT_VIDEO_ENCODERS, Packet, PacketTime, PacketView, Prune, TrackStatus,
    apply_ept_offsets, apply_interleaved_packet_offset, caption_due,
    check_encoder_group_keyframe_alignment, check_received, discard_count,
    discard_unused_audio_packets, encoder_slot, find_first_packet_type_idx,
    find_last_packet_type_idx, find_packet_time, frame_timestamp, free_packets,
    get_encoder_duration, get_encoder_index, get_interleaved_start_idx, group_complete,
    has_higher_opposing_ts, insert_interleaved_packet, insert_position, interleave_packets,
    interleaved_start_idx, packet_dts_usec, premature_prune, prune_interleaved_packets,
    prune_premature_packets, raw_caption_due, resort_interleaved_packets, send_interleaved,
    sends_for, set_higher_ts,
};

const T: i64 = 1_000_000;
const FRAME_US: i64 = 33333;
const NUM_VIDEO: usize = 2;
const NUM_AUDIO: usize = 2;
const LOG_WARNING: i32 = 200;
const LOG_DEBUG: i32 = 400;

/// The test's encoders, as identities.
const VENC: [usize; NUM_VIDEO] = [0x100, 0x101];
const AENC: [usize; NUM_AUDIO] = [0x200, 0x201];
/// The encoder group of the group case.
const GROUP: usize = 0x900;

/// `struct encoder_packet`, with a one-byte tag for its data. `copy` is 0
/// for the encoder's own data, else the number of the held copy.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Pkt {
    kind: Kind,
    track: usize,
    dts: i64,
    pts: i64,
    dts_usec: i64,
    keyframe: bool,
    num: i32,
    den: i32,
    encoder: usize,
    tag: u8,
    size: usize,
    copy: usize,
}

impl Pkt {
    /// What `obs_encoder_packet_release` leaves: a zeroed struct.
    fn zeroed() -> Self {
        Self {
            kind: Kind::Audio,
            track: 0,
            dts: 0,
            pts: 0,
            dts_usec: 0,
            keyframe: false,
            num: 0,
            den: 0,
            encoder: 0,
            tag: 0,
            size: 0,
            copy: 0,
        }
    }
}

impl PacketView for Pkt {
    fn kind(&self) -> Kind {
        self.kind
    }
    fn track(&self) -> usize {
        self.track
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
        self.num
    }
    fn timebase_den(&self) -> i32 {
        self.den
    }
    fn encoder(&self) -> usize {
        self.encoder
    }
}

impl Packet for Pkt {
    fn set_track(&mut self, track: usize) {
        self.track = track;
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

/// `struct encoder_packet_time`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Time {
    pts: i64,
    cts: u64,
    fer: u64,
    ferc: u64,
    pir: u64,
}

impl PacketTime for Time {
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

/// `struct keyframe_group_data`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Group {
    group_id: usize,
    pts: i64,
    required_tracks: u32,
    seen_on_track: [TrackStatus; MAX_OUTPUT_VIDEO_ENCODERS],
}

impl KeyframeGroup for Group {
    fn new(group_id: usize, pts: i64, track: usize, required_tracks: u32) -> Self {
        let mut seen_on_track = [TrackStatus::NotSeen; MAX_OUTPUT_VIDEO_ENCODERS];
        seen_on_track[track] = TrackStatus::Seen;
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
        self.seen_on_track[track]
    }
    fn set_seen(&mut self, track: usize, status: TrackStatus) {
        self.seen_on_track[track] = status;
    }
}

/// An encoder (`struct obs_encoder`) as the interleaver sees it.
#[derive(Debug, Clone)]
struct Enc {
    id: usize,
    name: &'static str,
    info: EncoderInfo,
}

/// `struct caption_track_data`.
#[derive(Debug, Clone, Default)]
struct CaptionTrack {
    /// `caption_head`'s display duration and text.
    head: Option<(f64, String)>,
    timestamp: f64,
    last_timestamp: f64,
    data_size: usize,
    locked: bool,
}

/// `struct sent` of the C test.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Sent {
    kind: Kind,
    track_idx: usize,
    dts: i64,
    pts: i64,
    dts_usec: i64,
    keyframe: bool,
    tag: u8,
    /// `interleaved_mutex` was held when `encoded_packet()` ran.
    locked: bool,
}

/// `struct callback_call`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CallbackCall {
    tag: u8,
    time_found: bool,
    time: Time,
    callbacks_locked: bool,
}

/// `struct caption_call`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CaptionCall {
    track_idx: usize,
    locked: bool,
}

#[derive(Debug, Default)]
struct Log {
    last: String,
    level: i32,
    count: usize,
    warnings: usize,
}

/// The output of the C test: the interleaver fields of `struct obs_output`,
/// its encoders, the stub results and the recorded calls.
#[derive(Debug)]
struct Out {
    name: &'static str,
    active: bool,
    active_delay_ns: u64,
    video_encoders: [Option<usize>; MAX_OUTPUT_VIDEO_ENCODERS],
    audio_encoders: [Option<usize>; MAX_OUTPUT_AUDIO_ENCODERS],
    encoders: Vec<Enc>,
    num_encoders_started: u32,
    received_video: [bool; MAX_OUTPUT_VIDEO_ENCODERS],
    received_audio: bool,
    video_offsets: [i64; MAX_OUTPUT_VIDEO_ENCODERS],
    audio_offsets: [i64; MAX_OUTPUT_AUDIO_ENCODERS],
    highest_audio_ts: i64,
    highest_video_ts: [i64; MAX_OUTPUT_VIDEO_ENCODERS],
    queue: Vec<Pkt>,
    /// The queue's buffer (a number per allocation), `None` when freed.
    buffer: Option<usize>,
    buffers: usize,
    packet_times: [Vec<Time>; MAX_OUTPUT_VIDEO_ENCODERS],
    groups: Vec<Group>,
    captions: [CaptionTrack; MAX_OUTPUT_VIDEO_ENCODERS],
    callbacks: usize,
    total_frames: i32,
    max_batch_size: usize,
    add_caption_result: bool,

    locked: bool,
    lock_calls: usize,
    callbacks_locked: bool,
    sent: Vec<Sent>,
    callback_calls: Vec<CallbackCall>,
    caption_calls: Vec<CaptionCall>,
    /// Whether `interleaved_mutex` was held at each release.
    releases_locked: Vec<bool>,
    /// Held copies not yet released (`bnum_allocs()` in the C test).
    live: HashSet<usize>,
    copies: usize,
    clock: Cell<u64>,
    log: RefCell<Log>,
}

impl Out {
    /// `setup()`.
    fn setup() -> Self {
        let mut encoders = Vec::new();
        let video_names = ["video 0", "video 1"];
        let audio_names = ["audio 0", "audio 1"];
        for i in 0..NUM_VIDEO {
            encoders.push(Enc {
                id: VENC[i],
                name: video_names[i],
                info: EncoderInfo {
                    timebase_num: 1,
                    timebase_den: 30,
                    framesize: 0,
                    group: None,
                },
            });
        }
        for i in 0..NUM_AUDIO {
            encoders.push(Enc {
                id: AENC[i],
                name: audio_names[i],
                info: EncoderInfo {
                    timebase_num: 1,
                    timebase_den: 48000,
                    framesize: 1024,
                    group: None,
                },
            });
        }
        let mut o = Self {
            name: "interleaved output",
            active: true,
            active_delay_ns: 0,
            video_encoders: [None; MAX_OUTPUT_VIDEO_ENCODERS],
            audio_encoders: [None; MAX_OUTPUT_AUDIO_ENCODERS],
            encoders,
            num_encoders_started: 0,
            received_video: [false; MAX_OUTPUT_VIDEO_ENCODERS],
            received_audio: false,
            video_offsets: [0; MAX_OUTPUT_VIDEO_ENCODERS],
            audio_offsets: [0; MAX_OUTPUT_AUDIO_ENCODERS],
            highest_audio_ts: 0,
            // What reset_packet_data() in obs-output.c leaves at each start.
            highest_video_ts: [i64::MIN; MAX_OUTPUT_VIDEO_ENCODERS],
            queue: Vec::new(),
            buffer: None,
            buffers: 0,
            packet_times: Default::default(),
            groups: Vec::new(),
            captions: Default::default(),
            callbacks: 1,
            total_frames: 0,
            max_batch_size: 8,
            add_caption_result: true,
            locked: false,
            lock_calls: 0,
            callbacks_locked: false,
            sent: Vec::new(),
            callback_calls: Vec::new(),
            caption_calls: Vec::new(),
            releases_locked: Vec::new(),
            live: HashSet::new(),
            copies: 0,
            clock: Cell::new(1),
            log: RefCell::new(Log {
                last: String::new(),
                level: -1,
                count: 0,
                warnings: 0,
            }),
        };
        // One video and one audio encoder unless a case adds more.
        o.video_encoders[0] = Some(VENC[0]);
        o.audio_encoders[0] = Some(AENC[0]);
        o
    }

    /// `teardown()`: every packet the interleaver copied has been released.
    fn teardown(mut self) {
        free_packets(&mut self);
        assert!(!self.locked);
        assert!(!self.callbacks_locked);
        assert!(self.captions.iter().all(|c| !c.locked));
        assert!(self.live.is_empty(), "every packet copy is released");
    }

    fn use_tracks(&mut self, num_video: usize, num_audio: usize) {
        for (i, (slot, enc)) in self.video_encoders.iter_mut().zip(VENC).enumerate() {
            *slot = (i < num_video).then_some(enc);
        }
        for (i, (slot, enc)) in self.audio_encoders.iter_mut().zip(AENC).enumerate() {
            *slot = (i < num_audio).then_some(enc);
        }
    }

    fn enc(&self, id: usize) -> &Enc {
        self.encoders
            .iter()
            .find(|e| e.id == id)
            .expect("a test encoder")
    }

    fn enc_mut(&mut self, id: usize) -> &mut Enc {
        self.encoders
            .iter_mut()
            .find(|e| e.id == id)
            .expect("a test encoder")
    }

    fn logged(&self, level: i32, text: String) {
        let mut log = self.log.borrow_mut();
        log.last = text;
        log.level = level;
        log.count += 1;
        if level == LOG_WARNING {
            log.warnings += 1;
        }
    }

    fn ensure_buffer(&mut self) {
        if self.buffer.is_none() {
            self.buffers += 1;
            self.buffer = Some(self.buffers);
        }
    }

    // the C test's helpers

    /// An encoder's video packet: data the encoder owns, `track_idx` 99.
    fn video_packet(
        track: usize,
        dts: i64,
        pts: i64,
        dts_usec: i64,
        keyframe: bool,
        tag: u8,
    ) -> Pkt {
        Pkt {
            kind: Kind::Video,
            track: 99,
            dts,
            pts,
            dts_usec,
            keyframe,
            num: 1,
            den: 30,
            encoder: VENC[track],
            tag,
            size: 1,
            copy: 0,
        }
    }

    fn audio_packet(track: usize, dts: i64, dts_usec: i64, tag: u8) -> Pkt {
        Pkt {
            kind: Kind::Audio,
            track: 99,
            dts,
            pts: dts,
            dts_usec,
            keyframe: false,
            num: 1,
            den: 48000,
            encoder: AENC[track],
            tag,
            size: 1,
            copy: 0,
        }
    }

    /// An encoder's video packet comes with its packet time; cts carries
    /// the tag.
    fn feed_video(
        &mut self,
        track: usize,
        dts: i64,
        pts: i64,
        dts_usec: i64,
        keyframe: bool,
        tag: u8,
    ) {
        let mut pkt = Self::video_packet(track, dts, pts, dts_usec, keyframe, tag);
        let ept = Time {
            pts,
            cts: u64::from(tag),
            ..Time::default()
        };
        interleave_packets(self, &mut pkt, Some(&ept));
        // the caller's packet is copied, not taken
        assert_eq!(pkt.copy, 0);
        assert_eq!(pkt.tag, tag);
        assert_eq!(pkt.track, track);
    }

    fn feed_audio(&mut self, track: usize, dts: i64, dts_usec: i64, tag: u8) {
        let mut pkt = Self::audio_packet(track, dts, dts_usec, tag);
        interleave_packets(self, &mut pkt, None);
        assert_eq!(pkt.copy, 0);
        assert_eq!(pkt.tag, tag);
        assert_eq!(pkt.track, track);
    }

    /// Queue a copy directly, as the interleaver would have.
    fn queue_packet(&mut self, mut pkt: Pkt, track: usize) {
        pkt.track = track;
        let copy = self.copy_packet(&pkt);
        self.ensure_buffer();
        self.queue.push(copy);
    }

    fn push_packet_time(&mut self, track: usize, pts: i64, cts: u64) {
        self.packet_times[track].push(Time {
            pts,
            cts,
            ..Time::default()
        });
    }

    fn queued(&self) -> usize {
        self.queue.len()
    }

    fn queued_at(&mut self, idx: usize) -> &mut Pkt {
        assert!(idx < self.queue.len());
        &mut self.queue[idx]
    }

    fn tag_at(&self, idx: usize) -> u8 {
        self.queue[idx].tag
    }

    fn assert_queue_tags(&self, expected: &[u8]) {
        assert_eq!(self.queued(), expected.len());
        let tags: Vec<u8> = (0..expected.len()).map(|i| self.tag_at(i)).collect();
        assert_eq!(tags, expected);
    }

    fn num_sent(&self) -> usize {
        self.sent.len()
    }

    fn last_log(&self) -> String {
        self.log.borrow().last.clone()
    }
}

impl InterleaveOutput for Out {
    type Packet = Pkt;
    type Time = Time;
    type Group = Group;
    type Detached = Vec<Pkt>;

    fn active(&self) -> bool {
        self.active
    }

    fn first_audio_encoder_index(&self) -> Option<usize> {
        self.audio_encoders.iter().position(Option::is_some)
    }

    fn first_video_encoder_index(&self) -> Option<usize> {
        self.video_encoders.iter().position(Option::is_some)
    }

    fn add_caption(&mut self, packet: &mut Pkt) -> bool {
        assert!(self.caption_calls.len() < 64);
        let locked = self.captions[packet.track].locked;
        self.caption_calls.push(CaptionCall {
            track_idx: packet.track,
            locked,
        });
        self.add_caption_result
    }

    fn video_encoder(&self, slot: usize) -> Option<usize> {
        self.video_encoders[slot]
    }

    fn audio_encoder(&self, slot: usize) -> Option<usize> {
        self.audio_encoders[slot]
    }

    fn encoder_info(&self, encoder: usize) -> EncoderInfo {
        self.enc(encoder).info
    }

    fn group_num_started(&self, group: usize) -> u32 {
        assert_eq!(group, GROUP);
        self.num_encoders_started
    }

    fn lock(&mut self) {
        assert!(!self.locked, "interleaved_mutex is not recursive");
        self.locked = true;
        self.lock_calls += 1;
    }

    fn unlock(&mut self) {
        assert!(self.locked);
        self.locked = false;
    }

    fn received_video(&self, track: usize) -> bool {
        self.received_video[track]
    }
    fn set_received_video(&mut self, track: usize, received: bool) {
        self.received_video[track] = received;
    }
    fn received_audio(&self) -> bool {
        self.received_audio
    }
    fn set_received_audio(&mut self, received: bool) {
        self.received_audio = received;
    }
    fn video_offset(&self, track: usize) -> i64 {
        self.video_offsets[track]
    }
    fn set_video_offset(&mut self, track: usize, offset: i64) {
        self.video_offsets[track] = offset;
    }
    fn audio_offset(&self, track: usize) -> i64 {
        self.audio_offsets[track]
    }
    fn set_audio_offset(&mut self, track: usize, offset: i64) {
        self.audio_offsets[track] = offset;
    }
    fn highest_audio_ts(&self) -> i64 {
        self.highest_audio_ts
    }
    fn set_highest_audio_ts(&mut self, ts: i64) {
        self.highest_audio_ts = ts;
    }
    fn highest_video_ts(&self, track: usize) -> i64 {
        self.highest_video_ts[track]
    }
    fn set_highest_video_ts(&mut self, track: usize, ts: i64) {
        self.highest_video_ts[track] = ts;
    }
    fn active_delay_ns(&self) -> u64 {
        self.active_delay_ns
    }
    fn max_batch_size(&self) -> usize {
        self.max_batch_size
    }
    fn inc_total_frames(&mut self) {
        self.total_frames += 1;
    }

    fn packets(&self) -> &[Pkt] {
        &self.queue
    }
    fn packets_mut(&mut self) -> &mut [Pkt] {
        &mut self.queue
    }
    fn insert_packet(&mut self, idx: usize, packet: Pkt) {
        self.ensure_buffer();
        self.queue.insert(idx, packet);
    }
    fn erase_front(&mut self, end: usize) {
        assert!(end > 0 && end <= self.queue.len());
        self.queue.drain(..end);
    }
    fn detach_packets(&mut self) -> Vec<Pkt> {
        self.buffer = None;
        std::mem::take(&mut self.queue)
    }
    fn free_detached(&mut self, _old: Vec<Pkt>) {}
    fn free_queue(&mut self) {
        self.queue.clear();
        self.buffer = None;
    }

    fn packet_times(&self, track: usize) -> &[Time] {
        &self.packet_times[track]
    }
    fn packet_times_mut(&mut self, track: usize) -> &mut [Time] {
        &mut self.packet_times[track]
    }
    fn push_packet_time(&mut self, track: usize, time: Time) {
        self.packet_times[track].push(time);
    }
    fn erase_packet_time(&mut self, track: usize, idx: usize) {
        self.packet_times[track].remove(idx);
    }
    fn pop_packet_time(&mut self, track: usize) {
        if !self.packet_times[track].is_empty() {
            self.packet_times[track].remove(0);
        }
    }

    fn groups(&self) -> &[Group] {
        &self.groups
    }
    fn groups_mut(&mut self) -> &mut [Group] {
        &mut self.groups
    }
    fn erase_group(&mut self, idx: usize) {
        self.groups.remove(idx);
    }
    fn insert_group(&mut self, idx: usize, group: Group) {
        self.groups.insert(idx, group);
    }

    fn lock_caption(&mut self, track: usize) {
        assert!(!self.captions[track].locked);
        self.captions[track].locked = true;
    }
    fn unlock_caption(&mut self, track: usize) {
        assert!(self.captions[track].locked);
        self.captions[track].locked = false;
    }
    fn caption_head_duration(&self, track: usize) -> Option<f64> {
        self.captions[track].head.as_ref().map(|h| h.0)
    }
    fn caption_timestamp(&self, track: usize) -> f64 {
        self.captions[track].timestamp
    }
    fn set_caption_timestamp(&mut self, track: usize, ts: f64) {
        self.captions[track].timestamp = ts;
    }
    fn last_caption_timestamp(&self, track: usize) -> f64 {
        self.captions[track].last_timestamp
    }
    fn set_last_caption_timestamp(&mut self, track: usize, ts: f64) {
        self.captions[track].last_timestamp = ts;
    }
    fn caption_data_size(&self, track: usize) -> usize {
        self.captions[track].data_size
    }

    fn lock_callbacks(&mut self) {
        assert!(!self.callbacks_locked);
        self.callbacks_locked = true;
    }
    fn unlock_callbacks(&mut self) {
        assert!(self.callbacks_locked);
        self.callbacks_locked = false;
    }
    fn callback_count(&self) -> usize {
        self.callbacks
    }
    fn call_callback(&mut self, idx: usize, packet: &mut Pkt, time: Option<&mut Time>) {
        assert!(idx < self.callbacks);
        assert!(self.callback_calls.len() < 64);
        self.callback_calls.push(CallbackCall {
            tag: if packet.size != 0 { packet.tag } else { 0 },
            time_found: time.is_some(),
            time: time.map(|t| *t).unwrap_or_default(),
            callbacks_locked: self.callbacks_locked,
        });
    }
    fn encoded_packet(&mut self, packet: &mut Pkt) {
        assert!(self.sent.len() < 64);
        self.sent.push(Sent {
            kind: packet.kind,
            track_idx: packet.track,
            dts: packet.dts,
            pts: packet.pts,
            dts_usec: packet.dts_usec,
            keyframe: packet.keyframe,
            tag: if packet.size != 0 { packet.tag } else { 0 },
            locked: self.locked,
        });
    }
    fn now(&self) -> u64 {
        let now = self.clock.get();
        self.clock.set(now + 1);
        now
    }

    fn copy_packet(&mut self, src: &Pkt) -> Pkt {
        self.copies += 1;
        self.live.insert(self.copies);
        Pkt {
            copy: self.copies,
            ..src.clone()
        }
    }

    fn release(&mut self, packet: &mut Pkt) {
        if packet.copy != 0 {
            assert!(self.live.remove(&packet.copy), "a copy is released once");
        }
        self.releases_locked.push(self.locked);
        *packet = Pkt::zeroed();
    }

    fn log_sending_caption(&self, track: usize, frame_timestamp: f64) {
        let text = self.captions[track]
            .head
            .as_ref()
            .map(|h| h.1.clone())
            .unwrap_or_default();
        self.logged(
            LOG_DEBUG,
            format!("Sending caption: {frame_timestamp:.6} \"{text}\""),
        );
    }

    fn log_packet_time_not_found(&self, track: usize, pts: i64) {
        self.logged(
            LOG_DEBUG,
            format!(
                "send_interleaved: Track {track} encoder packet timing for PTS{pts} not found."
            ),
        );
    }

    fn log_packet_times_empty(&self, track: usize) {
        self.logged(
            LOG_DEBUG,
            format!("send_interleaved: Track {track} encoder packet timing array empty."),
        );
    }

    fn warn_missing_keyframe(&self, pts: i64, encoder: usize, track: usize) {
        let name = self.enc(encoder).name;
        self.logged(
            LOG_WARNING,
            format!(
                "obs-output '{}': Missing keyframe with pts {pts} for encoder '{name}' (track: {track})",
                self.name
            ),
        );
    }
}

fn video_packet(track: usize, dts: i64, pts: i64, dts_usec: i64, keyframe: bool, tag: u8) -> Pkt {
    Out::video_packet(track, dts, pts, dts_usec, keyframe, tag)
}

fn audio_packet(track: usize, dts: i64, dts_usec: i64, tag: u8) -> Pkt {
    Out::audio_packet(track, dts, dts_usec, tag)
}

// ---------------------------------------------------------------------------
// encoder index and received flags

#[test]
fn encoder_index_is_the_encoder_slot() {
    let mut out = Out::setup();
    out.use_tracks(2, 2);

    let v1 = video_packet(1, 0, 0, T, true, 1);
    let v0 = video_packet(0, 0, 0, T, true, 2);
    let a1 = audio_packet(1, 0, T, 3);
    let a0 = audio_packet(0, 0, T, 4);
    assert_eq!(get_encoder_index(&out, &v1), 1);
    assert_eq!(get_encoder_index(&out, &v0), 0);
    assert_eq!(get_encoder_index(&out, &a1), 1);
    assert_eq!(get_encoder_index(&out, &a0), 0);

    // The type picks the array: an audio encoder's packet marked video is
    // not found. That is an assert(false), then index 0 in a build without
    // asserts.
    let mut stray = audio_packet(1, 0, T, 5);
    stray.kind = Kind::Video;
    assert_eq!(
        encoder_slot(&stray, |i| out.video_encoders[i], |i| out.audio_encoders[i]),
        None
    );
    #[cfg(not(debug_assertions))]
    assert_eq!(get_encoder_index(&out, &stray), 0);

    // interleave_packets() writes the index into the caller's packet
    out.feed_audio(1, 0, T, 6);
    assert_eq!(out.queued_at(0).track, 1);
    out.teardown();
}

#[test]
fn received_flags_per_video_track_one_for_audio() {
    let mut out = Out::setup();
    out.use_tracks(2, 2);

    let mut v1 = video_packet(1, 0, 0, T, true, 1);
    v1.track = 1;
    check_received(&mut out, &v1);
    assert!(!out.received_video[0]);
    assert!(out.received_video[1]);
    assert!(!out.received_audio);

    // any audio track sets the one audio flag
    let mut a1 = audio_packet(1, 0, T, 2);
    a1.track = 1;
    check_received(&mut out, &a1);
    assert!(out.received_audio);
    assert!(!out.received_video[0]);

    // Through interleave_packets(): each packet sets its flag before the
    // start, and nothing is sent while one video track has nothing.
    out.received_video = [false; MAX_OUTPUT_VIDEO_ENCODERS];
    out.received_audio = false;
    out.feed_audio(0, 48000, T + 1000, 3);
    assert!(out.received_audio);
    out.feed_video(1, 30, 30, T, true, 4);
    assert!(out.received_video[1]);
    assert!(!out.received_video[0]);
    assert_eq!(out.num_sent(), 0);
    assert_eq!(out.queued(), 2);
    out.teardown();
}

#[test]
fn inactive_output_ignores_the_packet() {
    let mut out = Out::setup();
    out.active = false;

    let mut pkt = video_packet(0, 30, 30, T, true, 1);
    interleave_packets(&mut out, &mut pkt, None);
    // not even the index is set
    assert_eq!(pkt.track, 99);
    assert_eq!(out.queued(), 0);
    assert!(!out.received_video[0]);

    // Characterized, not endorsed: with a delay running, the delay hands the
    // callback a copy that the callback owns (process_delay()), and this
    // early return does not release it. The test releases it.
    out.active_delay_ns = 1;
    let mut held = out.copy_packet(&pkt);
    let allocs = out.live.len();
    interleave_packets(&mut out, &mut held, None);
    assert_eq!(out.live.len(), allocs);
    out.release(&mut held);
    out.teardown();
}

// ---------------------------------------------------------------------------
// keyframe wait

#[test]
fn video_waits_for_a_keyframe() {
    let mut out = Out::setup();

    out.feed_audio(0, 48000, T - 20000, 1);
    out.feed_audio(0, 49024, T + 1333, 2);
    assert!(out.received_audio);

    // A first video packet that is not a keyframe is dropped, and so is
    // every queued packet with a lower dts_usec (here the first audio).
    // The packet at or after it stays.
    out.feed_video(0, 30, 30, T, false, 3);
    assert!(!out.received_video[0]);
    out.assert_queue_tags(&[2]);
    assert_eq!(out.packet_times[0].len(), 0);
    assert!(out.received_audio);

    // The keyframe is queued and sets the flag.
    out.feed_video(0, 31, 31, T + FRAME_US, true, 4);
    assert!(out.received_video[0]);
    assert_eq!(out.num_sent(), 0);
    out.assert_queue_tags(&[2, 4]);
    // its packet time is kept
    assert_eq!(out.packet_times[0].len(), 1);

    // The start failed (no audio past the keyframe yet), but once a track
    // has its keyframe, later non-keyframes are queued.
    assert!(!out.received_audio);
    out.feed_video(0, 32, 32, T + 2 * FRAME_US, false, 5);
    out.assert_queue_tags(&[2, 4, 5]);
    out.teardown();
}

#[test]
fn keyframe_wait_discards_other_tracks() {
    let mut out = Out::setup();
    out.use_tracks(2, 1);

    // Track 1 has its keyframe, track 0 has not.
    out.feed_video(1, 30, 30, T, true, 1);
    out.feed_audio(0, 48000, T + 1000, 2);
    assert!(out.received_video[1]);
    assert_eq!(out.packet_times[1].len(), 1);

    // Characterized, not endorsed: track 0's non-keyframe discards every
    // packet below its dts_usec, track 1's keyframe and its packet time
    // included, but track 1's received flag stays set.
    out.feed_video(0, 31, 31, T + 2000, false, 3);
    assert_eq!(out.queued(), 0);
    assert_eq!(out.packet_times[1].len(), 0);
    assert!(out.received_video[1]);
    assert!(!out.received_video[0]);
    assert_eq!(out.num_sent(), 0);
    out.teardown();
}

#[test]
fn delayed_packets_are_taken_not_copied() {
    let mut out = Out::setup();
    // With a delay running, the packet is the delay's copy and the callback
    // owns it (process_delay()).
    out.active_delay_ns = 1;

    let src = audio_packet(0, 48000, T, 1);
    let mut held = out.copy_packet(&src);
    interleave_packets(&mut out, &mut held, None);
    // queued as it is, no second copy
    assert_eq!(out.queued_at(0).copy, held.copy);

    // a non-keyframe that waits is released
    let src = video_packet(0, 30, 30, T + 1000, false, 2);
    let mut held = out.copy_packet(&src);
    interleave_packets(&mut out, &mut held, None);
    // it discarded the queued audio below it too
    assert_eq!(out.queued(), 0);
    // teardown's live check shows both were released
    out.teardown();
}

// ---------------------------------------------------------------------------
// start index

#[test]
fn start_index_is_the_audio_closest_to_video() {
    let mut out = Out::setup();

    out.queue_packet(audio_packet(0, 100, T - 100_000, 1), 0);
    out.queue_packet(audio_packet(0, 200, T - 50000, 2), 0);
    out.queue_packet(audio_packet(0, 300, T - 10000, 3), 0);
    out.queue_packet(video_packet(0, 30, 30, T, true, 4), 0);
    out.queue_packet(audio_packet(0, 400, T + 10000, 5), 0);

    // closest by |dts_usec - first video|; on a tie the earlier one
    assert_eq!(get_interleaved_start_idx(&out), 2);

    // the video packet when it comes first
    free_packets(&mut out);
    out.queue_packet(video_packet(0, 30, 30, T, true, 6), 0);
    out.queue_packet(audio_packet(0, 100, T + 100_000, 7), 0);
    assert_eq!(get_interleaved_start_idx(&out), 0);
    out.teardown();
}

#[test]
fn start_index_keeps_priming_audio() {
    let mut out = Out::setup();
    out.use_tracks(1, 2);

    // Early AAC/Opus packets prime the encoder and have pts <= 0. If the
    // first audio at the start index has pts <= 0, the start moves back to
    // the first audio packet of every track.
    out.queue_packet(audio_packet(0, 1024, T - 100_000, 1), 0);
    out.queue_packet(audio_packet(1, -1024, T - 50000, 2), 1);
    out.queue_packet(audio_packet(1, 0, T - 10000, 3), 1);
    out.queue_packet(video_packet(0, 30, 30, T, true, 4), 0);
    out.queue_packet(audio_packet(0, 2048, T + 10000, 5), 0);

    // Characterized, not endorsed: track 1's priming packet also keeps
    // track 0's packet from 100 ms before the video, whose pts is > 0.
    assert_eq!(get_interleaved_start_idx(&out), 0);

    // without priming, the closest audio (index 2)
    out.queued_at(2).pts = 1;
    assert_eq!(get_interleaved_start_idx(&out), 2);
    out.teardown();
}

#[test]
fn start_index_follows_video_track_0_only() {
    let mut out = Out::setup();
    out.use_tracks(2, 1);

    out.queue_packet(video_packet(1, 30, 30, T - 10000, true, 1), 1);
    out.queue_packet(audio_packet(0, 100, T - 5000, 2), 0);
    out.queue_packet(video_packet(0, 30, 30, T, true, 3), 0);
    out.queue_packet(audio_packet(0, 200, T + 5000, 4), 0);

    // Characterized, not endorsed: only track 0's first video counts, so
    // track 1's keyframe before the start index would be discarded.
    assert_eq!(get_interleaved_start_idx(&out), 1);
    out.teardown();
}

// ---------------------------------------------------------------------------
// premature prune

#[test]
fn premature_audio_prunes_through_it() {
    let mut out = Out::setup();

    // Audio that starts more than one video frame after the first video
    // packet: prune through the later of the two (index + 1).
    out.queue_packet(video_packet(0, 30, 30, T, true, 1), 0);
    out.queue_packet(audio_packet(0, 48000, T + 40000, 2), 0);
    assert_eq!(prune_premature_packets(&mut out).c_value(), 2);

    // exactly one frame (33333 us) is not pruned: strict >
    out.queued_at(1).dts_usec = T + FRAME_US;
    assert_eq!(prune_premature_packets(&mut out).c_value(), 0);

    // audio before the video is not pruned either
    out.queued_at(1).dts_usec = T - 40000;
    assert_eq!(prune_premature_packets(&mut out).c_value(), 0);
    out.teardown();
}

#[test]
fn premature_prune_needs_every_track() {
    let mut out = Out::setup();
    out.use_tracks(1, 2);

    // no video on track 0: -1, flags untouched
    out.queue_packet(audio_packet(0, 48000, T, 1), 0);
    out.received_audio = true;
    assert_eq!(prune_premature_packets(&mut out).c_value(), -1);
    assert!(out.received_audio);

    // an audio track with nothing queued: -1 and received_audio cleared
    out.queue_packet(video_packet(0, 30, 30, T, true, 2), 0);
    assert_eq!(prune_premature_packets(&mut out).c_value(), -1);
    assert!(!out.received_audio);

    // prune_interleaved_packets() then does nothing
    out.received_audio = true;
    assert!(!prune_interleaved_packets(&mut out));
    assert_eq!(out.queued(), 2);
    out.teardown();
}

#[test]
fn premature_prune_uses_the_last_audio_track() {
    let mut out = Out::setup();
    out.use_tracks(1, 2);

    out.queue_packet(video_packet(0, 30, 30, T, true, 1), 0);
    out.queue_packet(audio_packet(1, 48000, T + 1000, 2), 1);
    out.queue_packet(audio_packet(0, 48000, T + 40000, 3), 0);

    // Characterized, not endorsed: the decision uses the difference of the
    // last audio track checked (track 1, 1 ms), not the largest one (track
    // 0, 40 ms, computed as max_diff but not used), so this is not pruned.
    assert_eq!(prune_premature_packets(&mut out).c_value(), 0);

    // with the late audio on track 1 it is, through the latest first audio
    out.queued_at(1).dts_usec = T + 40000;
    out.queued_at(2).dts_usec = T + 1000;
    assert_eq!(prune_premature_packets(&mut out).c_value(), 3);
    out.teardown();
}

#[test]
fn premature_prune_widens_to_the_audio_frame() {
    let mut out = Out::setup();

    // 60 fps video: one frame is 16666 us.
    let mut v = video_packet(0, 60, 60, T, true, 1);
    v.den = 60;
    out.queue_packet(v, 0);
    out.queue_packet(audio_packet(0, 48000, T + 18000, 2), 0);

    // one audio track: 18 ms > 16.7 ms, pruned
    assert_eq!(prune_premature_packets(&mut out).c_value(), 2);

    // With more than one audio track, the limit widens to the longest audio
    // frame, get_encoder_duration(). Characterized, not endorsed: that
    // truncates 1e6 / 48000 to 20 before multiplying by the 1024-sample
    // frame, so the limit is 20480 us, not 21333 us.
    out.use_tracks(1, 2);
    out.queue_packet(audio_packet(1, 48000, T + 18000, 3), 1);
    assert_eq!(get_encoder_duration(out.encoder_info(AENC[1])), 20480);
    assert_eq!(prune_premature_packets(&mut out).c_value(), 0);
    out.queued_at(2).dts_usec = T + 21000;
    assert_eq!(prune_premature_packets(&mut out).c_value(), 3);
    out.queued_at(2).dts_usec = T + 20480;
    assert_eq!(prune_premature_packets(&mut out).c_value(), 0);
    out.teardown();
}

#[test]
fn late_audio_drops_the_first_keyframe() {
    let mut out = Out::setup();

    // The case a delayed stream start hits: the first audio packet comes
    // more than one frame after the keyframe.
    out.feed_video(0, 30, 30, T, true, 1);
    out.feed_audio(0, 48000, T + 40000, 2);

    // prune_start is 2: the keyframe, its packet time and the audio go.
    // There is then no video, so the start fails and track 0 waits for a
    // keyframe again. Nothing is sent.
    assert_eq!(out.queued(), 0);
    assert_eq!(out.packet_times[0].len(), 0);
    assert!(!out.received_video[0]);
    assert!(out.received_audio);
    assert_eq!(out.num_sent(), 0);

    // The next frames are not keyframes: dropped, and each takes the audio
    // queued below it.
    out.feed_video(0, 31, 31, T + FRAME_US, false, 3);
    out.feed_audio(0, 49024, T + 61333, 4);
    out.assert_queue_tags(&[4]);
    out.feed_video(0, 32, 32, T + 2 * FRAME_US, false, 5);
    assert_eq!(out.queued(), 0);
    assert_eq!(out.num_sent(), 0);

    // Characterized, not endorsed: the output starts at the next keyframe,
    // here one second later; everything before it is lost. With no audio
    // queued at that keyframe, the start waits for audio.
    out.feed_video(0, 60, 60, T + 1_000_000, true, 6);
    assert!(!out.received_audio);
    assert_eq!(out.num_sent(), 0);
    out.feed_audio(0, 96000, T + 1_010_000, 7);
    assert_eq!(out.num_sent(), 1);
    assert_eq!(out.sent[0].tag, 6);
    assert!(out.sent[0].keyframe);
    assert_eq!(out.sent[0].dts, 0);
    out.teardown();
}

#[test]
fn premature_prune_can_start_on_a_non_keyframe() {
    let mut out = Out::setup();
    out.use_tracks(2, 1);

    // Track 1 has no keyframe yet, so packets queue up: track 0's keyframe
    // and the frames after it, and audio from 40 ms after the keyframe.
    out.feed_video(0, 30, 30, T, true, 1);
    out.feed_audio(0, 48000, T + 40000, 2);
    out.feed_video(0, 31, 31, T + FRAME_US, false, 3);
    out.feed_audio(0, 49024, T + 50000, 4);
    out.feed_video(0, 32, 32, T + 2 * FRAME_US, false, 5);
    out.feed_audio(0, 50048, T + 70000, 6);
    out.assert_queue_tags(&[1, 3, 2, 4, 5, 6]);
    assert_eq!(out.num_sent(), 0);

    // Track 1's keyframe starts it. The premature prune drops through the
    // first audio (the keyframe with it); the start index then drops the
    // audio before the one closest to the first remaining video.
    out.feed_video(1, 30, 30, T + 2 * FRAME_US, true, 7);

    // Characterized, not endorsed: track 0 starts on a frame that is not a
    // keyframe.
    assert_eq!(out.num_sent(), 1);
    assert_eq!(out.sent[0].tag, 5);
    assert!(!out.sent[0].keyframe);
    assert_eq!(out.sent[0].dts, 0);
    assert_eq!(out.video_offsets[0], 32);
    assert_eq!(out.video_offsets[1], 30);
    assert_eq!(out.audio_offsets[0], 50048);
    // the dropped frames took their packet times; the sent one had its own
    assert!(out.callback_calls[0].time_found);
    assert_eq!(out.callback_calls[0].time.cts, 5);
    assert_eq!(out.packet_times[0].len(), 0);
    out.assert_queue_tags(&[7, 6]);
    out.teardown();
}

// ---------------------------------------------------------------------------
// start: offsets and resort

#[test]
fn start_waits_for_every_track() {
    let mut out = Out::setup();
    out.use_tracks(2, 2);

    out.feed_video(0, 30, 30, T, true, 10);
    out.feed_video(1, 30, 30, T, true, 11);
    out.feed_audio(0, 48000, T + 1000, 20);

    // Every flag is set, but audio track 1 has nothing: the premature prune
    // says -1 and clears received_audio. Nothing is sent or dropped.
    assert!(!out.received_audio);
    assert_eq!(out.num_sent(), 0);
    out.assert_queue_tags(&[10, 11, 20]);

    out.feed_audio(1, 48000, T + 2000, 21);

    // Started: the first packet is sent at once. Each track gets its own
    // offset, so both audio tracks start at dts 0 although they were 1 ms
    // apart.
    assert_eq!(out.num_sent(), 1);
    assert_eq!(out.sent[0].tag, 10);
    assert_eq!(out.sent[0].dts, 0);
    assert!(out.sent[0].locked);
    assert_eq!(out.video_offsets[0], 30);
    assert_eq!(out.video_offsets[1], 30);
    assert_eq!(out.audio_offsets[0], 48000);
    assert_eq!(out.audio_offsets[1], 48000);
    out.assert_queue_tags(&[11, 20, 21]);
    for i in 0..3 {
        assert_eq!(out.queued_at(i).dts, 0);
        assert_eq!(out.queued_at(i).dts_usec, 0);
    }
    assert_eq!(out.highest_video_ts[0], 0);
    assert_eq!(out.highest_video_ts[1], 0);
    assert_eq!(out.highest_audio_ts, 0);
    assert_eq!(out.total_frames, 1);
    out.teardown();
}

#[test]
fn start_offsets_and_resort() {
    let mut out = Out::setup();
    out.use_tracks(1, 2);

    out.feed_audio(0, 48000, T - 1000, 20);
    out.feed_audio(1, 96000, T - 3000, 30);
    // a B-frame stream: the keyframe's dts is below its pts
    out.feed_video(0, 29, 30, T, true, 10);

    // Characterized, not endorsed: the start index is track 0's audio,
    // closest to the video, so track 1's only packet is discarded; then
    // track 1 has no audio, and the start waits.
    assert!(!out.received_audio);
    out.assert_queue_tags(&[20, 10]);
    assert_eq!(out.num_sent(), 0);

    out.feed_audio(0, 49024, T + 20333, 21);
    assert!(!out.received_audio); // track 1 still has nothing
    out.feed_audio(1, 97024, T + 18333, 31);

    // Started. The video offset is the first video pts, so the keyframe's
    // dts is -1 (-33333 us). Each audio offset is that track's first dts.
    assert_eq!(out.video_offsets[0], 30);
    assert_eq!(out.audio_offsets[0], 48000);
    assert_eq!(out.audio_offsets[1], 97024);

    assert_eq!(out.num_sent(), 1);
    assert_eq!(out.sent[0].tag, 10);
    assert_eq!(out.sent[0].dts, -1);
    assert_eq!(out.sent[0].pts, 0);
    assert_eq!(out.sent[0].dts_usec, -FRAME_US);
    // its packet time, queued before the start, got the same offset
    assert_eq!(out.callback_calls.len(), 1);
    assert!(out.callback_calls[0].time_found);
    assert_eq!(out.callback_calls[0].time.pts, 0);
    assert_eq!(out.callback_calls[0].time.cts, 10);
    assert_eq!(out.packet_times[0].len(), 0);

    // the rest, re-sorted by the offset dts_usec
    out.assert_queue_tags(&[20, 31, 21]);
    assert_eq!(out.queued_at(0).dts_usec, 0);
    assert_eq!(out.queued_at(1).dts_usec, 0);
    assert_eq!(out.queued_at(2).dts, 1024);
    assert_eq!(out.queued_at(2).dts_usec, 21333);
    assert_eq!(out.highest_video_ts[0], -FRAME_US);
    assert_eq!(out.highest_audio_ts, 21333);

    // After the start, offsets apply as packets come: a video packet at the
    // same dts_usec as queued audio goes before it, and is sent at once,
    // with its packet time offset too.
    out.feed_video(0, 30, 31, T + FRAME_US, false, 11);
    assert_eq!(out.num_sent(), 2);
    assert_eq!(out.sent[1].tag, 11);
    assert_eq!(out.sent[1].dts, 0);
    assert_eq!(out.sent[1].pts, 1);
    assert_eq!(out.callback_calls[1].time.pts, 1);
    assert_eq!(out.highest_video_ts[0], 0);
    out.assert_queue_tags(&[20, 31, 21]);
    out.teardown();
}

#[test]
fn start_leaves_priming_audio_unshifted() {
    let mut out = Out::setup();

    out.feed_audio(0, -1024, T - 21333, 20);
    out.feed_video(0, 30, 30, T, true, 10);
    out.feed_audio(0, 0, T, 21);

    // The priming packet (pts <= 0) keeps the start at the first audio.
    // Characterized, not endorsed: an audio offset is taken only from a
    // dts > 0, so the track keeps offset 0 and its own timestamps.
    assert_eq!(out.audio_offsets[0], 0);
    assert_eq!(out.video_offsets[0], 30);
    assert_eq!(out.num_sent(), 1);
    assert_eq!(out.sent[0].tag, 20);
    assert_eq!(out.sent[0].dts, -1024);
    assert_eq!(out.sent[0].dts_usec, -21333);
    out.assert_queue_tags(&[10, 21]);
    assert_eq!(out.queued_at(0).dts_usec, 0);
    assert_eq!(out.queued_at(1).dts_usec, 0);
    assert_eq!(out.highest_audio_ts, 0);
    out.teardown();
}

#[test]
fn start_with_only_priming_audio() {
    let mut out = Out::setup();

    out.feed_audio(0, -2048, T - 10000, 20);
    out.feed_video(0, 30, 30, T, true, 10);
    out.feed_audio(0, -1024, T + 11333, 21);

    // Every audio packet still primes the encoder (dts < 0, offset 0), so
    // after the start all audio is below the video. highest_audio_ts is 0
    // minus the first audio's system dts_usec, raised by the resort only to
    // the highest (negative) offset audio.
    assert_eq!(out.audio_offsets[0], 0);
    assert_eq!(out.num_sent(), 1);
    assert_eq!(out.sent[0].tag, 20);
    assert_eq!(out.sent[0].dts_usec, -42666);
    out.assert_queue_tags(&[21, 10]);
    assert_eq!(out.queued_at(0).dts_usec, -21333);
    assert_eq!(out.highest_audio_ts, -21333);
    assert_eq!(out.highest_video_ts[0], 0);
    out.teardown();
}

#[test]
fn start_needs_audio_past_the_first_video() {
    let mut out = Out::setup();

    out.feed_audio(0, 48000, T - 10000, 20);
    out.feed_video(0, 30, 30, T, true, 10);

    // The last audio is before the video: received_audio is cleared, and
    // nothing is dropped or sent.
    assert!(!out.received_audio);
    out.assert_queue_tags(&[20, 10]);
    assert_eq!(out.num_sent(), 0);

    out.feed_audio(0, 49024, T + 11333, 21);
    // The audio 10 ms before the video starts with it at dts 0, and the
    // video goes first.
    assert_eq!(out.num_sent(), 1);
    assert_eq!(out.sent[0].tag, 10);
    out.assert_queue_tags(&[20, 21]);
    assert_eq!(out.queued_at(0).dts, 0);
    assert_eq!(out.queued_at(1).dts_usec, 21333);
    // highest_audio_ts: 0 minus the first audio's dts_usec, then raised by
    // the resort to the highest offset audio
    assert_eq!(out.highest_audio_ts, 21333);
    out.teardown();
}

#[test]
fn offset_arithmetic() {
    let mut out = Out::setup();
    out.use_tracks(2, 2);
    out.video_offsets[1] = 100;
    out.audio_offsets[1] = 48000;

    let mut v = video_packet(1, 105, 107, 123, false, 1);
    v.track = 1;
    let mut ept = Time {
        pts: 107,
        ..Time::default()
    };
    apply_interleaved_packet_offset(&out, &mut v, Some(&mut ept));
    assert_eq!(v.dts, 5);
    assert_eq!(v.pts, 7);
    assert_eq!(ept.pts, 7);
    // dts_usec is recomputed from the offset dts: dts * 1e6 / den
    assert_eq!(v.dts_usec, 166_666);

    let mut a = audio_packet(1, 49024, 123, 2);
    a.track = 1;
    apply_interleaved_packet_offset(&out, &mut a, None);
    assert_eq!(a.dts, 1024);
    assert_eq!(a.pts, 1024);
    assert_eq!(a.dts_usec, 21333);

    // the timebase numerator is not used: pts units are 1/den
    let mut ntsc = video_packet(0, 1001, 1001, 0, false, 3);
    ntsc.num = 1001;
    ntsc.den = 30000;
    ntsc.track = 0;
    apply_interleaved_packet_offset(&out, &mut ntsc, None);
    assert_eq!(ntsc.dts_usec, 33366);

    // queued packet times get the video offset of their track
    out.push_packet_time(1, 110, 0);
    out.push_packet_time(0, 110, 0);
    apply_ept_offsets(&mut out);
    assert_eq!(out.packet_times[1][0].pts, 10);
    assert_eq!(out.packet_times[0][0].pts, 110);
    out.teardown();
}

// ---------------------------------------------------------------------------
// insertion order and resort

#[test]
fn insertion_order() {
    let mut out = Out::setup();
    out.use_tracks(2, 2);

    let mut p;

    p = audio_packet(0, 0, 100, 1);
    p.track = 0;
    insert_interleaved_packet(&mut out, p.clone());
    // video goes before audio with the same dts_usec
    p = video_packet(1, 0, 0, 100, false, 2);
    p.track = 1;
    insert_interleaved_packet(&mut out, p.clone());
    // and video of a lower track before a higher one
    p = video_packet(0, 0, 0, 100, false, 3);
    p.track = 0;
    insert_interleaved_packet(&mut out, p.clone());
    // a second track 1 video goes before the first one
    p = video_packet(1, 0, 0, 100, false, 4);
    p.track = 1;
    insert_interleaved_packet(&mut out, p.clone());
    // audio goes after everything with the same dts_usec
    p = audio_packet(1, 0, 100, 5);
    p.track = 1;
    insert_interleaved_packet(&mut out, p.clone());
    // otherwise by dts_usec
    p = audio_packet(0, 0, 50, 6);
    p.track = 0;
    insert_interleaved_packet(&mut out, p.clone());
    p = video_packet(0, 0, 0, 150, false, 7);
    p.track = 0;
    insert_interleaved_packet(&mut out, p);

    // The queue does not own these (the test's data): empty it by hand.
    out.assert_queue_tags(&[6, 3, 4, 2, 1, 5, 7]);
    out.free_queue();
    out.teardown();
}

#[test]
fn resort_reinserts_and_raises_highest() {
    let mut out = Out::setup();
    out.use_tracks(2, 1);
    out.highest_audio_ts = 500;

    out.queue_packet(audio_packet(0, 0, 300, 1), 0);
    out.queue_packet(video_packet(1, 0, 0, 200, false, 2), 1);
    out.queue_packet(audio_packet(0, 0, 100, 3), 0);
    out.queue_packet(video_packet(0, 0, 0, 200, false, 4), 0);

    let before = out.buffer;
    resort_interleaved_packets(&mut out);
    out.assert_queue_tags(&[3, 4, 2, 1]);
    // a new array
    assert_ne!(out.buffer, before);
    // the highest timestamps are only raised
    assert_eq!(out.highest_audio_ts, 500);
    assert_eq!(out.highest_video_ts[0], 200);
    assert_eq!(out.highest_video_ts[1], 200);

    let mut p = audio_packet(0, 0, 900, 5);
    p.track = 0;
    set_higher_ts(&mut out, &p);
    assert_eq!(out.highest_audio_ts, 900);
    p.dts_usec = 800;
    set_higher_ts(&mut out, &p);
    assert_eq!(out.highest_audio_ts, 900);

    // per video track, also only raised
    p = video_packet(1, 0, 0, 150, false, 6);
    p.track = 1;
    set_higher_ts(&mut out, &p);
    assert_eq!(out.highest_video_ts[1], 200);
    assert_eq!(out.highest_video_ts[0], 200);
    p.dts_usec = 250;
    set_higher_ts(&mut out, &p);
    assert_eq!(out.highest_video_ts[1], 250);
    assert_eq!(out.highest_video_ts[0], 200);
    out.teardown();
}

// ---------------------------------------------------------------------------
// sending after the start

#[test]
fn sends_when_the_opposing_types_are_ahead() {
    let mut out = Out::setup();
    out.use_tracks(2, 1);
    out.highest_video_ts[0] = 200;
    out.highest_video_ts[1] = 100;
    out.highest_audio_ts = 200;

    // audio needs every video track above it; video needs the other video
    // tracks and the audio above it. Equal is not above.
    let mut a = audio_packet(0, 0, 99, 1);
    assert!(has_higher_opposing_ts(&out, &a));
    a.dts_usec = 100;
    assert!(!has_higher_opposing_ts(&out, &a));

    let mut v0 = video_packet(0, 0, 0, 99, false, 2);
    v0.track = 0;
    assert!(has_higher_opposing_ts(&out, &v0));
    v0.dts_usec = 100;
    assert!(!has_higher_opposing_ts(&out, &v0));
    // its own track does not count
    let mut v1 = video_packet(1, 0, 0, 150, false, 3);
    v1.track = 1;
    assert!(has_higher_opposing_ts(&out, &v1));
    v1.dts_usec = 200;
    assert!(!has_higher_opposing_ts(&out, &v1));

    // tracks without an encoder are skipped
    out.video_encoders[1] = None;
    a.dts_usec = 150;
    assert!(has_higher_opposing_ts(&out, &a));
    out.teardown();
}

fn started_state(out: &mut Out) {
    out.received_audio = true;
    out.received_video[0] = true;
    out.queue_packet(audio_packet(0, 0, 0, 1), 0);
    out.queue_packet(audio_packet(0, 1024, 21333, 2), 0);
    out.queue_packet(audio_packet(0, 2048, 42666, 3), 0);
    out.highest_audio_ts = 42666;
}

#[test]
fn one_packet_per_call_then_catch_up() {
    let mut out = Out::setup();

    // Three packets are streamable after this video packet (two audio
    // below it, then itself, below the highest audio). One is sent, and a
    // second one only when more than interleaver_max_batch_size are left.
    started_state(&mut out);
    out.max_batch_size = 2;
    out.feed_video(0, 1, 1, 0, false, 4);
    assert_eq!(out.num_sent(), 1);
    assert_eq!(out.sent[0].tag, 1);
    out.assert_queue_tags(&[2, 4, 3]);
    assert_eq!(out.highest_video_ts[0], FRAME_US);

    // After the next video packet (66666 us) three are streamable again,
    // and two left is more than a limit of 1: two are sent.
    out.max_batch_size = 1;
    out.feed_video(0, 2, 2, 0, false, 5);
    assert_eq!(out.num_sent(), 3);
    assert_eq!(out.sent[1].tag, 2);
    assert_eq!(out.sent[2].tag, 4);
    out.assert_queue_tags(&[3, 5]);

    // Audio alone sends nothing while no video is above it.
    free_packets(&mut out);
    out.sent.clear();
    out.highest_video_ts[0] = i64::MIN;
    started_state(&mut out);
    out.feed_audio(0, 3072, 0, 7);
    assert_eq!(out.highest_audio_ts, 64000);
    assert_eq!(out.num_sent(), 0);

    // Never more than two per call: four are streamable here.
    out.max_batch_size = 0;
    out.feed_video(0, 3, 3, 0, false, 8);
    assert_eq!(out.num_sent(), 2);
    out.assert_queue_tags(&[3, 7, 8]);
    out.teardown();
}

#[test]
fn send_interleaved_times_callbacks_and_counts() {
    let mut out = Out::setup();

    out.queue_packet(video_packet(0, 5, 5, 0, false, 1), 0);
    out.queue_packet(audio_packet(0, 0, 0, 2), 0);
    out.queue_packet(video_packet(0, 7, 7, 0, false, 3), 0);
    out.queue_packet(video_packet(0, 8, 8, 0, false, 4), 0);
    out.push_packet_time(0, 4, 40);
    out.push_packet_time(0, 5, 50);
    out.push_packet_time(0, 6, 60);

    // video: the packet time with the same pts is handed to the packet
    // callback, with pir set, and taken out of the array; the others stay
    send_interleaved(&mut out);
    assert_eq!(out.num_sent(), 1);
    assert_eq!(out.sent[0].tag, 1);
    assert_eq!(out.total_frames, 1);
    assert_eq!(out.callback_calls.len(), 1);
    assert_eq!(out.callback_calls[0].tag, 1);
    assert!(out.callback_calls[0].time_found);
    assert_eq!(out.callback_calls[0].time.cts, 50);
    assert!(out.callback_calls[0].time.pir > 0);
    assert!(out.callback_calls[0].callbacks_locked);
    assert_eq!(out.packet_times[0].len(), 2);
    assert_eq!(out.packet_times[0][0].pts, 4);
    assert_eq!(out.packet_times[0][1].pts, 6);

    // audio: no packet time, not counted as a frame
    send_interleaved(&mut out);
    assert_eq!(out.sent[1].tag, 2);
    assert!(!out.callback_calls[1].time_found);
    assert_eq!(out.total_frames, 1);

    // video without a matching time: NULL, and a debug line
    out.log.borrow_mut().count = 0;
    send_interleaved(&mut out);
    assert!(!out.callback_calls[2].time_found);
    assert_eq!(out.total_frames, 2);
    assert_eq!(out.packet_times[0].len(), 2);
    assert_eq!(out.log.borrow().count, 1);
    assert_eq!(out.log.borrow().level, LOG_DEBUG);
    assert!(
        out.last_log()
            .contains("encoder packet timing for PTS7 not found")
    );

    // empty array: another debug line
    out.packet_times[0].clear();
    send_interleaved(&mut out);
    assert!(!out.callback_calls[3].time_found);
    assert!(out.last_log().contains("encoder packet timing array empty"));

    assert_eq!(out.queued(), 0);
    assert_eq!(out.caption_calls.len(), 0);
    out.teardown();
}

#[test]
fn send_interleaved_caption_decision() {
    let mut out = Out::setup();

    out.captions[0].head = Some((2.0, "x".to_string()));
    out.captions[0].timestamp = 1.0;

    // pts 15 at 1/30 is 0.5 s: before the caption's time, not added
    out.queue_packet(video_packet(0, 15, 15, 0, false, 1), 0);
    send_interleaved(&mut out);
    assert_eq!(out.caption_calls.len(), 0);

    // pts 30 is 1.0 s: added (under the track's caption mutex), and the
    // next caption waits for its display duration
    out.queue_packet(video_packet(0, 30, 30, 0, false, 2), 0);
    send_interleaved(&mut out);
    assert_eq!(out.caption_calls.len(), 1);
    assert!(out.caption_calls[0].locked);
    assert!(out.captions[0].timestamp == 3.0);

    // add_caption() failing leaves the time alone
    out.add_caption_result = false;
    out.queue_packet(video_packet(0, 90, 90, 0, false, 3), 0);
    send_interleaved(&mut out);
    assert_eq!(out.caption_calls.len(), 2);
    assert!(out.captions[0].timestamp == 3.0);

    // raw CEA-608 data: added once per later frame time
    out.captions[0].head = None;
    out.captions[0].data_size = 3;
    out.queue_packet(video_packet(0, 120, 120, 0, false, 4), 0);
    send_interleaved(&mut out);
    assert_eq!(out.caption_calls.len(), 3);
    assert!(out.captions[0].last_timestamp == 4.0);
    out.queue_packet(video_packet(0, 120, 120, 0, false, 5), 0);
    send_interleaved(&mut out);
    assert_eq!(out.caption_calls.len(), 3);

    // audio never touches captions
    out.queue_packet(audio_packet(0, 0, 0, 6), 0);
    send_interleaved(&mut out);
    assert_eq!(out.caption_calls.len(), 3);
    assert_eq!(out.num_sent(), 6);
    out.teardown();
}

// ---------------------------------------------------------------------------
// discard of unused audio

#[test]
fn discard_below_a_dts() {
    let mut out = Out::setup();
    out.use_tracks(2, 1);

    out.queue_packet(audio_packet(0, 0, 10, 1), 0);
    out.queue_packet(video_packet(1, 0, 3, 20, true, 2), 1);
    out.queue_packet(audio_packet(0, 0, 30, 3), 0);
    out.queue_packet(video_packet(1, 1, 4, 35, false, 4), 1);
    out.push_packet_time(1, 3, 0);
    out.push_packet_time(1, 4, 0);

    // everything below the dts_usec goes, whatever its type; a video
    // packet takes the oldest packet time of its track with it
    discard_unused_audio_packets(&mut out, 30);
    out.assert_queue_tags(&[3, 4]);
    assert_eq!(out.packet_times[1].len(), 1);
    assert_eq!(out.packet_times[1][0].pts, 4);

    discard_unused_audio_packets(&mut out, 0);
    assert_eq!(out.queued(), 2);

    // the first one at or above it stops the discard
    out.queued_at(0).dts_usec = 100;
    discard_unused_audio_packets(&mut out, 50);
    assert_eq!(out.queued(), 2);
    out.teardown();
}

// ---------------------------------------------------------------------------
// encoder group keyframe check

fn group_keyframe(track: usize, pts: i64) -> Pkt {
    let mut p = video_packet(track, pts, pts, 0, true, 1);
    p.track = track;
    p
}

#[test]
fn group_keyframes_are_tracked_until_every_track_has_one() {
    let mut out = Out::setup();
    out.use_tracks(2, 1);
    out.num_encoders_started = 2;
    out.enc_mut(VENC[0]).info.group = Some(GROUP);
    out.enc_mut(VENC[1]).info.group = Some(GROUP);

    let mut p = group_keyframe(0, 10);
    check_encoder_group_keyframe_alignment(&mut out, &p);
    assert_eq!(out.groups.len(), 1);
    let d = &out.groups[0];
    assert_eq!(d.group_id, GROUP);
    assert_eq!(d.pts, 10);
    assert_eq!(d.required_tracks, 2);
    assert_eq!(d.seen_on_track[0], TrackStatus::Seen);
    assert_eq!(d.seen_on_track[1], TrackStatus::NotSeen);

    // the other track's keyframe at the same pts completes it
    p = group_keyframe(1, 10);
    check_encoder_group_keyframe_alignment(&mut out, &p);
    assert_eq!(out.groups.len(), 0);
    assert_eq!(out.log.borrow().warnings, 0);

    // non-keyframes, audio and encoders without a group are not tracked
    p = group_keyframe(0, 20);
    p.keyframe = false;
    check_encoder_group_keyframe_alignment(&mut out, &p);
    let mut a = audio_packet(0, 0, 0, 2);
    a.keyframe = true;
    check_encoder_group_keyframe_alignment(&mut out, &a);
    out.enc_mut(VENC[1]).info.group = None;
    p = group_keyframe(1, 20);
    check_encoder_group_keyframe_alignment(&mut out, &p);
    assert_eq!(out.groups.len(), 0);
    out.enc_mut(VENC[1]).info.group = Some(GROUP);

    // Track 0 has keyframes at 20 and 30, track 1 only at 30. Characterized,
    // not endorsed: track 0's own keyframe at 30 marks its entry at 20
    // "skipped" (over "seen"), without a warning.
    p = group_keyframe(0, 20);
    check_encoder_group_keyframe_alignment(&mut out, &p);
    p = group_keyframe(0, 30);
    check_encoder_group_keyframe_alignment(&mut out, &p);
    assert_eq!(out.groups.len(), 2);
    assert_eq!(out.groups[0].seen_on_track[0], TrackStatus::Skipped);
    assert_eq!(out.log.borrow().warnings, 0);

    // track 1 missed the keyframe at 20: a warning, and both entries close
    p = group_keyframe(1, 30);
    check_encoder_group_keyframe_alignment(&mut out, &p);
    assert_eq!(out.groups.len(), 0);
    assert_eq!(out.log.borrow().warnings, 1);
    assert_eq!(
        out.last_log(),
        "obs-output 'interleaved output': Missing keyframe with pts 20 for encoder 'video 1' (track: 1)"
    );

    // entries of other groups and later pts are left alone; a new one is
    // inserted in pts order
    p = group_keyframe(0, 50);
    check_encoder_group_keyframe_alignment(&mut out, &p);
    p = group_keyframe(0, 40);
    check_encoder_group_keyframe_alignment(&mut out, &p);
    assert_eq!(out.groups.len(), 2);
    assert_eq!(out.groups[0].pts, 40);
    assert_eq!(out.groups[1].pts, 50);
    // the same keyframe again marks track 0 seen and adds nothing
    p = group_keyframe(0, 40);
    check_encoder_group_keyframe_alignment(&mut out, &p);
    assert_eq!(out.groups.len(), 2);

    out.enc_mut(VENC[0]).info.group = None;
    out.enc_mut(VENC[1]).info.group = None;
    out.teardown();
}

// ---------------------------------------------------------------------------
// free_packets

#[test]
fn free_packets_releases_everything() {
    let mut out = Out::setup();

    let allocs = out.live.len();
    out.queue_packet(video_packet(0, 0, 0, 0, true, 1), 0);
    out.queue_packet(audio_packet(0, 0, 0, 2), 0);
    out.queue_packet(audio_packet(0, 0, 0, 3), 0);
    assert!(out.live.len() > allocs);

    free_packets(&mut out);
    assert_eq!(out.queued(), 0);
    assert_eq!(out.buffer, None);
    assert_eq!(out.live.len(), allocs);

    // the other interleaver state is left alone
    out.received_audio = true;
    out.video_offsets[0] = 5;
    free_packets(&mut out);
    assert!(out.received_audio);
    assert_eq!(out.video_offsets[0], 5);
    out.teardown();
}

// ---------------------------------------------------------------------------
// edge cases (new)

#[test]
fn dts_usec_truncates_toward_zero() {
    assert_eq!(packet_dts_usec(1, 30), 33333);
    assert_eq!(packet_dts_usec(-1, 30), -33333);
    assert_eq!(packet_dts_usec(-1024, 48000), -21333);
    assert_eq!(packet_dts_usec(0, 48000), 0);
}

#[test]
fn encoder_duration_is_unsigned_then_signed() {
    let info = |num, den, framesize| EncoderInfo {
        timebase_num: num,
        timebase_den: den,
        framesize,
        group: None,
    };
    assert_eq!(get_encoder_duration(info(1, 48000, 1024)), 20480);
    assert_eq!(get_encoder_duration(info(1, 44100, 1024)), 22528);
    // a frame size of zero, as a video encoder has
    assert_eq!(get_encoder_duration(info(1, 30, 0)), 0);
    // a per-sample time below 1 us truncates to 0
    assert_eq!(get_encoder_duration(info(1, 2_000_000, 1024)), 0);
}

#[test]
fn first_and_last_packet_of_a_track() {
    let mut a0 = audio_packet(0, 0, 0, 1);
    a0.track = 0;
    let mut v0 = video_packet(0, 0, 0, 0, true, 2);
    v0.track = 0;
    let mut a1 = audio_packet(1, 0, 0, 3);
    a1.track = 1;
    let packets = [a0.clone(), v0, a1, a0];
    assert_eq!(
        find_first_packet_type_idx(&packets, Kind::Audio, 0),
        Some(0)
    );
    assert_eq!(find_last_packet_type_idx(&packets, Kind::Audio, 0), Some(3));
    assert_eq!(
        find_first_packet_type_idx(&packets, Kind::Video, 0),
        Some(1)
    );
    assert_eq!(find_last_packet_type_idx(&packets, Kind::Video, 0), Some(1));
    assert_eq!(
        find_first_packet_type_idx(&packets, Kind::Audio, 1),
        Some(2)
    );
    assert_eq!(find_first_packet_type_idx(&packets, Kind::Video, 1), None);
    assert_eq!(find_last_packet_type_idx::<Pkt>(&[], Kind::Audio, 0), None);
}

#[test]
fn start_index_without_audio_is_the_video() {
    let mut v = video_packet(0, 0, 0, T, true, 1);
    v.track = 0;
    let mut a = audio_packet(0, 0, T + 5, 2);
    a.track = 0;
    // a single closest audio after the video: the video comes first
    assert_eq!(interleaved_start_idx(&[v.clone(), a.clone()]), 0);
    // audio first, closer than nothing: the audio
    let mut early = a.clone();
    early.dts_usec = T - 5;
    early.pts = 10;
    assert_eq!(interleaved_start_idx(&[early, v, a]), 0);
}

#[test]
fn premature_prune_without_audio_encoders_keeps() {
    let mut v = video_packet(0, 0, 0, T, true, 1);
    v.track = 0;
    assert_eq!(premature_prune(&[v], |_| None, |_| 0), Prune::Keep);
    assert_eq!(Prune::Keep.c_value(), 0);
    assert_eq!(Prune::NoVideo.c_value(), -1);
    assert_eq!(Prune::MissingAudio.c_value(), -1);
    assert_eq!(Prune::Through(7).c_value(), 7);
}

#[test]
fn premature_prune_asks_durations_up_to_the_missing_track() {
    let mut v = video_packet(0, 0, 0, T, true, 1);
    v.track = 0;
    let mut a0 = audio_packet(0, 0, T, 2);
    a0.track = 0;
    let asked = RefCell::new(Vec::new());
    let slots = [Some(AENC[0]), None, Some(AENC[1]), None, None, None];
    let prune = premature_prune(
        &[v, a0],
        |slot| slots[slot],
        |encoder| {
            asked.borrow_mut().push(encoder);
            20480
        },
    );
    assert_eq!(prune, Prune::MissingAudio);
    // slot 0 was asked, slot 2 (no packet) stopped the loop first
    assert_eq!(*asked.borrow(), vec![AENC[0]]);
}

#[test]
fn insert_position_edges() {
    let mut v = video_packet(0, 0, 0, 100, false, 1);
    v.track = 0;
    // an empty queue
    assert_eq!(insert_position::<Pkt>(&[], &v), 0);
    // video at the same time as video of a higher track goes before it
    let mut v3 = v.clone();
    v3.track = 3;
    assert_eq!(insert_position(std::slice::from_ref(&v3), &v), 0);
    // ... and after video of the same track
    assert_eq!(insert_position(std::slice::from_ref(&v), &v), 0);
    // audio at the same time goes after video and audio
    let mut a = audio_packet(0, 0, 100, 2);
    a.track = 0;
    assert_eq!(insert_position(&[v.clone(), a.clone()], &a), 2);
    // anything goes before a later packet
    let mut later = a.clone();
    later.dts_usec = 101;
    assert_eq!(insert_position(&[v, later], &a), 1);
}

#[test]
fn same_track_video_at_the_same_time_goes_first() {
    // The rule only skips video of a lower track; one of the same track at
    // the same time is passed over by the "video breaks" rule.
    let mut first = video_packet(0, 0, 0, 100, false, 1);
    first.track = 1;
    let mut second = first.clone();
    second.tag = 2;
    assert_eq!(insert_position(std::slice::from_ref(&first), &second), 0);
}

#[test]
fn highest_without_video_encoders() {
    let mut a = audio_packet(0, 0, 100, 1);
    a.track = 0;
    let mut v = video_packet(0, 0, 0, 100, false, 2);
    v.track = 0;
    let h = Highest {
        video: [None; MAX_OUTPUT_VIDEO_ENCODERS],
        audio: 101,
    };
    // audio has nothing to wait for; video waits for audio only
    assert!(h.has_higher_opposing(&a));
    assert!(h.has_higher_opposing(&v));
    let h = Highest { audio: 100, ..h };
    assert!(!h.has_higher_opposing(&v));
    // the streamable count stops at the first one that is not
    assert_eq!(h.streamable(&[a.clone(), v, a]), 1);
}

#[test]
fn catch_up_rule() {
    assert_eq!(sends_for(0, 0), 0);
    assert_eq!(sends_for(1, 0), 1);
    assert_eq!(sends_for(2, 0), 2);
    assert_eq!(sends_for(2, 1), 1);
    assert_eq!(sends_for(3, 1), 2);
    assert_eq!(sends_for(100, 8), 2);
    assert_eq!(sends_for(9, 8), 1);
    assert_eq!(sends_for(10, 8), 2);
    assert_eq!(sends_for(1, usize::MAX), 1);
}

#[test]
fn discard_count_edges() {
    let mut a = audio_packet(0, 0, 10, 1);
    a.track = 0;
    let mut b = a.clone();
    b.dts_usec = 5;
    assert_eq!(discard_count::<Pkt>(&[], 100), 0);
    // stops at the first at or above, even with lower ones after it
    assert_eq!(discard_count(&[a.clone(), b.clone()], 10), 0);
    assert_eq!(discard_count(&[b.clone(), a.clone()], 10), 1);
    assert_eq!(discard_count(&[b, a], 11), 2);
}

#[test]
fn caption_timing() {
    assert!(frame_timestamp(30, 1, 30) == 1.0);
    assert!(frame_timestamp(1001, 1001, 30000) == 1001.0 * 1001.0 / 30000.0);
    assert!(frame_timestamp(-15, 1, 30) == -0.5);
    // a caption is due at its time, not before; never without one
    assert!(caption_due(true, 1.0, 1.0));
    assert!(!caption_due(true, 1.0, 0.99));
    assert!(!caption_due(false, 0.0, 1.0));
    // raw data goes only into a later frame
    assert!(raw_caption_due(1, 0.5, 1.0));
    assert!(!raw_caption_due(1, 1.0, 1.0));
    assert!(!raw_caption_due(0, 0.0, 1.0));
}

#[test]
fn packet_time_lookup_takes_the_first_match() {
    let t = |pts, cts| Time {
        pts,
        cts,
        ..Time::default()
    };
    let times = [t(4, 1), t(5, 2), t(5, 3)];
    assert_eq!(find_packet_time(&times, 5), Some(1));
    assert_eq!(find_packet_time(&times, 6), None);
    assert_eq!(find_packet_time::<Time>(&[], 0), None);
}

#[test]
fn group_needs_its_required_tracks() {
    let mut g = Group::new(GROUP, 10, 2, 2);
    assert!(!group_complete(&g));
    g.set_seen(5, TrackStatus::Skipped);
    assert!(group_complete(&g));
    // a group that needs one track is complete when created, but it is only
    // erased when a later keyframe of the group looks at it
    let g = Group::new(GROUP, 10, 0, 1);
    assert!(group_complete(&g));
}

#[test]
fn single_encoder_group_entry_stays_until_the_next_keyframe() {
    let mut out = Out::setup();
    out.num_encoders_started = 1;
    out.enc_mut(VENC[0]).info.group = Some(GROUP);

    check_encoder_group_keyframe_alignment(&mut out, &group_keyframe(0, 10));
    assert_eq!(out.groups.len(), 1);
    // the next one closes it as seen again, and adds its own
    check_encoder_group_keyframe_alignment(&mut out, &group_keyframe(0, 20));
    assert_eq!(out.groups.len(), 1);
    assert_eq!(out.groups[0].pts, 20);
    assert_eq!(out.log.borrow().warnings, 0);
    // an older keyframe goes before the entries after it
    check_encoder_group_keyframe_alignment(&mut out, &group_keyframe(0, 15));
    assert_eq!(out.groups.len(), 2);
    assert_eq!(out.groups[0].pts, 15);
    out.teardown();
}

#[test]
fn other_groups_are_passed_over() {
    let mut out = Out::setup();
    out.use_tracks(2, 1);
    out.num_encoders_started = 2;
    out.enc_mut(VENC[0]).info.group = Some(GROUP);
    out.groups.push(Group::new(GROUP + 1, 5, 1, 2));

    check_encoder_group_keyframe_alignment(&mut out, &group_keyframe(0, 10));
    assert_eq!(out.groups.len(), 2);
    assert_eq!(out.groups[0].group_id, GROUP + 1);
    // the other group's entry is not touched
    assert_eq!(out.groups[0].seen_on_track[0], TrackStatus::NotSeen);
    assert_eq!(out.groups[1].group_id, GROUP);
    out.teardown();
}

#[test]
fn one_lock_per_packet_and_released() {
    let mut out = Out::setup();
    out.feed_audio(0, 48000, T + 1000, 1);
    assert_eq!(out.lock_calls, 1);
    // a waiting non-keyframe locks once too
    out.feed_video(0, 30, 30, T + 2000, false, 2);
    assert_eq!(out.lock_calls, 2);
    // the start, with its sends, under one lock
    out.feed_video(0, 31, 31, T + 3000, true, 3);
    out.feed_audio(0, 49024, T + 30000, 4);
    assert_eq!(out.lock_calls, 4);
    assert_eq!(out.num_sent(), 1);
    assert!(out.sent.iter().all(|s| s.locked));
    // an inactive output takes no lock
    out.active = false;
    let mut pkt = audio_packet(0, 50048, T + 50000, 5);
    interleave_packets(&mut out, &mut pkt, None);
    assert_eq!(out.lock_calls, 4);
    out.teardown();
}

#[test]
fn delayed_waiting_packet_is_released_after_the_unlock() {
    let mut out = Out::setup();
    out.active_delay_ns = 1;
    let src = video_packet(0, 30, 30, T, false, 1);
    let mut held = out.copy_packet(&src);
    interleave_packets(&mut out, &mut held, None);
    assert_eq!(out.releases_locked, vec![false]);
    // the caller's packet is zeroed by the release
    assert_eq!(held, Pkt::zeroed());
    out.teardown();
}

#[test]
fn no_packet_time_pushes_nothing() {
    let mut out = Out::setup();
    let mut pkt = video_packet(0, 30, 30, T, true, 1);
    interleave_packets(&mut out, &mut pkt, None);
    assert!(out.packet_times.iter().all(Vec::is_empty));
    out.teardown();
}

#[test]
fn audio_packet_time_goes_to_the_same_numbered_array() {
    // An audio packet with a packet time (encoders give none) pushes it to
    // encoder_packet_times[audio track], as the C does.
    let mut out = Out::setup();
    out.use_tracks(1, 2);
    let mut pkt = audio_packet(1, 48000, T, 1);
    let ept = Time {
        pts: 48000,
        cts: 9,
        ..Time::default()
    };
    interleave_packets(&mut out, &mut pkt, Some(&ept));
    assert_eq!(out.packet_times[1].len(), 1);
    assert_eq!(out.packet_times[1][0].cts, 9);
    out.teardown();
}

#[test]
fn packet_time_offset_after_the_start() {
    let mut out = Out::setup();
    out.feed_audio(0, 48000, T, 1);
    out.feed_video(0, 30, 30, T, true, 2);
    // started: the video went out with its time
    assert_eq!(out.num_sent(), 1);
    // a later frame's queued time gets the offset when it arrives
    out.feed_video(0, 31, 32, T + FRAME_US, false, 3);
    assert_eq!(out.packet_times[0].len(), 1);
    assert_eq!(out.packet_times[0][0].pts, 2);
    out.teardown();
}

#[test]
fn every_callback_gets_its_own_pir() {
    let mut out = Out::setup();
    out.callbacks = 3;
    out.queue_packet(video_packet(0, 5, 5, 0, false, 1), 0);
    out.push_packet_time(0, 5, 50);
    send_interleaved(&mut out);
    let pirs: Vec<u64> = out.callback_calls.iter().map(|c| c.time.pir).collect();
    assert_eq!(pirs.len(), 3);
    assert!(pirs.windows(2).all(|w| w[0] < w[1]));
    assert!(out.callback_calls.iter().all(|c| c.time_found));
    // no callbacks: still sent and released
    out.callbacks = 0;
    out.queue_packet(video_packet(0, 6, 6, 0, false, 2), 0);
    send_interleaved(&mut out);
    assert_eq!(out.num_sent(), 2);
    out.teardown();
}

#[test]
fn start_without_video_encoders_waits() {
    // get_audio_and_video_packets() finds no video encoder: the start fails
    // without touching the flags.
    let mut out = Out::setup();
    out.use_tracks(0, 1);
    out.queue_packet(audio_packet(0, 48000, T, 1), 0);
    out.received_audio = true;
    assert!(!interleave::initialize_interleaved_packets(&mut out));
    assert!(out.received_audio);
    out.teardown();
}

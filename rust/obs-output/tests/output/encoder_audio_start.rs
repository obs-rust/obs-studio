//! Tier 1: `obs_output::encoder_audio_start`, the encoder audio start
//! alignment and the setter gates.
//!
//! The `steps_*` cases port `test/cmocka/test_encoder_audio_start_steps.c`
//! 1:1: its 20 cases under the same names, in the same order, with the same
//! values and expectations, the "characterized, not endorsed" ones included.
//! The C calls the hidden `obs_encoder_audio_start_*` functions; here the
//! safe core is called directly. The C's stand-in encoder (`buffer_audio()`,
//! `start_from_buffer()`, `calc_offset_size()` and `push_back_audio()` with
//! the deques reduced to a byte count) is [`StandIn`], with the same
//! `util_mul_div64()` (the portable formula, as Linux and macOS build it).
//!
//! The `edge_*` cases are new: every pair of edge values against models of
//! the original C, every packet plan around the video's start, the setter
//! type over -8..=8 and the `i32` extremes, the size and pts arithmetic at
//! the integer edges, and proptests of every decision over full-range
//! inputs and of the stand-in over packet sequences.

use obs_output::encoder_audio_start::{
    OBS_ENCODER_VIDEO, PacketStart, audio_pts, buffer_offset, can_send, divisor_ok, group_ready,
    no_video, not_video, offset_ns, offset_size, packet_size, packet_start, paired_late,
    paired_start, push_size, unpaired_start, video_pts,
};
use proptest::prelude::*;

const NS: u64 = 1_000_000_000;

const EDGE_U64: [u64; 7] = [
    0,
    1,
    2,
    0x7fff_ffff_ffff_ffff,
    0x8000_0000_0000_0000,
    u64::MAX - 1,
    u64::MAX,
];

/// 1024 frames at 48 kHz: 21333333 ns.
const PKT: u64 = 21_333_333;
const V: u64 = 10_000_000_000;
/// One sample at 48 kHz is 20833.3 ns: 20834 ns converts to one frame.
const SAMPLE: u64 = 20_834;

/// `PacketStart::Start` from the C's three flags.
fn start(truncate: bool, clear: bool, from_buffer: bool) -> PacketStart {
    PacketStart::Start {
        truncate,
        clear,
        from_buffer,
    }
}

/// `util_mul_div64()` as the C builds it outside MSVC x64: the wrapping
/// `u64` formula (a test helper; the port does not divide).
fn util_mul_div64(num: u64, mul: u64, div: u64) -> u64 {
    let rem = num % div;
    (num / div)
        .wrapping_mul(mul)
        .wrapping_add(rem.wrapping_mul(mul) / div)
}

// ---------------------------------------------------------------------------
// receive_video()

#[test]
fn steps_group_ready() {
    for &a in &EDGE_U64 {
        for &b in &EDGE_U64 {
            assert_eq!(group_ready(a, b), a == b, "group_ready({a}, {b})");
        }
    }
    assert!(group_ready(1_000_000_000, 1_000_000_000));
    assert!(!group_ready(1_000_000_000, 1_000_000_001));
    assert!(!group_ready(1_000_000_001, 1_000_000_000));
    // A group that has not started has start_timestamp 0: only a frame at
    // 0 would match.
    assert!(!group_ready(0, 16_666_667));
}

#[test]
fn steps_paired_late() {
    let ts = 5_000_000_000;
    assert!(paired_late(ts + 1, ts));
    assert!(paired_late(ts + 21_333_333, ts));
    assert!(!paired_late(ts, ts));
    assert!(!paired_late(ts - 1, ts));
    assert!(!paired_late(0, ts));
    for &a in &EDGE_U64 {
        for &b in &EDGE_U64 {
            assert_eq!(paired_late(a, b), a > b, "paired_late({a}, {b})");
        }
    }
}

#[test]
fn steps_video_pts() {
    assert_eq!(video_pts(0, 1, 1), 1);
    assert_eq!(video_pts(0, 1001, 1), 1001);
    assert_eq!(video_pts(30, 1, 2), 32);
    assert_eq!(video_pts(1001 * 7, 1001, 3), 1001 * 10);
    assert_eq!(video_pts(-5, 1, 1), -4);
    assert_eq!(video_pts(-5, 0, 7), -5);
    assert_eq!(video_pts(12, 7, 0), 12);
    // timebase_num * frame_rate_divisor is a u32 product: it wraps before
    // it is added (characterized, not endorsed).
    assert_eq!(video_pts(100, 0x10000, 0x10000), 100);
    assert_eq!(video_pts(100, 0x10000, 0x10001), 100 + 0x10000);
    assert_eq!(video_pts(0, u32::MAX, u32::MAX), 1);
    assert_eq!(video_pts(0, u32::MAX, 1), i64::from(u32::MAX));
    assert_eq!(
        video_pts(i64::MAX - i64::from(u32::MAX), u32::MAX, 1),
        i64::MAX
    );
}

// ---------------------------------------------------------------------------
// buffer_audio() and its helpers

#[test]
fn steps_packet_size() {
    assert_eq!(packet_size(1024, 4), 4096);
    assert_eq!(packet_size(1024, 8), 8192);
    assert_eq!(packet_size(960, 4), 3840);
    assert_eq!(packet_size(0, 8), 0);
    assert_eq!(packet_size(1024, 0), 0);
    assert_eq!(packet_size(1, 4), 4);
    if usize::BITS > 32 {
        // A size_t product: no 32-bit wrap ...
        assert_eq!(
            packet_size(u32::MAX, 8),
            usize::try_from(u32::MAX).unwrap() * 8
        );
        // ... and a size_t wrap past it (characterized, not endorsed).
        assert_eq!(packet_size(2, usize::MAX), usize::MAX - 1);
    }
}

#[test]
fn steps_branches() {
    // Not started, paired: wait for or start with the video.
    assert!(paired_start(0, true));
    assert!(!unpaired_start(0, true));
    // Not started, unpaired: start at this packet.
    assert!(!paired_start(0, false));
    assert!(unpaired_start(0, false));
    // Started: neither.
    for &ts in &EDGE_U64[1..] {
        assert!(!paired_start(ts, true));
        assert!(!paired_start(ts, false));
        assert!(!unpaired_start(ts, true));
        assert!(!unpaired_start(ts, false));
    }
}

#[test]
fn steps_no_video() {
    assert!(no_video(0));
    for &ts in &EDGE_U64[1..] {
        assert!(!no_video(ts));
    }
}

fn expect_plan(ts: u64, dur: u64, v: u64, want: PacketStart) {
    assert_eq!(packet_start(ts, dur, v), want, "packet({ts}, {dur}, {v})");
}

#[test]
fn steps_packet_plan() {
    // Ends before the video starts: wait.
    expect_plan(V - 2 * PKT, PKT, V, PacketStart::Wait);
    // Ends exactly at the video's start: still wait.
    expect_plan(V - PKT, PKT, V, PacketStart::Wait);
    // Ends one ns into the video: start, truncated and cleared.
    expect_plan(V - PKT + 1, PKT, V, start(true, true, false));
    // Starts one ns before the video.
    expect_plan(V - 1, PKT, V, start(true, true, false));
    // Starts at the video: cleared, nothing truncated.
    expect_plan(V, PKT, V, start(false, true, false));
    // Starts one ns or one packet after: start from the buffer.
    expect_plan(V + 1, PKT, V, start(false, false, true));
    expect_plan(V + PKT, PKT, V, start(false, false, true));
    // A packet of 0 frames: wait at or before the video, else from the
    // buffer.
    expect_plan(V, 0, V, PacketStart::Wait);
    expect_plan(V + 1, 0, V, start(false, false, true));
    // The end is a u64 sum: a packet whose end wraps past u64::MAX waits
    // (characterized, not endorsed).
    expect_plan(u64::MAX - 10, PKT, V, PacketStart::Wait);
    expect_plan(u64::MAX - PKT, PKT, u64::MAX - 1, start(true, true, false));
    expect_plan(u64::MAX - PKT, PKT, u64::MAX, PacketStart::Wait);
    // A video start of 0 never gets here (no_video); with it, any packet
    // not ending at 0 starts from the buffer.
    expect_plan(1, 0, 0, start(false, false, true));
}

#[test]
fn steps_offsets() {
    assert_eq!(offset_ns(V, V - 20_834), 20_834);
    assert_eq!(offset_ns(V, 0), V);
    assert_eq!(offset_ns(V, V), 0);
    // A u64 difference: wraps when the audio is after the video, which the
    // callers never ask (characterized, not endorsed).
    assert_eq!(offset_ns(V, V + 1), u64::MAX);

    assert_eq!(offset_size(1, 4), 4);
    assert_eq!(offset_size(1024, 8), 8192);
    assert_eq!(offset_size(0, 8), 0);
    assert_eq!(offset_size(7, 0), 0);
    if usize::BITS > 32 {
        assert_eq!(offset_size(0x1_0000_0000, 4), 0x4_0000_0000);
        assert_eq!(offset_size(u64::MAX, 1), usize::MAX);
        // A size_t product: wraps (characterized, not endorsed).
        assert_eq!(offset_size(0x4000_0000_0000_0001, 4), 4);
    }

    assert!(buffer_offset(V - 1, V));
    assert!(!buffer_offset(V, V));
    assert!(!buffer_offset(V + 1, V));
    assert!(buffer_offset(0, 1));
    assert!(!buffer_offset(u64::MAX, u64::MAX));
}

#[test]
fn steps_push_size() {
    // The offset below, at and above the packet.
    assert_eq!(push_size(4096, 0), Some(4096));
    assert_eq!(push_size(4096, 4), Some(4092));
    assert_eq!(push_size(4096, 4092), Some(4));
    assert_eq!(push_size(4096, 4095), Some(1));
    assert_eq!(push_size(4096, 4096), None);
    assert_eq!(push_size(4096, 4097), None);
    assert_eq!(push_size(4096, usize::MAX), None);
    assert_eq!(push_size(0, 0), None);
    assert_eq!(push_size(usize::MAX, 0), Some(usize::MAX));
    assert_eq!(push_size(usize::MAX, usize::MAX - 1), Some(1));
}

#[test]
fn steps_can_send() {
    assert!(!can_send(4095, 4096));
    assert!(can_send(4096, 4096));
    assert!(can_send(4097, 4096));
    assert!(!can_send(0, 1));
    // A frame of 0 bytes is always ready (characterized, not endorsed).
    assert!(can_send(0, 0));
    assert!(can_send(usize::MAX, usize::MAX));
    assert!(!can_send(usize::MAX - 1, usize::MAX));
}

#[test]
fn steps_audio_pts() {
    assert_eq!(audio_pts(0, 1024), 1024);
    assert_eq!(audio_pts(1024, 1024), 2048);
    assert_eq!(audio_pts(-1024, 960), -64);
    assert_eq!(audio_pts(5, 0), 5);
    // Computed in size_t and converted back: wraps (characterized, not
    // endorsed).
    assert_eq!(audio_pts(i64::MAX, 1), i64::MIN);
    assert_eq!(audio_pts(-1, usize::MAX), -2);
}

// ---------------------------------------------------------------------------
// The setters

#[test]
fn steps_setter_not_video() {
    assert!(!not_video(OBS_ENCODER_VIDEO));
    assert!(!not_video(1));
    // OBS_ENCODER_AUDIO and types outside the enum.
    assert!(not_video(0));
    assert!(not_video(2));
    assert!(not_video(-1));
    assert!(not_video(i32::MAX));
    assert!(not_video(i32::MIN));
}

#[test]
fn steps_divisor_ok() {
    assert!(!divisor_ok(0));
    assert!(divisor_ok(1));
    assert!(divisor_ok(2));
    assert!(divisor_ok(u32::MAX));
}

// ---------------------------------------------------------------------------
// buffer_audio() end to end, on a stand-in encoder

/// The C test's `struct stand_in`: `obs-encoder.c`'s code with the deques
/// reduced to the byte count of plane 0.
#[derive(Clone, Debug, Default)]
struct StandIn {
    samplerate: u32,
    blocksize: usize,
    start_ts: u64,
    first_raw_ts: u64,
    has_paired: bool,
    v_start_ts: u64,
    buffered: usize,
    last_push: usize,
    last_offset: usize,
    pushes: u32,
    clears: u32,
    from_buffers: u32,
}

impl StandIn {
    fn new(
        samplerate: u32,
        blocksize: usize,
        paired: bool,
        v_start_ts: u64,
        first_ts: u64,
    ) -> Self {
        Self {
            samplerate,
            blocksize,
            has_paired: paired,
            v_start_ts,
            first_raw_ts: first_ts,
            ..Self::default()
        }
    }

    fn push_back(&mut self, size: usize, offset: usize) {
        let Some(size) = push_size(size, offset) else {
            return;
        };
        self.buffered = self.buffered.wrapping_add(size);
        self.last_push = size;
        self.last_offset = offset;
        self.pushes += 1;
    }

    fn calc_offset_size(&self, v_start_ts: u64, a_start_ts: u64) -> usize {
        let offset = offset_ns(v_start_ts, a_start_ts);
        let offset = util_mul_div64(offset, u64::from(self.samplerate), NS);
        offset_size(offset, self.blocksize)
    }

    fn start_from_buffer(&mut self, v_start_ts: u64) {
        let size = self.buffered;
        let mut offset = 0;
        self.buffered = 0;
        self.from_buffers += 1;
        if buffer_offset(self.first_raw_ts, v_start_ts) {
            offset = self.calc_offset_size(v_start_ts, self.first_raw_ts);
        }
        self.push_back(size, offset);
    }

    fn buffer_audio(&mut self, timestamp: u64, frames: u32) -> bool {
        let size = packet_size(frames, self.blocksize);
        let mut offset = 0;
        let mut success = true;

        if paired_start(self.start_ts, self.has_paired) {
            let v_start_ts = self.v_start_ts;
            if no_video(v_start_ts) {
                success = false;
            } else {
                let duration = util_mul_div64(u64::from(frames), NS, u64::from(self.samplerate));
                match packet_start(timestamp, duration, v_start_ts) {
                    PacketStart::Wait => success = false,
                    PacketStart::Start {
                        truncate,
                        clear,
                        from_buffer,
                    } => {
                        if truncate {
                            offset = self.calc_offset_size(v_start_ts, timestamp);
                        }
                        if clear {
                            self.buffered = 0;
                            self.clears += 1;
                        }
                        self.start_ts = v_start_ts;
                        if from_buffer {
                            self.start_from_buffer(v_start_ts);
                        }
                    }
                }
            }
        } else if unpaired_start(self.start_ts, self.has_paired) {
            self.start_ts = timestamp;
        }

        self.push_back(size, offset);
        success
    }
}

const BLOCKSIZES: [usize; 2] = [4, 8];

#[test]
fn steps_e2e_before_by_one_sample() {
    for bs in BLOCKSIZES {
        let mut e = StandIn::new(48000, bs, true, V, V - SAMPLE);
        assert!(e.buffer_audio(V - SAMPLE, 1024));
        assert_eq!(e.start_ts, V);
        assert_eq!(e.clears, 1);
        assert_eq!(e.from_buffers, 0);
        // The first frame is dropped.
        assert_eq!(e.last_offset, bs);
        assert_eq!(e.buffered, 1023 * bs);
    }
    // 20833 ns is less than one frame: nothing is dropped (characterized,
    // not endorsed: the conversion rounds down).
    let mut e = StandIn::new(48000, 4, true, V, V - SAMPLE + 1);
    assert!(e.buffer_audio(V - SAMPLE + 1, 1024));
    assert_eq!(e.buffered, 4096);
    assert_eq!(e.last_offset, 0);
}

#[test]
fn steps_e2e_at_video() {
    for bs in BLOCKSIZES {
        let mut e = StandIn::new(48000, bs, true, V, V);
        e.buffered = 3 * bs; // left from before: cleared
        assert!(e.buffer_audio(V, 1024));
        assert_eq!(e.start_ts, V);
        assert_eq!(e.clears, 1);
        assert_eq!(e.buffered, 1024 * bs);
        assert_eq!(e.last_offset, 0);
    }
}

#[test]
fn steps_e2e_before_by_one_packet() {
    for bs in BLOCKSIZES {
        let mut e = StandIn::new(48000, bs, true, V, V - PKT);
        assert!(!e.buffer_audio(V - PKT, 1024));
        assert_eq!(e.start_ts, 0);
        assert_eq!(e.buffered, 1024 * bs);
        assert!(e.buffer_audio(V, 1024));
        assert_eq!(e.start_ts, V);
        assert_eq!(e.clears, 1);
        assert_eq!(e.buffered, 1024 * bs);
    }
}

#[test]
fn steps_e2e_ends_at_video() {
    let mut e = StandIn::new(48000, 4, true, V, V - PKT);
    assert!(!e.buffer_audio(V - PKT, 1024));
    assert_eq!(e.start_ts, 0);
    assert_eq!(e.pushes, 1);
    assert_eq!(e.buffered, 4096);
    // One ns later it starts, its first 1023 frames dropped.
    let mut e = StandIn::new(48000, 4, true, V, V - PKT + 1);
    assert!(e.buffer_audio(V - PKT + 1, 1024));
    assert_eq!(e.start_ts, V);
    assert_eq!(e.last_offset, 1023 * 4);
    assert_eq!(e.buffered, 4);
}

#[test]
fn steps_e2e_after_video() {
    for bs in BLOCKSIZES {
        // Audio from V - PKT; the video at V - PKT + SAMPLE.
        let mut e = StandIn::new(48000, bs, true, 0, V - PKT);
        assert!(!e.buffer_audio(V - PKT, 1024)); // no video yet
        assert_eq!(e.buffered, 1024 * bs);
        e.v_start_ts = V - PKT + SAMPLE;
        assert!(e.buffer_audio(V, 1024));
        assert_eq!(e.start_ts, V - PKT + SAMPLE);
        assert_eq!(e.from_buffers, 1);
        assert_eq!(e.clears, 0);
        // One frame of the buffer dropped, then the packet whole.
        assert_eq!(e.buffered, 1023 * bs + 1024 * bs);

        // Audio from V - 2 PKT; the video at V - PKT (one packet after).
        let mut e = StandIn::new(48000, bs, true, 0, V - 2 * PKT);
        assert!(!e.buffer_audio(V - 2 * PKT, 1024));
        assert!(!e.buffer_audio(V - PKT, 1024));
        assert_eq!(e.buffered, 2048 * bs);
        e.v_start_ts = V - PKT;
        assert!(e.buffer_audio(V, 1024));
        assert_eq!(e.from_buffers, 1);
        // 21333333 ns is 1023 frames (rounded down): one frame of the first
        // packet stays (characterized, not endorsed).
        assert_eq!(e.buffered, (2048 - 1023) * bs + 1024 * bs);
    }
}

#[test]
fn steps_e2e_buffer_against_offset() {
    // Below: the buffer is shorter than the offset; it is dropped whole.
    let mut e = StandIn::new(48000, 4, true, V, V - 3 * PKT);
    e.buffered = 1024 * 4;
    e.start_ts = 0;
    assert!(e.buffer_audio(V + 1, 1024));
    assert_eq!(e.from_buffers, 1);
    assert_eq!(e.buffered, 4096);
    assert_eq!(e.pushes, 1);

    // At: the offset equals the buffer; nothing of it is pushed.
    let mut e = StandIn::new(48000, 4, true, V, V - 2 * SAMPLE);
    e.buffered = 2 * 4;
    assert!(e.buffer_audio(V + 1, 1024));
    assert_eq!(e.buffered, 4096);
    assert_eq!(e.pushes, 1);

    // Above: what is past the offset is kept.
    let mut e = StandIn::new(48000, 4, true, V, V - 2 * SAMPLE);
    e.buffered = 5 * 4;
    assert!(e.buffer_audio(V + 1, 1024));
    assert_eq!(e.pushes, 2);
    assert_eq!(e.buffered, 3 * 4 + 4096);

    // The first audio at or after the video: no offset.
    let mut e = StandIn::new(48000, 8, true, V, V);
    e.buffered = 5 * 8;
    assert!(e.buffer_audio(V + 1, 1024));
    assert_eq!(e.buffered, 5 * 8 + 8192);
}

#[test]
fn steps_e2e_unpaired_and_started() {
    // Unpaired: starts at its first packet, whatever its time.
    let mut e = StandIn::new(48000, 4, false, 0, V - 7);
    assert!(e.buffer_audio(V - 7, 1024));
    assert_eq!(e.start_ts, V - 7);
    assert_eq!(e.buffered, 4096);
    assert!(e.buffer_audio(V + PKT, 1024));
    assert_eq!(e.start_ts, V - 7);
    assert_eq!(e.buffered, 8192);

    // Started: every packet is buffered whole.
    let mut e = StandIn::new(44100, 8, true, V, V);
    e.start_ts = V;
    assert!(e.buffer_audio(V - 5 * PKT, 1024));
    assert_eq!(e.buffered, 8192);
    assert_eq!(e.clears, 0);
    assert_eq!(e.from_buffers, 0);

    // Paired, no video yet: waits, buffered whole.
    let mut e = StandIn::new(48000, 4, true, 0, V);
    assert!(!e.buffer_audio(V, 1024));
    assert_eq!(e.start_ts, 0);
    assert_eq!(e.buffered, 4096);
}

// ---------------------------------------------------------------------------
// Edge cases: models of the original C

/// `obs_encoder_audio_start_packet()` as the C writes it.
fn model_packet(ts: u64, dur: u64, v: u64) -> PacketStart {
    let end = ts.wrapping_add(dur);
    if end <= v {
        PacketStart::Wait
    } else {
        start(ts < v, ts <= v, v < ts)
    }
}

/// Values around a centre: the centre, one either side, and a packet either
/// side.
fn around(c: u64) -> [u64; 7] {
    [
        c,
        c.wrapping_add(1),
        c.wrapping_sub(1),
        c.wrapping_add(PKT),
        c.wrapping_sub(PKT),
        c.wrapping_add(PKT + 1),
        c.wrapping_sub(PKT - 1),
    ]
}

#[test]
fn edge_packet_around_video() {
    let durations = [0, 1, PKT - 1, PKT, PKT + 1, 2 * PKT, u64::MAX];
    for &v in EDGE_U64.iter().chain(&[V, PKT]) {
        for ts in around(v).into_iter().chain(EDGE_U64) {
            for dur in durations {
                assert_eq!(
                    packet_start(ts, dur, v),
                    model_packet(ts, dur, v),
                    "packet({ts}, {dur}, {v})"
                );
            }
        }
    }
}

#[test]
fn edge_packet_flags_consistent() {
    // Exactly one of clear and from_buffer, and truncate only with clear.
    for v in [1, V, u64::MAX] {
        for ts in around(v) {
            if let PacketStart::Start {
                truncate,
                clear,
                from_buffer,
            } = packet_start(ts, PKT, v)
            {
                assert_ne!(clear, from_buffer, "packet({ts}, {PKT}, {v})");
                assert!(!truncate || clear, "packet({ts}, {PKT}, {v})");
            }
        }
    }
}

#[test]
fn edge_setter_types() {
    for t in (-8..=8).chain([i32::MIN, i32::MAX]) {
        assert_eq!(not_video(t), t != 1, "not_video({t})");
    }
    assert_eq!(OBS_ENCODER_VIDEO, 1);
}

#[test]
fn edge_pts_wrap() {
    // video_pts wraps in i64 (the C's int64_t sum overflows only past
    // INT64_MAX, which a pts never reaches).
    assert_eq!(video_pts(i64::MAX, 1, 1), i64::MIN);
    assert_eq!(video_pts(i64::MIN, 0, 0), i64::MIN);
    assert_eq!(audio_pts(i64::MIN, 0), i64::MIN);
    assert_eq!(audio_pts(i64::MAX, usize::MAX), i64::MAX.wrapping_sub(1));
}

#[test]
fn edge_two_value_decisions() {
    let sizes = [0, 1, 2, 4095, 4096, 4097, usize::MAX - 1, usize::MAX];
    for &a in &sizes {
        for &b in &sizes {
            let want = if b >= a { None } else { Some(a - b) };
            assert_eq!(push_size(a, b), want, "push_size({a}, {b})");
            assert_eq!(can_send(a, b), a >= b, "can_send({a}, {b})");
        }
    }
    for &a in &EDGE_U64 {
        for &b in &EDGE_U64 {
            assert_eq!(buffer_offset(a, b), a < b, "buffer_offset({a}, {b})");
            assert_eq!(offset_ns(a, b), a.wrapping_sub(b), "offset_ns({a}, {b})");
            assert_eq!(
                paired_start(a, b != 0),
                a == 0 && b != 0,
                "paired_start({a}, {b})"
            );
            assert_eq!(
                unpaired_start(a, b != 0),
                a == 0 && b == 0,
                "unpaired_start({a}, {b})"
            );
        }
        assert_eq!(no_video(a), a == 0);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    #[test]
    fn edge_proptest_decisions(
        a in any::<u64>(),
        b in any::<u64>(),
        d in any::<u64>(),
        frames in any::<u32>(),
        blocksize in any::<usize>(),
        size in any::<usize>(),
        offset in any::<usize>(),
        pts in any::<i64>(),
        tb in any::<u32>(),
        div in any::<u32>(),
        t in any::<i32>(),
    ) {
        prop_assert_eq!(group_ready(a, b), a == b);
        prop_assert_eq!(paired_late(a, b), a > b);
        prop_assert_eq!(video_pts(pts, tb, div), pts.wrapping_add(i64::from(tb.wrapping_mul(div))));
        prop_assert_eq!(
            packet_size(frames, blocksize),
            usize::try_from(frames).unwrap().wrapping_mul(blocksize)
        );
        prop_assert_eq!(packet_start(a, d, b), model_packet(a, d, b));
        prop_assert_eq!(offset_ns(a, b), a.wrapping_sub(b));
        prop_assert_eq!(
            offset_size(d, blocksize),
            usize::try_from(d & u64::try_from(usize::MAX).unwrap()).unwrap().wrapping_mul(blocksize)
        );
        prop_assert_eq!(buffer_offset(a, b), a < b);
        prop_assert_eq!(push_size(size, offset), size.checked_sub(offset).filter(|&s| s != 0));
        prop_assert_eq!(can_send(size, offset), size >= offset);
        prop_assert_eq!(
            audio_pts(pts, size),
            pts.cast_unsigned().wrapping_add(u64::try_from(size).unwrap()).cast_signed()
        );
        prop_assert_eq!(not_video(t), t != OBS_ENCODER_VIDEO);
        prop_assert_eq!(divisor_ok(div), div != 0);
    }

    /// The stand-in over packet sequences against the same code written with
    /// the C's comparisons inline.
    #[test]
    fn edge_proptest_stand_in(
        samplerate in prop_oneof![Just(48000u32), Just(44100), Just(8000), 1u32..400_000],
        blocksize in prop_oneof![Just(4usize), Just(8), 1usize..64],
        paired in any::<bool>(),
        v_start in prop_oneof![Just(0u64), V - 3 * PKT..V + 3 * PKT],
        first in V - 4 * PKT..V + 2 * PKT,
        packets in proptest::collection::vec((0u32..4096, 0u64..2 * PKT), 1..8),
    ) {
        let mut e = StandIn::new(samplerate, blocksize, paired, v_start, first);
        let mut m = Model::new(samplerate, blocksize, paired, v_start, first);
        let mut ts = first;
        for (frames, gap) in packets {
            prop_assert_eq!(e.buffer_audio(ts, frames), m.buffer_audio(ts, frames));
            prop_assert_eq!(
                (e.start_ts, e.buffered, e.pushes, e.clears, e.from_buffers),
                (m.start_ts, m.buffered, m.pushes, m.clears, m.from_buffers)
            );
            ts = ts.wrapping_add(gap);
        }
    }
}

/// `buffer_audio()` and its helpers as `obs-encoder.c` wrote them on
/// origin/master, before the move: the comparisons inline.
struct Model {
    samplerate: u32,
    blocksize: usize,
    start_ts: u64,
    first_raw_ts: u64,
    has_paired: bool,
    v_start_ts: u64,
    buffered: usize,
    pushes: u32,
    clears: u32,
    from_buffers: u32,
}

impl Model {
    fn new(samplerate: u32, blocksize: usize, paired: bool, v_start_ts: u64, first: u64) -> Self {
        Self {
            samplerate,
            blocksize,
            start_ts: 0,
            first_raw_ts: first,
            has_paired: paired,
            v_start_ts,
            buffered: 0,
            pushes: 0,
            clears: 0,
            from_buffers: 0,
        }
    }

    fn push_back(&mut self, size: usize, offset: usize) {
        if offset >= size {
            return;
        }
        self.buffered = self.buffered.wrapping_add(size - offset);
        self.pushes += 1;
    }

    fn calc_offset_size(&self, v: u64, a: u64) -> usize {
        let offset = util_mul_div64(v.wrapping_sub(a), u64::from(self.samplerate), NS);
        usize::try_from(offset)
            .unwrap()
            .wrapping_mul(self.blocksize)
    }

    fn buffer_audio(&mut self, ts: u64, frames: u32) -> bool {
        let size = usize::try_from(frames)
            .unwrap()
            .wrapping_mul(self.blocksize);
        let mut offset = 0;
        let mut success = true;
        if self.start_ts == 0 && self.has_paired {
            let v = self.v_start_ts;
            if v == 0 {
                success = false;
            } else {
                let end = ts.wrapping_add(util_mul_div64(
                    u64::from(frames),
                    NS,
                    u64::from(self.samplerate),
                ));
                if end <= v {
                    success = false;
                } else {
                    if ts < v {
                        offset = self.calc_offset_size(v, ts);
                    }
                    if ts <= v {
                        self.buffered = 0;
                        self.clears += 1;
                    }
                    self.start_ts = v;
                    if v < ts {
                        let size = self.buffered;
                        let mut off = 0;
                        self.buffered = 0;
                        self.from_buffers += 1;
                        if self.first_raw_ts < v {
                            off = self.calc_offset_size(v, self.first_raw_ts);
                        }
                        self.push_back(size, off);
                    }
                }
            }
        } else if self.start_ts == 0 && !self.has_paired {
            self.start_ts = ts;
        }
        self.push_back(size, offset);
        success
    }
}

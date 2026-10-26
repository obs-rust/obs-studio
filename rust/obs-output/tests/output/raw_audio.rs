//! Tier 1: `obs_output::raw_audio`, the raw output audio admission and the
//! capture hookup.
//!
//! The `steps_*` cases port `test/cmocka/test_output_raw_audio_steps.c` 1:1:
//! its 19 cases under the same names, in the same order, with the same values
//! and expectations, the "characterized, not endorsed" ones included. The C
//! calls the hidden `obs_output_raw_audio_*` functions; here the safe core is
//! called directly. The C's stand-in output (`prepare_audio()`, the block loop
//! of `default_raw_audio_callback()`, `start_raw_audio()` and
//! `hook_data_capture()`, the deques reduced to a byte count) is [`StandIn`],
//! with the same `util_mul_div64()` (the portable formula, as Linux and macOS
//! build it) and `obs_output_first_mixer()` from `video_checks`.
//!
//! The `edge_*` cases are new: every decision against a model of the
//! original C over edge values and every low flag word and mask, the
//! arithmetic at the integer edges, and proptests of every decision over
//! full-range inputs and of the stand-in over packet sequences against the
//! origin/master code with its expressions inline.

use obs_output::raw_audio::{
    AUDIO_OUTPUT_FRAMES, OBS_OUTPUT_VIDEO, before_video, block_bytes, block_ts, can_pop, cut_bytes,
    cut_frames, cut_ns, cut_ts, delay_ns, delay_on, ends_before, interleave, mix_on, next_frames,
    passthrough, paused_ts, push_bytes, start_unset, video_unset,
};
use obs_output::video_checks::first_mixer;
use proptest::prelude::*;

const NS: u64 = 1_000_000_000;
/// One 1024-frame packet at 48 kHz: `util_mul_div64(1024, NS, 48000)`.
const PKT: u64 = 21_333_333;

const EDGE_U64: [u64; 7] = [
    0,
    1,
    2,
    0x7fff_ffff_ffff_ffff,
    0x8000_0000_0000_0000,
    u64::MAX - 1,
    u64::MAX,
];

const EDGE_USIZE: [usize; 9] = [
    0,
    1,
    2,
    4,
    8,
    4096,
    u32::MAX as usize,
    usize::MAX - 1,
    usize::MAX,
];

/// `util_mul_div64()`'s portable formula, as `libobs/util/util_uint64.h`
/// builds it off MSVC x64. Test only; the port never divides.
fn util_mul_div64(num: u64, mul: u64, div: u64) -> u64 {
    let rem = num % div;
    (num / div)
        .wrapping_mul(mul)
        .wrapping_add(rem.wrapping_mul(mul) / div)
}

// ------------------------------------------------------------------------
// test_output_raw_audio_steps.c, 1:1

#[test]
fn steps_constants() {
    assert_eq!(OBS_OUTPUT_VIDEO, 1);
    assert_eq!(AUDIO_OUTPUT_FRAMES, 1024);
}

#[test]
fn steps_passthrough() {
    for f in 0..0x10000u32 {
        assert_eq!(passthrough(f), f & 1 == 0, "{f:#x}");
    }
    assert!(!passthrough(u32::MAX));
    assert!(passthrough(u32::MAX - 1));
    // OBS_OUTPUT_AUDIO alone (2), audio and encoded (6) pass
    assert!(passthrough(2));
    assert!(passthrough(6));
    assert!(!passthrough(3));
}

#[test]
fn steps_unset() {
    for v in EDGE_U64 {
        assert_eq!(video_unset(v), v == 0);
        assert_eq!(start_unset(v), v == 0);
    }
}

#[test]
fn steps_before_video() {
    for a in EDGE_U64 {
        for b in EDGE_U64 {
            assert_eq!(before_video(a, b), a < b);
        }
    }
    assert!(before_video(NS - 1, NS));
    assert!(!before_video(NS, NS));
    assert!(!before_video(NS + 1, NS));
}

#[test]
fn steps_ends_before() {
    // a packet ending exactly at the video start is dropped
    assert!(ends_before(NS - PKT, PKT, NS));
    assert!(!ends_before(NS - PKT + 1, PKT, NS));
    assert!(ends_before(NS - 2 * PKT, PKT, NS));
    // a 0-frame packet before the video ends before it
    assert!(ends_before(NS - 1, 0, NS));
    // the end wraps past u64::MAX and so is "before" the video
    // (characterized, not endorsed; timestamps never get there)
    assert!(ends_before(u64::MAX - 1, 3, u64::MAX));
    assert!(ends_before(10, u64::MAX, 100));
    for a in EDGE_U64 {
        for b in EDGE_U64 {
            for c in EDGE_U64 {
                assert_eq!(ends_before(a, b, c), a.wrapping_add(b) <= c);
            }
        }
    }
}

#[test]
fn steps_cut() {
    assert_eq!(cut_ns(NS, NS - 20834), 20834);
    assert_eq!(cut_ns(NS, NS), 0);
    // u64 difference, wrapping (only asked with timestamp before)
    assert_eq!(cut_ns(0, 1), u64::MAX);
    assert_eq!(cut_ts(NS - 20834, 20834), NS);
    assert_eq!(cut_ts(u64::MAX, 2), 1);

    assert_eq!(cut_bytes(4, 1), 4);
    assert_eq!(cut_bytes(8, 1023), 8184);
    assert_eq!(cut_bytes(0, 1000), 0);
    assert_eq!(cut_bytes(4, 0), 0);
    // the frames are truncated to u32 first (characterized, not endorsed;
    // a cut is always shorter than its packet)
    assert_eq!(cut_bytes(4, 0x1_0000_0001), 4);
    assert_eq!(cut_bytes(4, 0x1_0000_0000), 0);
    // a usize product: no 32-bit wrap
    #[cfg(target_pointer_width = "64")]
    assert_eq!(cut_bytes(8, u64::from(u32::MAX)), u32::MAX as usize * 8);

    assert_eq!(cut_frames(1024, 1), 1023);
    assert_eq!(cut_frames(1024, 1023), 1);
    assert_eq!(cut_frames(1024, 0), 1024);
    // u32, wrapping, after the same truncation
    assert_eq!(cut_frames(1024, 1025), u32::MAX);
    assert_eq!(cut_frames(1024, 0x1_0000_0001), 1023);
}

#[test]
fn steps_block_bytes() {
    assert_eq!(block_bytes(4), 4096);
    assert_eq!(block_bytes(8), 8192);
    assert_eq!(block_bytes(0), 0);
    assert_eq!(block_bytes(usize::MAX), 0usize.wrapping_sub(1024));

    assert_eq!(push_bytes(1024, 4), 4096);
    assert_eq!(push_bytes(480, 8), 3840);
    assert_eq!(push_bytes(0, 8), 0);
    assert_eq!(push_bytes(1, 0), 0);
    #[cfg(target_pointer_width = "64")]
    assert_eq!(push_bytes(u32::MAX, 8), u32::MAX as usize * 8);
}

#[test]
fn steps_can_pop() {
    // strictly more than one block: exactly one block stays buffered
    // (characterized, not endorsed)
    assert!(!can_pop(4096, 4096));
    assert!(can_pop(4097, 4096));
    assert!(!can_pop(4095, 4096));
    assert!(can_pop(8192, 4096));
    assert!(!can_pop(0, 0));
    // a block of 0 bytes pops while anything is buffered
    assert!(can_pop(1, 0));
    assert!(!can_pop(usize::MAX, usize::MAX));
}

#[test]
fn steps_block_ts() {
    assert_eq!(block_ts(NS, 0), NS);
    assert_eq!(block_ts(NS, PKT), NS + PKT);
    assert_eq!(block_ts(u64::MAX, 1), 0);
    assert_eq!(paused_ts(NS, 0), NS);
    assert_eq!(paused_ts(NS, 5 * NS), 6 * NS);
    assert_eq!(paused_ts(u64::MAX, 2), 1);

    assert_eq!(next_frames(0), 1024);
    assert_eq!(next_frames(1024), 2048);
    assert_eq!(next_frames(usize::MAX), 1023);
}

#[test]
fn steps_mix_on() {
    for mask in 0..0x100usize {
        for idx in 0..8usize {
            assert_eq!(mix_on(mask, idx), (mask >> idx) & 1 != 0);
        }
    }
    assert!(mix_on(usize::MAX, 5));
    assert!(!mix_on(usize::MAX - 32, 5));
}

#[test]
fn steps_hook_rules() {
    assert!(interleave(true, true));
    assert!(!interleave(true, false));
    assert!(!interleave(false, true));
    assert!(!interleave(false, false));

    assert!(!delay_on(0));
    assert!(delay_on(1));
    assert!(delay_on(u32::MAX));

    assert_eq!(delay_ns(0), 0);
    assert_eq!(delay_ns(1), NS);
    assert_eq!(delay_ns(30), 30 * NS);
    // a u64 product: no 32-bit wrap
    assert_eq!(delay_ns(u32::MAX), u64::from(u32::MAX) * NS);
}

// The callers, on a stand-in output (the C's struct stand_in).

const SI_PLANES: usize = 8;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct StandIn {
    flags: u32,
    video_start_ts: u64,
    last_video_ts: u64,
    ts_offset: u64,
    audio_start_ts: u64,
    sample_rate: u64,
    audio_size: usize,
    planes: usize,
    total_audio_frames: usize,
    buffered: [usize; SI_PLANES],
    block_ts: Vec<u64>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Packet {
    timestamp: u64,
    frames: u32,
    /// bytes data[0] moved forward
    advance: usize,
}

impl StandIn {
    fn new(flags: u32, video_start_ts: u64, last_video_ts: u64) -> Self {
        Self {
            flags,
            video_start_ts,
            last_video_ts,
            sample_rate: 48000,
            audio_size: 4,
            planes: 2,
            ..Self::default()
        }
    }

    /// `prepare_audio()`
    fn prepare(&mut self, old: Packet) -> Option<Packet> {
        if passthrough(self.flags) {
            return Some(old);
        }
        if video_unset(self.video_start_ts) {
            self.video_start_ts = self.last_video_ts;
        }
        if video_unset(self.video_start_ts) {
            return None;
        }
        let mut new = old;
        if before_video(old.timestamp, self.video_start_ts) {
            let duration = util_mul_div64(u64::from(old.frames), NS, self.sample_rate);
            if ends_before(old.timestamp, duration, self.video_start_ts) {
                return None;
            }
            let cutoff = cut_ns(self.video_start_ts, old.timestamp);
            new.timestamp = cut_ts(new.timestamp, cutoff);
            let cutoff = util_mul_div64(cutoff, self.sample_rate, NS);
            new.advance = new.advance.wrapping_add(cut_bytes(self.audio_size, cutoff));
            new.frames = cut_frames(new.frames, cutoff);
        }
        Some(new)
    }

    /// `default_raw_audio_callback()`, with `data_active()` true and no
    /// pause.
    fn callback(&mut self, timestamp: u64, frames: u32) -> Option<Packet> {
        let out = self.prepare(Packet {
            timestamp,
            frames,
            advance: 0,
        })?;
        if start_unset(self.audio_start_ts) {
            self.audio_start_ts = out.timestamp;
        }
        let frame_size_bytes = block_bytes(self.audio_size);
        for i in 0..self.planes {
            self.buffered[i] =
                self.buffered[i].wrapping_add(push_bytes(out.frames, self.audio_size));
        }
        while can_pop(self.buffered[0], frame_size_bytes) {
            for i in 0..self.planes {
                self.buffered[i] = self.buffered[i].wrapping_sub(frame_size_bytes);
            }
            let ts = block_ts(
                self.audio_start_ts,
                util_mul_div64(self.total_audio_frames as u64, NS, self.sample_rate),
            );
            let ts = paused_ts(ts, self.ts_offset);
            self.total_audio_frames = next_frames(self.total_audio_frames);
            self.block_ts.push(ts);
            assert!(self.block_ts.len() <= 64, "runaway block loop");
        }
        Some(out)
    }
}

const V: u64 = 10 * NS;
/// `OBS_OUTPUT_AV`
const AV: u32 = 3;

#[test]
fn steps_e2e_before_by_one_sample() {
    // 20834 ns is just over one sample at 48 kHz: one frame dropped
    let mut o = StandIn::new(AV, V, 0);
    let out = o.callback(V - 20834, 1024).expect("admitted");
    assert_eq!(out.timestamp, V);
    assert_eq!(out.frames, 1023);
    assert_eq!(out.advance, 4);
    assert_eq!(o.audio_start_ts, V);
    assert_eq!(o.buffered[0], 1023 * 4);
    assert_eq!(o.buffered[1], 1023 * 4);

    // 20833 ns is under one sample: the timestamp moves to the video's, no
    // frame is dropped (characterized, not endorsed: the conversion rounds
    // down)
    let mut o = StandIn::new(AV, V, 0);
    let out = o.callback(V - 20833, 1024).expect("admitted");
    assert_eq!(out.timestamp, V);
    assert_eq!(out.frames, 1024);
    assert_eq!(out.advance, 0);
}

#[test]
fn steps_e2e_ends_at_video() {
    // a packet ending exactly at the video start is dropped
    let mut o = StandIn::new(AV, V, 0);
    assert_eq!(o.callback(V - PKT, 1024), None);
    assert_eq!(o.audio_start_ts, 0);
    assert_eq!(o.buffered[0], 0);
    // and one a packet earlier
    assert_eq!(o.callback(V - 2 * PKT, 1024), None);

    // one nanosecond later: 1023 frames are cut, one is kept (the
    // conversion rounds down)
    let mut o = StandIn::new(AV, V, 0);
    let out = o.callback(V - PKT + 1, 1024).expect("admitted");
    assert_eq!(out.timestamp, V);
    assert_eq!(out.frames, 1);
    assert_eq!(out.advance, 1023 * 4);
    assert_eq!(o.buffered[0], 4);
}

#[test]
fn steps_e2e_at_and_after_video() {
    let mut o = StandIn::new(AV, V, 0);
    let out = o.callback(V, 1024).expect("admitted");
    assert_eq!(out.timestamp, V);
    assert_eq!(out.frames, 1024);
    assert_eq!(out.advance, 0);

    let mut o = StandIn::new(AV, V, 0);
    let out = o.callback(V + 1, 1024).expect("admitted");
    assert_eq!(out.timestamp, V + 1);
    assert_eq!(out.frames, 1024);
    assert_eq!(o.audio_start_ts, V + 1);
}

#[test]
fn steps_e2e_no_video_start() {
    // no video yet: the packet waits, nothing is buffered
    let mut o = StandIn::new(AV, 0, 0);
    assert_eq!(o.callback(V, 1024), None);
    assert_eq!(o.video_start_ts, 0);
    assert_eq!(o.buffered[0], 0);

    // the video start comes from the pause data's last video timestamp
    let mut o = StandIn::new(AV, 0, V);
    let out = o.callback(V - 20834, 1024).expect("admitted");
    assert_eq!(o.video_start_ts, V);
    assert_eq!(out.frames, 1023);

    // once set it stays, whatever the pause data says later
    o.last_video_ts = 2 * V;
    assert!(o.callback(V + PKT, 1024).is_some());
    assert_eq!(o.video_start_ts, V);
}

#[test]
fn steps_e2e_audio_only() {
    // an output without video takes every packet whole, video start or not
    let mut o = StandIn::new(2, 0, 0);
    let out = o.callback(5, 1024).expect("admitted");
    assert_eq!(out.timestamp, 5);
    assert_eq!(out.frames, 1024);
    assert_eq!(o.video_start_ts, 0);
    assert_eq!(o.audio_start_ts, 5);

    let mut o = StandIn::new(2, V, V);
    let out = o.callback(V - 2 * PKT, 1024).expect("admitted");
    assert_eq!(out.timestamp, V - 2 * PKT);
    assert_eq!(out.frames, 1024);
}

#[test]
fn steps_e2e_hold_back_one_block() {
    let mut o = StandIn::new(2, 0, 0);
    o.ts_offset = 7;
    // one block buffered: nothing sent (characterized, not endorsed: the
    // strict > holds the last full block back, and nothing sends it when
    // the output stops)
    assert!(o.callback(V, 1024).is_some());
    assert!(o.block_ts.is_empty());
    assert_eq!(o.buffered[0], 4096);
    // two: one sent, at the audio start plus the pause offset
    assert!(o.callback(V + PKT, 1024).is_some());
    assert_eq!(o.block_ts, [V + 7]);
    assert_eq!(o.buffered[0], 4096);
    assert_eq!(o.buffered[1], 4096);
    assert_eq!(o.total_audio_frames, 1024);
    // three: two sent, the second a block later
    assert!(o.callback(V + 2 * PKT, 1024).is_some());
    assert_eq!(o.block_ts, [V + 7, V + PKT + 7]);
    assert_eq!(o.total_audio_frames, 2048);

    // 1.5 blocks in one packet: one sent, half a block kept; 2.5: two
    // sent, half kept
    let mut o = StandIn::new(2, 0, 0);
    assert!(o.callback(V, 1536).is_some());
    assert_eq!(o.block_ts.len(), 1);
    assert_eq!(o.buffered[0], 512 * 4);
    let mut o = StandIn::new(2, 0, 0);
    assert!(o.callback(V, 2560).is_some());
    assert_eq!(o.block_ts.len(), 2);
    assert_eq!(o.buffered[0], 512 * 4);
    // three blocks in one packet: two sent
    let mut o = StandIn::new(2, 0, 0);
    assert!(o.callback(V, 3072).is_some());
    assert_eq!(o.block_ts.len(), 2);
    assert_eq!(o.buffered[0], 4096);
    // the block timestamps count from the first packet, not from each
    assert_eq!(o.block_ts, [V, V + PKT]);
}

/// `start_raw_audio()`: the mixes connected.
fn si_mixers(mixer_mask: usize, raw_audio2: bool) -> Vec<usize> {
    if raw_audio2 {
        (0..6).filter(|&idx| mix_on(mixer_mask, idx)).collect()
    } else {
        vec![first_mixer(mixer_mask)]
    }
}

#[test]
fn steps_e2e_mixers() {
    for mask in 0..0x100usize {
        let want: Vec<usize> = (0..6).filter(|i| mask & (1 << i) != 0).collect();
        assert_eq!(si_mixers(mask, true), want, "{mask:#x}");
        let first = want.first().copied().unwrap_or(0);
        assert_eq!(si_mixers(mask, false), [first], "{mask:#x}");
    }
    // no mix in the mask: raw_audio2 connects none, raw_audio mix 0
    // (characterized, not endorsed)
    assert!(si_mixers(0, true).is_empty());
    assert_eq!(si_mixers(0, false), [0]);
    // bits past MAX_AUDIO_MIXES are ignored by both
    assert!(si_mixers(0xc0, true).is_empty());
    assert_eq!(si_mixers(0xc0, false), [0]);
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Cb {
    #[default]
    Default,
    Interleave,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Hook {
    encoded: bool,
    cb: Cb,
    delay: bool,
    delay_ns: u64,
    audio: bool,
    video: bool,
}

/// `hook_data_capture()`
fn si_hook(flags: u32, delay_sec: u32) -> Hook {
    let has_video = flags & 1 != 0;
    let has_audio = flags & 2 != 0;
    let mut h = Hook::default();
    if flags & 4 != 0 {
        h.encoded = true;
        h.cb = if interleave(has_video, has_audio) {
            Cb::Interleave
        } else {
            Cb::Default
        };
        if delay_on(delay_sec) {
            h.delay = true;
            h.delay_ns = delay_ns(delay_sec);
        }
    }
    h.audio = has_audio;
    h.video = has_video;
    h
}

#[test]
fn steps_e2e_hook() {
    for flags in 0..16u32 {
        for d in [0, 1, 20, u32::MAX] {
            let h = si_hook(flags, d);
            let enc = flags & 4 != 0;
            assert_eq!(h.encoded, enc);
            let cb = if enc && flags & 3 == 3 {
                Cb::Interleave
            } else {
                Cb::Default
            };
            assert_eq!(h.cb, cb);
            // the delay applies only to an encoded output
            assert_eq!(h.delay, enc && d != 0);
            assert_eq!(h.delay_ns, if enc { u64::from(d) * NS } else { 0 });
            assert_eq!(h.audio, flags & 2 != 0);
            assert_eq!(h.video, flags & 1 != 0);
        }
    }
}

// ------------------------------------------------------------------------
// New edge cases

/// Edge `u64`s, and each one either side of the 48 kHz packet edges.
fn special_u64() -> Vec<u64> {
    let mut v: Vec<u64> = EDGE_U64.to_vec();
    for base in [NS, V, PKT, 20833, 20834] {
        for d in [0u64, 1, 2] {
            v.push(base.wrapping_add(d));
            v.push(base.wrapping_sub(d));
        }
    }
    v.push(u64::from(u32::MAX));
    v.push(u64::from(u32::MAX) + 1);
    v.sort_unstable();
    v.dedup();
    v
}

#[test]
fn edge_two_value_decisions() {
    let s = special_u64();
    for &a in &s {
        assert_eq!(video_unset(a), a == 0);
        assert_eq!(start_unset(a), a == 0);
        for &b in &s {
            assert_eq!(before_video(a, b), a < b);
            assert_eq!(cut_ns(a, b), a.wrapping_sub(b));
            assert_eq!(cut_ts(a, b), a.wrapping_add(b));
            assert_eq!(block_ts(a, b), a.wrapping_add(b));
            assert_eq!(paused_ts(a, b), a.wrapping_add(b));
            for &c in &s {
                assert_eq!(ends_before(a, b, c), a.wrapping_add(b) <= c);
            }
        }
    }
}

#[test]
fn edge_sizes() {
    let s = special_u64();
    for size in EDGE_USIZE {
        assert_eq!(block_bytes(size), 1024usize.wrapping_mul(size));
        assert_eq!(next_frames(size), size.wrapping_add(1024));
        for f in [0u32, 1, 480, 1023, 1024, 1025, u32::MAX - 1, u32::MAX] {
            assert_eq!(push_bytes(f, size), (f as usize).wrapping_mul(size));
        }
        for &c in &s {
            assert_eq!(cut_bytes(size, c), size.wrapping_mul(c as u32 as usize));
        }
        for other in EDGE_USIZE {
            assert_eq!(can_pop(size, other), size > other);
        }
    }
    for f in [0u32, 1, 1023, 1024, 1025, u32::MAX] {
        for &c in &s {
            assert_eq!(cut_frames(f, c), f.wrapping_sub(c as u32));
        }
    }
}

#[test]
fn edge_flags_masks_and_delay() {
    for f in 0..0x1_0000u32 {
        for high in [0u32, 0x1_0000, 0x8000_0000, 0xffff_0000] {
            let flags = f | high;
            assert_eq!(passthrough(flags), flags & 1 == 0);
        }
    }
    for mask in 0..0x1_0000usize {
        for high in [0usize, 1 << 31, usize::MAX << 16] {
            let m = mask | high;
            for idx in 0..usize::BITS as usize {
                assert_eq!(mix_on(m, idx), (m >> idx) & 1 != 0);
            }
        }
    }
    // past the width: the C's shift is undefined; the port says not set
    for idx in [
        usize::BITS as usize,
        usize::BITS as usize + 1,
        u32::MAX as usize,
        usize::MAX,
    ] {
        assert!(!mix_on(usize::MAX, idx));
    }
    for d in (0..=u32::MAX).step_by(65_521).chain([u32::MAX]) {
        assert_eq!(delay_on(d), d != 0);
        assert_eq!(delay_ns(d), u64::from(d) * NS);
    }
    for (v, a) in [(false, false), (false, true), (true, false), (true, true)] {
        assert_eq!(interleave(v, a), v && a);
    }
}

/// origin/master's `prepare_audio()` and block loop with their expressions
/// inline, on the same stand-in.
fn master_callback(o: &mut StandIn, timestamp: u64, frames: u32) -> Option<Packet> {
    let old = Packet {
        timestamp,
        frames,
        advance: 0,
    };
    let out = if o.flags & OBS_OUTPUT_VIDEO == 0 {
        old
    } else {
        if o.video_start_ts == 0 {
            o.video_start_ts = o.last_video_ts;
        }
        if o.video_start_ts == 0 {
            return None;
        }
        let mut new = old;
        if old.timestamp < o.video_start_ts {
            let duration = util_mul_div64(u64::from(old.frames), NS, o.sample_rate);
            let end_ts = old.timestamp.wrapping_add(duration);
            if end_ts <= o.video_start_ts {
                return None;
            }
            let mut cutoff = o.video_start_ts.wrapping_sub(old.timestamp);
            new.timestamp = new.timestamp.wrapping_add(cutoff);
            cutoff = util_mul_div64(cutoff, o.sample_rate, NS);
            new.advance = new
                .advance
                .wrapping_add(o.audio_size.wrapping_mul(cutoff as u32 as usize));
            new.frames = new.frames.wrapping_sub(cutoff as u32);
        }
        new
    };
    if o.audio_start_ts == 0 {
        o.audio_start_ts = out.timestamp;
    }
    let frame_size_bytes = 1024usize.wrapping_mul(o.audio_size);
    for i in 0..o.planes {
        o.buffered[i] =
            o.buffered[i].wrapping_add((out.frames as usize).wrapping_mul(o.audio_size));
    }
    while o.buffered[0] > frame_size_bytes {
        for i in 0..o.planes {
            o.buffered[i] = o.buffered[i].wrapping_sub(frame_size_bytes);
        }
        let ts = o
            .audio_start_ts
            .wrapping_add(util_mul_div64(
                o.total_audio_frames as u64,
                NS,
                o.sample_rate,
            ))
            .wrapping_add(o.ts_offset);
        o.total_audio_frames = o.total_audio_frames.wrapping_add(1024);
        o.block_ts.push(ts);
        assert!(o.block_ts.len() <= 64, "runaway block loop");
    }
    Some(out)
}

fn arb_start() -> impl Strategy<Value = StandIn> {
    (
        any::<u32>(),
        prop_oneof![Just(0u64), V - 3 * PKT..V + 3 * PKT, any::<u64>()],
        prop_oneof![Just(0u64), V - 3 * PKT..V + 3 * PKT],
        prop_oneof![Just(0u64), 0..NS],
        prop_oneof![Just(0u64), V - 3 * PKT..V + 3 * PKT],
        prop_oneof![
            Just(48000u64),
            Just(44100u64),
            Just(1u64),
            Just(384_000u64),
            1..1_000_000u64
        ],
        prop_oneof![Just(4usize), Just(8usize), 1..9usize],
        1..=SI_PLANES,
        prop_oneof![Just(0usize), 0..100_000usize, any::<usize>()],
    )
        .prop_map(
            |(
                flags,
                video_start_ts,
                last_video_ts,
                ts_offset,
                audio_start_ts,
                rate,
                size,
                planes,
                frames,
            )| StandIn {
                flags,
                video_start_ts,
                last_video_ts,
                ts_offset,
                audio_start_ts,
                sample_rate: rate,
                audio_size: size,
                planes,
                total_audio_frames: frames,
                ..StandIn::default()
            },
        )
}

fn arb_packets() -> impl Strategy<Value = Vec<(u64, u32)>> {
    prop::collection::vec(
        (
            prop_oneof![
                V - 3 * PKT..V + 3 * PKT,
                Just(V - PKT),
                Just(V),
                any::<u64>()
            ],
            prop_oneof![Just(1024u32), Just(0u32), 1..4097u32],
        ),
        0..6,
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    #[test]
    fn edge_proptest_decisions(
        a in any::<u64>(),
        b in any::<u64>(),
        c in any::<u64>(),
        f in any::<u32>(),
        s in any::<usize>(),
        t in any::<usize>(),
        i in any::<usize>(),
        v in any::<bool>(),
        w in any::<bool>(),
    ) {
        prop_assert_eq!(passthrough(f), f & 1 == 0);
        prop_assert_eq!(video_unset(a), a == 0);
        prop_assert_eq!(start_unset(a), a == 0);
        prop_assert_eq!(before_video(a, b), a < b);
        prop_assert_eq!(ends_before(a, b, c), a.wrapping_add(b) <= c);
        prop_assert_eq!(cut_ns(a, b), a.wrapping_sub(b));
        prop_assert_eq!(cut_ts(a, b), a.wrapping_add(b));
        prop_assert_eq!(cut_bytes(s, a), s.wrapping_mul(a as u32 as usize));
        prop_assert_eq!(cut_frames(f, a), f.wrapping_sub(a as u32));
        prop_assert_eq!(block_bytes(s), 1024usize.wrapping_mul(s));
        prop_assert_eq!(push_bytes(f, s), (f as usize).wrapping_mul(s));
        prop_assert_eq!(can_pop(s, t), s > t);
        prop_assert_eq!(block_ts(a, b), a.wrapping_add(b));
        prop_assert_eq!(paused_ts(a, b), a.wrapping_add(b));
        prop_assert_eq!(next_frames(s), s.wrapping_add(1024));
        let idx = i % 70;
        let want = idx < usize::BITS as usize && (s >> idx) & 1 != 0;
        prop_assert_eq!(mix_on(s, idx), want);
        prop_assert_eq!(interleave(v, w), v && w);
        prop_assert_eq!(delay_on(f), f != 0);
        prop_assert_eq!(delay_ns(f), u64::from(f) * NS);
    }

    /// The stand-in over packet sequences against origin/master's code.
    #[test]
    fn edge_proptest_stand_in(start in arb_start(), packets in arb_packets()) {
        let mut port = start.clone();
        let mut master = start;
        for (ts, frames) in packets {
            let a = port.callback(ts, frames);
            let b = master_callback(&mut master, ts, frames);
            prop_assert_eq!(a, b);
            prop_assert_eq!(&port, &master);
        }
    }
}

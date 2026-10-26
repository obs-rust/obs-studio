//! Tier 1: `obs_source::audio_place`, the source audio placement and action
//! timeline of `source_output_audio_place()`,
//! `source_output_audio_push_back()`, `source_output_audio_data()` and
//! `apply_audio_actions()`.
//!
//! The `steps_*` cases port `test/cmocka/test_source_audio_place_steps.c`
//! 1:1: its 13 cases under the same names, in the same order, with the same
//! values and expectations, the "characterized, not endorsed" ones and the
//! copies of the callers on a stand-in source included. The C calls the
//! hidden `obs_source_audio_place_*()` functions; here the safe core is
//! called directly.
//!
//! The `edge_*` cases are new: every decision over every pair (and the
//! sizes over every triple) of special values against models of the
//! original C expressions, wide ranges of monitoring types and mixer masks,
//! and proptests over full-range values and over packet and action
//! sequences through the stand-in, against a copy of the callers with the
//! C's expressions inline.

use obs_source::audio_place::{
    FRAMES, MAX_BUF_SIZE, MONITOR_ONLY, action_frame, action_stops, fill_end, fits, mix_on, offset,
    older, outputs, pop_back, size, started,
};
use proptest::prelude::*;

const MAX_BUF: usize = MAX_BUF_SIZE;
const SEC: u64 = 1_000_000_000;
// `enum obs_monitoring_type` (obs.h).
const MONITORING_NONE: i32 = 0;
const MONITORING_AND_OUTPUT: i32 = 2;
// MAX_AUDIO_MIXES (obs-internal.h).
const MIXES: usize = 6;

const RATES: [u32; 4] = [8000, 44100, 48000, 384_000];

/// Per rate: the last ns that is still frame 0, and the first that is frame
/// 1; the last ns that is frame 1023, and the first that is frame 1024.
const FRAME0_LAST: [u64; 4] = [124_999, 22675, 20833, 2604];
const FRAME1_FIRST: [u64; 4] = [125_000, 22676, 20834, 2605];
const FRAME1023_LAST: [u64; 4] = [127_999_999, 23_219_954, 21_333_333, 2_666_666];
const FRAME1024_FIRST: [u64; 4] = [128_000_000, 23_219_955, 21_333_334, 2_666_667];

const TS: [u64; 9] = [
    0,
    1,
    2,
    1000,
    SEC,
    i64::MAX as u64,
    i64::MAX as u64 + 1,
    u64::MAX - 1,
    u64::MAX,
];

/// `util_mul_div64(num, rate, 1e9)` for a rate up to 1e9, where every
/// host's formula is the exact quotient.
fn exact_frames(ns: u64, rate: u64) -> u64 {
    assert!(rate <= SEC);
    u64::try_from(u128::from(ns) * u128::from(rate) / u128::from(SEC)).unwrap()
}

// test_source_audio_place_steps.c, 1:1

#[test]
fn steps_constants() {
    // the header's mirrors of the core
    assert_eq!(FRAMES, 1024);
    assert_eq!(MAX_BUF_SIZE, 1000 * 1024 * size_of::<f32>());
    assert_eq!(MAX_BUF_SIZE, 4_096_000);
    assert_eq!(MONITOR_ONLY, 1);
}

#[test]
fn steps_started_and_older() {
    assert!(!started(0));
    assert!(started(1));
    assert!(started(u64::MAX));

    for a in TS {
        assert_eq!(started(a), a != 0, "{a}");
        for b in TS {
            assert_eq!(older(a, b), a < b, "{a} {b}");
        }
    }

    // the same time is not older; one ns before is
    assert!(!older(SEC, SEC));
    assert!(older(SEC - 1, SEC));
    assert!(!older(SEC + 1, SEC));
    // nothing is older than 0
    assert!(!older(0, 0));
}

#[test]
fn steps_size_bytes() {
    assert_eq!(size(0), 0);
    assert_eq!(size(1), 4);
    assert_eq!(size(480), 1920);
    assert_eq!(size(1024), 4096);
    // in size_t: the largest packet does not wrap
    assert_eq!(size(u32::MAX), u32::MAX as usize * 4);
}

#[test]
fn steps_offset_per_rate() {
    let base = 1000 * SEC;

    for (r, rate) in RATES.into_iter().enumerate() {
        // placed at 0
        assert_eq!(offset(base, base, rate), 0);
        // characterized, not endorsed: less than one sample after audio_ts
        // rounds down to 0, onto the samples already there
        assert_eq!(offset(base + FRAME0_LAST[r], base, rate), 0);
        // one frame, in bytes
        assert_eq!(offset(base + FRAME1_FIRST[r], base, rate), 4);
        // one second
        assert_eq!(offset(base + SEC, base, rate), rate as usize * 4);
        // the timestamp alone does not matter, only the difference
        assert_eq!(offset(SEC, 0, rate), rate as usize * 4);
        assert_eq!(offset(u64::MAX, u64::MAX - SEC, rate), rate as usize * 4);
    }

    // 21.312 s at 48 kHz: 1022976 frames, one block short of MAX_BUF_SIZE
    assert_eq!(offset(base + 21_312_000_000, base, 48000), MAX_BUF - 4096);

    // characterized, not endorsed: a timestamp before audio_ts wraps to a
    // placement far past MAX_BUF_SIZE (the place resets first, so it never
    // gets one)
    assert_eq!(offset(base - 1, base, 48000), 885_443_715_538_058 * 4);
    // a rate of 0 places everything at 0
    assert_eq!(offset(base + SEC, base, 0), 0);
}

#[test]
fn steps_fits_and_pop_back() {
    assert!(fits(0, 0));
    assert!(fits(0, 4096));
    assert!(fits(0, MAX_BUF));
    assert!(!fits(0, MAX_BUF + 1));
    // just under, at and just over MAX_BUF_SIZE
    assert!(fits(MAX_BUF - 4097, 4096));
    assert!(fits(MAX_BUF - 4096, 4096));
    assert!(!fits(MAX_BUF - 4095, 4096));
    assert!(fits(MAX_BUF, 0));
    assert!(!fits(MAX_BUF + 1, 0));
    // characterized, not endorsed: the sum wraps as size_t, so a start and
    // size whose sum passes SIZE_MAX fit (neither caller can reach it)
    assert!(fits(usize::MAX, 1));
    assert!(fits(usize::MAX - 9, 10 + MAX_BUF));
    assert!(!fits(usize::MAX, 0));

    // after deque_place() the channel holds at least placement + size: the
    // rest is popped off
    assert_eq!(pop_back(8192, 0, 4096), 4096);
    assert_eq!(pop_back(8192, 4096, 4096), 0);
    assert_eq!(pop_back(MAX_BUF, 4, 4096), MAX_BUF - 4100);
    // wrapping, as size_t
    assert_eq!(pop_back(0, 0, 1), usize::MAX);
    assert_eq!(pop_back(5, usize::MAX, 1), 5);
}

#[test]
fn steps_outputs_each_type() {
    assert!(outputs(MONITORING_NONE));
    assert!(!outputs(MONITOR_ONLY));
    assert!(outputs(MONITORING_AND_OUTPUT));

    // characterized, not endorsed: a value outside the enum outputs
    for t in -64..=64 {
        assert_eq!(outputs(t), t != 1, "{t}");
    }
    assert!(outputs(i32::MIN));
    assert!(outputs(i32::MAX));
}

#[test]
fn steps_action_frames() {
    let base = 7 * SEC;

    for (r, rate) in RATES.into_iter().enumerate() {
        let rate = rate as usize;

        assert_eq!(action_frame(base, base, rate), 0);
        assert_eq!(action_frame(base + FRAME0_LAST[r], base, rate), 0);
        assert_eq!(action_frame(base + FRAME1_FIRST[r], base, rate), 1);
        assert_eq!(
            action_frame(base + FRAME1023_LAST[r], base, rate),
            FRAMES - 1
        );
        assert_eq!(action_frame(base + FRAME1024_FIRST[r], base, rate), FRAMES);
        assert_eq!(action_frame(base + SEC, base, rate), rate);
    }

    // characterized, not endorsed: unclamped, a timestamp before audio_ts
    // wraps (apply_audio_actions() clamps first)
    assert_eq!(action_frame(base - 1, base, 48000), 885_443_715_538_058);

    // the stop
    assert!(!action_stops(0));
    assert!(!action_stops(FRAMES - 1));
    assert!(action_stops(FRAMES));
    assert!(action_stops(FRAMES + 1));
    assert!(action_stops(usize::MAX));
}

#[test]
fn steps_fill_ends() {
    // the previous volume fills up to the action's frame
    assert_eq!(fill_end(0, 0), 0);
    assert_eq!(fill_end(0, 1), 1);
    assert_eq!(fill_end(0, FRAMES - 1), FRAMES - 1);
    assert_eq!(fill_end(100, 200), 200);
    // an action at or before the filled frames fills nothing
    assert_eq!(fill_end(200, 200), 200);
    assert_eq!(fill_end(200, 100), 200);
    assert_eq!(fill_end(usize::MAX, 0), usize::MAX);
    assert_eq!(fill_end(0, usize::MAX), usize::MAX);
}

#[test]
fn steps_mixes_every_mask() {
    for mask in 0_u32..256 {
        for mix in 0..MIXES {
            assert_eq!(mix_on(mask, mix), (mask >> mix) & 1 != 0, "{mask} {mix}");
        }
    }

    // bits past the mixes do not matter for the mixes
    for mix in 0..MIXES {
        assert!(mix_on(u32::MAX, mix));
        assert!(!mix_on(!(1_u32 << mix), mix));
        assert!(!mix_on(0xffff_ffc0, mix));
    }
}

// the callers, copied onto a stand-in source

#[derive(Clone, Debug, PartialEq)]
struct Standin {
    audio_ts: u64,
    buf: [usize; 8],
    channels: usize,
    sample_rate: u32,
    monitoring_type: i32,
    trace: String,
}

impl Standin {
    fn new(rate: u32, audio_ts: u64) -> Self {
        Self {
            audio_ts,
            buf: [0; 8],
            channels: 2,
            sample_rate: rate,
            monitoring_type: MONITORING_NONE,
            trace: String::new(),
        }
    }

    /// `reset_audio_data()`
    fn reset(&mut self, ts: u64) {
        self.buf = [0; 8];
        self.audio_ts = ts;
        self.trace.push_str("reset ");
    }

    /// `source_output_audio_place()`
    fn place(&mut self, timestamp: u64, frames: u32) {
        let size = size(frames);

        if !started(self.audio_ts) || older(timestamp, self.audio_ts) {
            self.reset(timestamp);
        }

        let placement = offset(timestamp, self.audio_ts, self.sample_rate);

        if !fits(placement, size) {
            self.trace.push_str(&format!("drop({placement}+{size}) "));
            return;
        }

        for i in 0..self.channels {
            // deque_place() grows the channel to placement + size
            let end = placement.wrapping_add(size);
            if self.buf[i] < end {
                self.buf[i] = end;
            }
            let pop = pop_back(self.buf[i], placement, size);
            self.buf[i] -= pop;
        }
        self.trace.push_str(&format!("place({placement},{size}) "));
    }

    /// `source_output_audio_push_back()`
    fn push_back(&mut self, frames: u32) {
        let size = size(frames);

        if !fits(self.buf[0], size) {
            self.trace
                .push_str(&format!("drop({}+{size}) ", self.buf[0]));
            return;
        }

        for i in 0..self.channels {
            self.buf[i] += size;
        }
        self.trace.push_str(&format!("push({size}) "));
    }

    /// The last step of `source_output_audio_data()`.
    fn output(&mut self, push_back: bool, timestamp: u64, frames: u32) {
        if outputs(self.monitoring_type) {
            if push_back && started(self.audio_ts) {
                self.push_back(frames);
            } else {
                self.place(timestamp, frames);
            }
        } else {
            self.trace.push_str("skip ");
        }
    }
}

#[test]
fn steps_place_sequence() {
    let base = 1000 * SEC;

    for (r, rate) in RATES.into_iter().enumerate() {
        // no audio_ts: reset, placed at 0
        let mut s = Standin::new(rate, 0);
        s.place(base, 1024);
        assert_eq!(s.trace, "reset place(0,4096) ");
        assert_eq!(s.audio_ts, base);
        assert_eq!(s.buf[0], 4096);
        assert_eq!(s.buf[1], 4096);
        assert_eq!(s.buf[2], 0);

        // one frame later: overlaps all but the first frame, and the
        // channel is cut to its end
        s.trace.clear();
        s.place(base + FRAME1_FIRST[r], 1024);
        assert_eq!(s.trace, "place(4,4096) ");
        assert_eq!(s.buf[0], 4100);

        // less than a sample after audio_ts: on top of the start, and the
        // longer buffer is cut back
        s.trace.clear();
        s.place(base + FRAME0_LAST[r], 480);
        assert_eq!(s.trace, "place(0,1920) ");
        assert_eq!(s.buf[0], 1920);

        // an older packet: reset, placed at 0 at its own time
        s.trace.clear();
        s.place(base - 1, 1024);
        assert_eq!(s.trace, "reset place(0,4096) ");
        assert_eq!(s.audio_ts, base - 1);

        // one second later
        let mut s = Standin::new(rate, base);
        s.place(base + SEC, 1);
        assert_eq!(s.trace, format!("place({},4) ", rate as usize * 4));
        assert_eq!(s.buf[0], rate as usize * 4 + 4);
    }

    // just under, at and just over MAX_BUF_SIZE at 48 kHz: the block that
    // ends exactly at MAX_BUF_SIZE is placed, one frame later it is dropped
    let mut s = Standin::new(48000, base);
    s.place(base + 21_312_000_000 - 20833, 1024);
    assert_eq!(s.trace, "place(4091900,4096) ");
    s.trace.clear();
    s.place(base + 21_312_000_000, 1024);
    assert_eq!(s.trace, "place(4091904,4096) ");
    assert_eq!(s.buf[0], MAX_BUF);
    s.trace.clear();
    s.place(base + 21_312_000_000 + 20834, 1024);
    assert_eq!(s.trace, "drop(4091908+4096) ");
    assert_eq!(s.buf[0], MAX_BUF);

    // characterized, not endorsed: a packet larger than MAX_BUF_SIZE on its
    // own is dropped whole, even into an empty buffer
    let mut s = Standin::new(48000, 0);
    s.place(base, 1_024_001);
    assert_eq!(s.trace, "reset drop(0+4096004) ");
    assert_eq!(s.audio_ts, base);
    let mut s = Standin::new(48000, 0);
    s.place(base, 1_024_000);
    assert_eq!(s.trace, "reset place(0,4096000) ");

    // no channels: placed with nothing to place into
    let mut s = Standin::new(48000, base);
    s.channels = 0;
    s.place(base, 1024);
    assert_eq!(s.trace, "place(0,4096) ");
    assert_eq!(s.buf[0], 0);
}

#[test]
fn steps_push_back_sequence() {
    let mut s = Standin::new(48000, SEC);
    s.push_back(1024);
    s.push_back(480);
    assert_eq!(s.trace, "push(4096) push(1920) ");
    assert_eq!(s.buf[0], 6016);
    assert_eq!(s.buf[1], 6016);

    // just under, at and just over MAX_BUF_SIZE
    let mut s = Standin::new(48000, SEC);
    s.buf[0] = MAX_BUF - 4097;
    s.push_back(1024);
    assert_eq!(s.trace, "push(4096) ");
    s.trace.clear();
    s.buf[0] = MAX_BUF - 4096;
    s.push_back(1024);
    assert_eq!(s.trace, "push(4096) ");
    assert_eq!(s.buf[0], MAX_BUF);
    s.trace.clear();
    s.push_back(1);
    assert_eq!(s.trace, "drop(4096000+4) ");
    assert_eq!(s.buf[0], MAX_BUF);

    // characterized, not endorsed: only the first channel is checked, so a
    // longer second channel still grows
    let mut s = Standin::new(48000, SEC);
    s.buf[1] = MAX_BUF;
    s.push_back(1024);
    assert_eq!(s.trace, "push(4096) ");
    assert_eq!(s.buf[1], MAX_BUF + 4096);
}

#[test]
fn steps_output_each_monitoring_type() {
    let base = 50 * SEC;
    let types = [MONITORING_NONE, MONITOR_ONLY, MONITORING_AND_OUTPUT, 3, -1];
    let outputs_of = [true, false, true, true, true];

    for (t, &monitoring_type) in types.iter().enumerate() {
        for push_back in [false, true] {
            for has_ts in [false, true] {
                let mut s = Standin::new(48000, if has_ts { base } else { 0 });
                s.monitoring_type = monitoring_type;
                s.output(push_back, base + SEC, 1024);

                let expected = if !outputs_of[t] {
                    "skip "
                } else if push_back && has_ts {
                    "push(4096) "
                } else if has_ts {
                    "place(192000,4096) "
                } else {
                    "reset place(0,4096) "
                };
                assert_eq!(s.trace, expected, "{monitoring_type} {push_back} {has_ts}");
            }
        }
    }
}

/// `apply_audio_actions()`: each action's frame, the fill and the mixes;
/// the volume after an action is its index + 2 (as `get_source_volume()`
/// would return after `apply_audio_action()`), the volume before the
/// first 1. Returns how many actions applied, the volume array and the
/// mixes.
fn standin_actions(
    audio_ts: u64,
    rate: usize,
    actions: &[u64],
    mixers: u32,
) -> (usize, Vec<f32>, [bool; MIXES]) {
    let mut vol_data = vec![0.0_f32; FRAMES];
    let mut cur_vol = 1.0_f32;
    let mut frame_num = 0;
    let mut applied = 0;

    for (i, &action_ts) in actions.iter().enumerate() {
        let mut timestamp = action_ts;

        if older(timestamp, audio_ts) {
            timestamp = audio_ts;
        }

        let new_frame_num = action_frame(timestamp, audio_ts, rate);

        if action_stops(new_frame_num) {
            break;
        }

        applied += 1;

        let end = fill_end(frame_num, new_frame_num);
        while frame_num < end {
            vol_data[frame_num] = cur_vol;
            frame_num += 1;
        }

        cur_vol = (i + 2) as f32;
    }

    while frame_num < FRAMES {
        vol_data[frame_num] = cur_vol;
        frame_num += 1;
    }

    let mut mixes = [false; MIXES];
    for (mix, on) in mixes.iter_mut().enumerate() {
        *on = mix_on(mixers, mix);
    }

    (applied, vol_data, mixes)
}

/// The frames of `vol_data` at each volume, as "from-to:v" runs.
fn runs_of(vol_data: &[f32]) -> String {
    let mut out = String::new();
    let mut start = 0;
    for i in 1..=vol_data.len() {
        if i == vol_data.len() || vol_data[i] != vol_data[start] {
            out.push_str(&format!("{}-{}:{} ", start, i - 1, vol_data[start]));
            start = i;
        }
    }
    out
}

#[test]
fn steps_action_timeline() {
    let base = 20 * SEC;

    for (r, rate) in RATES.into_iter().enumerate() {
        let rate = rate as usize;

        // characterized, not endorsed: an action before audio_ts applies at
        // frame 0, like one at it; one after it at its frame; one at frame
        // 1023 is the last that applies this tick
        let a = [
            base - SEC,
            base,
            base + FRAME1_FIRST[r],
            base + FRAME1023_LAST[r],
        ];
        let (applied, vol, _) = standin_actions(base, rate, &a, 0x3f);
        assert_eq!(applied, 4);
        assert_eq!(runs_of(&vol), "0-0:3 1-1022:4 1023-1023:5 ");

        // an action at frame 1024 stays queued, and so does every one after
        // it, even one that is due
        let b = [base + FRAME0_LAST[r], base + FRAME1024_FIRST[r], base];
        let (applied, vol, _) = standin_actions(base, rate, &b, 0x3f);
        assert_eq!(applied, 1);
        assert_eq!(runs_of(&vol), "0-1023:2 ");

        // the previous volume fills up to the action's frame (100 at every
        // rate)
        let c = [base + FRAME1_FIRST[r] * 100];
        let (_, vol, _) = standin_actions(base, rate, &c, 0x3f);
        assert_eq!(runs_of(&vol), "0-99:1 100-1023:2 ");

        // no actions: the volume before them fills the tick
        let (_, vol, _) = standin_actions(base, rate, &[], 0x3f);
        assert_eq!(runs_of(&vol), "0-1023:1 ");
    }

    // characterized, not endorsed: actions out of order; a later action at
    // an earlier frame fills nothing, and its volume starts where the
    // filled frames end
    let d = [base + 21_333_333 / 2, base + 20834];
    let (_, vol, _) = standin_actions(base, 48000, &d, 0x3f);
    assert_eq!(runs_of(&vol), "0-510:1 511-1023:3 ");

    // the mixes multiplied are the mixer bits
    let (_, _, mixes) = standin_actions(base, 48000, &[], 0x15);
    assert_eq!(mixes, [true, false, true, false, true, false]);
}

// edge cases

/// Special timestamps: around 0, 2^32, 2^63 and u64::MAX, one second, and
/// one sample and one block at 48 kHz either side of a base.
const SPECIAL_TS: [u64; 26] = [
    0,
    1,
    2,
    20833,
    20834,
    21_333_333,
    21_333_334,
    SEC - 1,
    SEC,
    SEC + 1,
    (1 << 32) - 1,
    1 << 32,
    (1 << 32) + 1,
    1000 * SEC,
    1000 * SEC - 1,
    1000 * SEC + 20834,
    1000 * SEC + 21_333_334,
    (1 << 63) - 1,
    1 << 63,
    (1 << 63) + 1,
    u64::MAX - SEC,
    u64::MAX - 20834,
    u64::MAX - 2,
    u64::MAX - 1,
    u64::MAX,
    18_446_744_073,
];

/// Special sizes: around 0, a block, MAX_BUF_SIZE, 2^32 and usize::MAX.
const SPECIAL_SIZES: [usize; 15] = [
    0,
    1,
    4,
    4095,
    4096,
    4097,
    MAX_BUF - 4097,
    MAX_BUF - 4096,
    MAX_BUF - 1,
    MAX_BUF,
    MAX_BUF + 1,
    u32::MAX as usize,
    usize::MAX - MAX_BUF,
    usize::MAX - 1,
    usize::MAX,
];

/// Rates where every host's `util_mul_div64()` is exact.
const EXACT_RATES: [u32; 10] = [
    0,
    1,
    8000,
    22050,
    44100,
    48000,
    96000,
    192_000,
    384_000,
    1_000_000_000,
];

#[test]
fn edge_timestamp_pairs() {
    for a in SPECIAL_TS {
        assert_eq!(started(a), a != 0);
        for b in SPECIAL_TS {
            assert_eq!(older(a, b), a < b, "{a} {b}");
            let diff = a.wrapping_sub(b);
            for rate in EXACT_RATES {
                let frames = exact_frames(diff, u64::from(rate));
                assert_eq!(
                    offset(a, b, rate),
                    usize::try_from(frames).unwrap().wrapping_mul(4),
                    "offset {a} {b} {rate}"
                );
                assert_eq!(
                    action_frame(a, b, rate as usize),
                    usize::try_from(frames).unwrap(),
                    "frame {a} {b} {rate}"
                );
            }
        }
    }
}

#[test]
fn edge_size_triples() {
    for frames in [
        0,
        1,
        2,
        480,
        1023,
        1024,
        1025,
        1_024_000,
        1_024_001,
        u32::MAX - 1,
        u32::MAX,
    ] {
        assert_eq!(size(frames), (frames as usize).wrapping_mul(4), "{frames}");
    }
    for a in SPECIAL_SIZES {
        assert_eq!(action_stops(a), a >= FRAMES);
        for b in SPECIAL_SIZES {
            assert_eq!(fits(a, b), a.wrapping_add(b) <= MAX_BUF, "fits {a} {b}");
            assert_eq!(fill_end(a, b), if b > a { b } else { a }, "fill {a} {b}");
            for c in SPECIAL_SIZES {
                assert_eq!(
                    pop_back(a, b, c),
                    a.wrapping_sub(b.wrapping_add(c)),
                    "pop {a} {b} {c}"
                );
            }
        }
    }
    for frame in 0..=3 * FRAMES {
        assert_eq!(action_stops(frame), frame >= FRAMES, "{frame}");
    }
}

#[test]
fn edge_monitoring_types() {
    for t in -70_000..=70_000 {
        assert_eq!(outputs(t), t != MONITOR_ONLY, "{t}");
    }
    for t in [i32::MIN, i32::MIN + 1, i32::MAX - 1, i32::MAX] {
        assert!(outputs(t));
    }
}

#[test]
fn edge_mixer_masks() {
    for high in [0_u32, 0x0001_0000, 0x8000_0000, 0xffff_0000] {
        for low in 0_u32..=0xffff {
            let mask = high | low;
            for mix in 0..32 {
                assert_eq!(mix_on(mask, mix), (mask >> mix) & 1 != 0, "{mask:#x} {mix}");
            }
        }
    }
    // past the u32's width no mix is on (the C's int shift is undefined
    // there; libobs asks only 0 to 5)
    for mix in [32, 33, 63, 64, 1000, usize::MAX] {
        assert!(!mix_on(u32::MAX, mix), "{mix}");
    }
}

#[test]
fn edge_frame_boundaries_every_rate() {
    // for every common rate, the first ns of each frame from 0 to 1025
    for rate in [
        8000_u32, 11025, 16000, 22050, 32000, 44100, 48000, 88200, 96000, 192_000, 384_000,
    ] {
        let base = 3 * SEC;
        for frame in 0..=1025_u64 {
            // the smallest ns with ns * rate >= frame * 1e9
            let first = (frame * SEC).div_ceil(u64::from(rate));
            assert_eq!(
                action_frame(base + first, base, rate as usize),
                frame as usize
            );
            assert_eq!(offset(base + first, base, rate), frame as usize * 4);
            if first > 0 {
                assert_eq!(
                    action_frame(base + first - 1, base, rate as usize),
                    frame as usize - 1
                );
            }
        }
    }
}

/// The callers with origin/master's expressions inline, for the proptests
/// of the stand-in.
#[derive(Clone, Debug, PartialEq)]
struct Inline(Standin);

impl Inline {
    fn reset(&mut self, ts: u64) {
        self.0.buf = [0; 8];
        self.0.audio_ts = ts;
        self.0.trace.push_str("reset ");
    }

    fn place(&mut self, timestamp: u64, frames: u32) {
        let s = &mut self.0;
        let size = (frames as usize).wrapping_mul(size_of::<f32>());
        if s.audio_ts == 0 || timestamp < s.audio_ts {
            self.reset(timestamp);
        }
        let s = &mut self.0;
        let frames_off = exact_frames(timestamp.wrapping_sub(s.audio_ts), u64::from(s.sample_rate));
        let placement = (frames_off as usize).wrapping_mul(size_of::<f32>());
        if placement.wrapping_add(size) > MAX_BUF_SIZE {
            s.trace.push_str(&format!("drop({placement}+{size}) "));
            return;
        }
        for i in 0..s.channels {
            let end = placement.wrapping_add(size);
            if s.buf[i] < end {
                s.buf[i] = end;
            }
            let pop = s.buf[i].wrapping_sub(placement.wrapping_add(size));
            s.buf[i] -= pop;
        }
        s.trace.push_str(&format!("place({placement},{size}) "));
    }

    fn push_back(&mut self, frames: u32) {
        let s = &mut self.0;
        let size = (frames as usize).wrapping_mul(size_of::<f32>());
        if s.buf[0].wrapping_add(size) > MAX_BUF_SIZE {
            s.trace.push_str(&format!("drop({}+{size}) ", s.buf[0]));
            return;
        }
        for i in 0..s.channels {
            s.buf[i] += size;
        }
        s.trace.push_str(&format!("push({size}) "));
    }

    fn output(&mut self, push_back: bool, timestamp: u64, frames: u32) {
        if self.0.monitoring_type != MONITOR_ONLY {
            if push_back && self.0.audio_ts != 0 {
                self.push_back(frames);
            } else {
                self.place(timestamp, frames);
            }
        } else {
            self.0.trace.push_str("skip ");
        }
    }
}

fn inline_actions(
    audio_ts: u64,
    rate: usize,
    actions: &[u64],
    mixers: u32,
) -> (usize, Vec<f32>, [bool; MIXES]) {
    let mut vol_data = vec![0.0_f32; FRAMES];
    let mut cur_vol = 1.0_f32;
    let mut frame_num = 0;
    let mut applied = 0;
    for (i, &action_ts) in actions.iter().enumerate() {
        let mut timestamp = action_ts;
        if timestamp < audio_ts {
            timestamp = audio_ts;
        }
        let new_frame_num = exact_frames(timestamp - audio_ts, rate as u64) as usize;
        if new_frame_num >= FRAMES {
            break;
        }
        applied += 1;
        if new_frame_num > frame_num {
            while frame_num < new_frame_num {
                vol_data[frame_num] = cur_vol;
                frame_num += 1;
            }
        }
        cur_vol = (i + 2) as f32;
    }
    while frame_num < FRAMES {
        vol_data[frame_num] = cur_vol;
        frame_num += 1;
    }
    let mut mixes = [false; MIXES];
    for (mix, on) in mixes.iter_mut().enumerate() {
        *on = mixers & (1 << mix) != 0;
    }
    (applied, vol_data, mixes)
}

/// A timestamp near `base`: at it, a few ns, samples or blocks either side,
/// or anywhere.
fn near(base: u64) -> impl Strategy<Value = u64> {
    prop_oneof![
        Just(base),
        (0_u64..4).prop_map(move |d| base.wrapping_add(d)),
        (0_u64..4).prop_map(move |d| base.wrapping_sub(d)),
        (0_u64..2_000_000).prop_map(move |n| base.wrapping_add(n * 20833)),
        (0_u64..1100).prop_map(move |n| base.wrapping_add(n * 21_333_333)),
        (0_u64..30).prop_map(move |n| base.wrapping_sub(n * SEC)),
        any::<u64>(),
    ]
}

fn rate() -> impl Strategy<Value = u32> {
    prop_oneof![
        prop::sample::select(EXACT_RATES.to_vec()),
        0_u32..=1_000_000_000
    ]
}

fn frames() -> impl Strategy<Value = u32> {
    prop_oneof![
        prop::sample::select(vec![
            0_u32,
            1,
            480,
            1023,
            1024,
            1025,
            1_023_999,
            1_024_000,
            1_024_001,
            u32::MAX
        ]),
        any::<u32>(),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    #[test]
    fn edge_decisions_full_range(
        a in any::<u64>(),
        b in any::<u64>(),
        r in rate(),
        f in any::<u32>(),
        x in any::<usize>(),
        y in any::<usize>(),
        z in any::<usize>(),
        t in any::<i32>(),
        mask in any::<u32>(),
        mix in 0_usize..40,
    ) {
        prop_assert_eq!(started(a), a != 0);
        prop_assert_eq!(older(a, b), a < b);
        prop_assert_eq!(size(f), (f as usize).wrapping_mul(4));
        let frames = exact_frames(a.wrapping_sub(b), u64::from(r));
        prop_assert_eq!(offset(a, b, r), (frames as usize).wrapping_mul(4));
        prop_assert_eq!(action_frame(a, b, r as usize), frames as usize);
        prop_assert_eq!(fits(x, y), x.wrapping_add(y) <= MAX_BUF);
        prop_assert_eq!(pop_back(x, y, z), x.wrapping_sub(y.wrapping_add(z)));
        prop_assert_eq!(outputs(t), t != 1);
        prop_assert_eq!(action_stops(x), x >= FRAMES);
        prop_assert_eq!(fill_end(x, y), x.max(y));
        prop_assert_eq!(mix_on(mask, mix), mix < 32 && (mask >> mix) & 1 != 0);
    }

    #[test]
    fn edge_packet_sequences(
        r in rate(),
        start in prop_oneof![Just(0_u64), near(1000 * SEC)],
        channels in 0_usize..=8,
        packets in prop::collection::vec(
            (any::<bool>(), 0_i32..4, near(1000 * SEC), frames()),
            1..24,
        ),
    ) {
        let mut core = Standin::new(r, start);
        core.channels = channels;
        let mut inline = Inline(core.clone());
        for (push_back, monitoring_type, ts, frames) in packets {
            core.monitoring_type = monitoring_type;
            inline.0.monitoring_type = monitoring_type;
            core.output(push_back, ts, frames);
            inline.output(push_back, ts, frames);
            prop_assert_eq!(&core, &inline.0);
        }
    }

    #[test]
    fn edge_action_sequences(
        r in rate(),
        audio_ts in near(20 * SEC),
        actions in prop::collection::vec(near(20 * SEC), 0..12),
        mixers in any::<u32>(),
    ) {
        let core = standin_actions(audio_ts, r as usize, &actions, mixers);
        let inline = inline_actions(audio_ts, r as usize, &actions, mixers);
        prop_assert_eq!(core.0, inline.0);
        prop_assert_eq!(runs_of(&core.1), runs_of(&inline.1));
        prop_assert_eq!(core.2, inline.2);
    }
}

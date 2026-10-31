//! Tier 1: the safe core of `libobs/media-io/audio-resampler-drift.c.h`.
//!
//! The `steps_*` tests port `test/cmocka/test_audio_resampler_drift_steps.c`
//! one to one: the same cases under the same names, in the same order, with
//! the same values, the characterized-not-endorsed ones included. The doubles
//! are compared bit for bit, as the C test does.
//!
//! The `edge_*` tests and the proptests check the core against [`model`], the
//! origin/master C written out expression by expression, over the edges of
//! each input: the dead band, the rounding of the delta, the clamp, a zero
//! input rate, and the integer ends of the padding and the counters.

use obs_media_io::audio_io::MAX_AUDIO_CHANNELS;
use obs_media_io::audio_resampler_drift::{
    Action, C1, C2, Compensation, DEAD_BAND_NS, DISTANCE, GAIN, MAX_PPM, count,
    delta as core_delta, fmt, input_rate, output_frames, reports_drift, sample_format, set_error,
    step as core_step, upmix_row,
};
use proptest::prelude::*;

const MS: i64 = 1_000_000;

/// A step as `audio_resampler_resample()` asks it: the action, then (after
/// the engage log there) the delta, unless idle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Idle,
    Engage { delta: i32 },
    Set { delta: i32 },
}

impl Step {
    fn to_c(self) -> u32 {
        match self {
            Self::Idle => Action::Idle.to_c(),
            Self::Engage { .. } => Action::Engage.to_c(),
            Self::Set { .. } => Action::Set.to_c(),
        }
    }

    fn delta(self) -> Option<i32> {
        match self {
            Self::Idle => None,
            Self::Engage { delta } | Self::Set { delta } => Some(delta),
        }
    }
}

fn step(c: &mut Compensation, input_freq: u32, in_frames: u32) -> Step {
    match core_step(c, input_freq, in_frames) {
        Action::Idle => Step::Idle,
        Action::Engage => Step::Engage {
            delta: core_delta(c, input_freq),
        },
        Action::Set => Step::Set {
            delta: core_delta(c, input_freq),
        },
    }
}

/// The drift report as `audio_resampler_destroy()` asks it: the rate only
/// when the gate is open.
fn drift_report(c: &Compensation, output_freq: u32) -> Option<f64> {
    reports_drift(c).then(|| input_rate(c, output_freq))
}

/// The C test's FNV-1a hash of every delta, over the delta's `uint32_t`
/// bits.
fn fnv(hash: u64, delta: i32) -> u64 {
    (hash ^ u64::from(delta.cast_unsigned())).wrapping_mul(1_099_511_628_211)
}

const FNV_BASIS: u64 = 1_469_598_103_934_665_603;

/// The C test's `DELTA_UNSET`: the delta a step leaves alone when idle.
const DELTA_UNSET: i32 = 123_456;

/// The action's C value and the delta beside it, as the C test sees them.
fn seen(action: Step) -> (u32, i32) {
    (action.to_c(), action.delta().unwrap_or(DELTA_UNSET))
}

fn bits(value: f64) -> u64 {
    value.to_bits()
}

/// Bit-exact equality, except that any NaN equals any NaN: Rust does not
/// fix a NaN's sign or payload, and only a zero input rate (which
/// `swr_init()` rejects before any step) makes one.
#[track_caller]
fn same_bits(got: f64, want: f64, what: &str) {
    if got.is_nan() && want.is_nan() {
        return;
    }
    assert_eq!(bits(got), bits(want), "{what}: {got:e}, expected {want:e}");
}

/// The negative run's state from the positive run's bits: the same value
/// with the sign flipped, but a 0 stays +0 as the C computes it.
fn mirrored(bits: u64, sign: f64) -> f64 {
    let value = f64::from_bits(bits);
    if value == 0.0 { value } else { sign * value }
}

/// The origin/master C, expression by expression.
mod model {
    use super::*;

    pub fn set_error(c: &mut Compensation, error_ns: i64) {
        if !c.enabled {
            c.enabled = true;
            c.vc1 = 0.0;
            c.vc2 = 0.0;
        }
        c.error_ns = if error_ns > DEAD_BAND_NS {
            error_ns - DEAD_BAND_NS
        } else if error_ns < -DEAD_BAND_NS {
            error_ns + DEAD_BAND_NS
        } else {
            0
        };
    }

    fn clamp_drift(drift: f64, max: f64) -> f64 {
        if drift > max {
            max
        } else if drift < -max {
            -max
        } else {
            drift
        }
    }

    /// `(int)lround(x)` for |x| well within `int`, NaN as glibc's
    /// `lround()` truncated to `int` (0).
    fn lround_int(x: f64) -> i32 {
        if x.is_nan() {
            return 0;
        }
        let r = x.round();
        r as i32
    }

    pub fn step(c: &mut Compensation, input_freq: u32, in_frames: u32) -> Step {
        let fs = f64::from(input_freq);
        let max = MAX_PPM * 1e-6 * fs;
        let icp = GAIN * (c.error_ns as f64) * 1e-9;
        let ic2 = (c.vc1 - c.vc2) / fs;
        let ic1 = icp - ic2;
        c.vc1 = clamp_drift(c.vc1 + ic1 / C1 * f64::from(in_frames), max);
        c.vc2 = clamp_drift(c.vc2 + ic2 / C2 * f64::from(in_frames), max);
        let mut engage = false;
        if !c.engaged {
            if c.error_ns == 0 {
                return Step::Idle;
            }
            c.engaged = true;
            engage = true;
        }
        let delta = -lround_int(c.vc1 * f64::from(DISTANCE) / fs);
        if engage {
            Step::Engage { delta }
        } else {
            Step::Set { delta }
        }
    }

    pub fn output_frames(estimated: i32, engaged: bool) -> i32 {
        if engaged {
            estimated.wrapping_add(estimated / 512 + 2)
        } else {
            estimated
        }
    }

    pub fn count(c: &mut Compensation, in_frames: u32, converted: i32) {
        if c.engaged {
            c.input_samples = c.input_samples.wrapping_add(u64::from(in_frames));
            c.output_samples = c.output_samples.wrapping_add(converted as i64 as u64);
        }
    }

    pub fn drift_report(c: &Compensation, output_freq: u32) -> Option<f64> {
        if c.engaged && c.output_samples != 0 {
            Some(c.input_samples as f64 / c.output_samples as f64 * f64::from(output_freq))
        } else {
            None
        }
    }
}

/// Bit-exact state equality (NaN states compare by bits).
#[track_caller]
fn same_state(got: &Compensation, want: &Compensation) {
    assert_eq!(
        (
            got.enabled,
            got.engaged,
            got.error_ns,
            got.input_samples,
            got.output_samples
        ),
        (
            want.enabled,
            want.engaged,
            want.error_ns,
            want.input_samples,
            want.output_samples
        ),
    );
    same_bits(got.vc1, want.vc1, "vc1");
    same_bits(got.vc2, want.vc2, "vc2");
}

#[test]
fn steps_constants() {
    assert_eq!(DEAD_BAND_NS, 10_000_000);
    same_bits(MAX_PPM, 1000.0, "MAX_PPM");
    assert_eq!(DISTANCE, 16_777_216);
    same_bits(GAIN, 3.756e-2, "GAIN");
    same_bits(C1, 8.25, "C1");
    same_bits(C2, 107.1, "C2");
    assert_eq!(Action::Idle.to_c(), 0);
    assert_eq!(Action::Engage.to_c(), 1);
    assert_eq!(Action::Set.to_c(), 2);
    assert_eq!(MAX_AUDIO_CHANNELS, 8);
}

#[test]
fn steps_sample_formats() {
    assert_eq!(sample_format(0), 1); // AUDIO_FORMAT_UNKNOWN
    assert_eq!(sample_format(1), 0); // AUDIO_FORMAT_U8BIT
    assert_eq!(sample_format(2), 1); // AUDIO_FORMAT_16BIT
    assert_eq!(sample_format(3), 2); // AUDIO_FORMAT_32BIT
    assert_eq!(sample_format(4), 3); // AUDIO_FORMAT_FLOAT
    assert_eq!(sample_format(5), 5); // AUDIO_FORMAT_U8BIT_PLANAR
    assert_eq!(sample_format(6), 6); // AUDIO_FORMAT_16BIT_PLANAR
    assert_eq!(sample_format(7), 7); // AUDIO_FORMAT_32BIT_PLANAR
    assert_eq!(sample_format(8), 8); // AUDIO_FORMAT_FLOAT_PLANAR

    // characterized, not endorsed: one past the last format is S16
    assert_eq!(sample_format(9), 1);
    assert_eq!(sample_format(100), 1);

    assert_eq!(fmt::U8, 0);
    assert_eq!(fmt::S16, 1);
    assert_eq!(fmt::S32, 2);
    assert_eq!(fmt::FLT, 3);
    assert_eq!(fmt::U8P, 5);
    assert_eq!(fmt::S16P, 6);
    assert_eq!(fmt::S32P, 7);
    assert_eq!(fmt::FLTP, 8);
}

#[test]
fn steps_upmix_rows() {
    let want: [[f64; 8]; 8] = [
        [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        [1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        [1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        [1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0],
        [1.0, 1.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0],
        [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0],
        [1.0, 1.0, 1.0, 0.0, 1.0, 1.0, 1.0, 0.0],
        [1.0, 1.0, 1.0, 0.0, 1.0, 1.0, 1.0, 1.0],
    ];
    for (ch, want_row) in (1u32..=8).zip(want) {
        let row = upmix_row(ch).expect("a row for 1 to 8 channels");
        for (got, want) in row.into_iter().zip(want_row) {
            same_bits(got, want, "upmix row");
        }
    }

    // no row for 0 channels or more than 8
    for ch in [0, 9, 64, u32::MAX] {
        assert_eq!(upmix_row(ch), None, "{ch} channels");
    }
}

#[test]
fn steps_first_error_enables() {
    let mut c = Compensation {
        vc1: 5.0,
        vc2: -3.0,
        input_samples: 7,
        output_samples: 9,
        ..Compensation::default()
    };

    set_error(&mut c, 0);
    assert!(c.enabled);
    assert!(!c.engaged);
    same_bits(c.vc1, 0.0, "vc1 cleared");
    same_bits(c.vc2, 0.0, "vc2 cleared");
    assert_eq!(c.input_samples, 7);
    assert_eq!(c.output_samples, 9);

    c.vc1 = 5.0;
    c.vc2 = -3.0;
    set_error(&mut c, 20 * MS);
    same_bits(c.vc1, 5.0, "vc1 kept");
    same_bits(c.vc2, -3.0, "vc2 kept");
    assert_eq!(c.error_ns, 10 * MS);
}

#[test]
fn steps_dead_band() {
    let band = DEAD_BAND_NS;
    let cases = [
        (0, 0),
        (1, 0),
        (-1, 0),
        (band - 1, 0),
        (-(band - 1), 0),
        (band, 0),
        (-band, 0),
        (band + 1, 1),
        (-(band + 1), -1),
        (50 * MS, 40 * MS),
        (-50 * MS, -40 * MS),
        (i64::MAX, i64::MAX - band),
        (i64::MIN, i64::MIN + band),
    ];
    for (error, want) in cases {
        let mut c = Compensation {
            error_ns: 777,
            ..Compensation::default()
        };
        set_error(&mut c, error);
        assert_eq!(c.error_ns, want, "error {error}");
    }
}

#[test]
fn steps_zero_error_never_engages() {
    let mut c = Compensation::default();
    set_error(&mut c, 5 * MS);
    for i in 0..10_000 {
        assert_eq!(step(&mut c, 48_000, 480), Step::Idle, "step {i}");
    }
    assert!(!c.engaged);
    same_bits(c.vc1, 0.0, "vc1");
    same_bits(c.vc2, 0.0, "vc2");
}

/// A checkpoint of the constant-error runs: the step, its delta and the
/// bits of both states, for the positive error (the negative run is the
/// mirror image).
struct Checkpoint {
    at: usize,
    delta: i32,
    vc1: u64,
    vc2: u64,
}

const POSITIVE_RUN: [Checkpoint; 11] = [
    Checkpoint {
        at: 0,
        delta: -31,
        vc1: 0x3fb6_60a8_1b15_90db,
        vc2: 0,
    },
    Checkpoint {
        at: 1,
        delta: -61,
        vc1: 0x3fc6_5d2f_4bc9_1025,
        vc2: 0x3ee1_1dce_a655_3d84,
    },
    Checkpoint {
        at: 2,
        delta: -92,
        vc1: 0x3fd0_c349_71e3_3e22,
        vc2: 0x3ef9_a9d9_c2fa_c973,
    },
    Checkpoint {
        at: 3,
        delta: -122,
        vc1: 0x3fd6_563f_ff15_b5d6,
        vc2: 0x3f09_a6fe_06d0_9508,
    },
    Checkpoint {
        at: 10,
        delta: -334,
        vc1: 0x3fee_9559_c012_e8e0,
        vc2: 0x3f3d_4dcf_e994_1a86,
    },
    Checkpoint {
        at: 100,
        delta: -2907,
        vc1: 0x4020_a1bf_2923_3af3,
        vc2: 0x3fa4_38e4_a3cc_7684,
    },
    Checkpoint {
        at: 500,
        delta: -11531,
        vc1: 0x4040_7ed6_8737_3662,
        vc2: 0x3fea_a0e1_2d3e_c92e,
    },
    Checkpoint {
        at: 1000,
        delta: -16777,
        vc1: 0x4048_0000_0000_0000,
        vc2: 0x4005_f393_37ed_4d10,
    },
    Checkpoint {
        at: 2000,
        delta: -16777,
        vc1: 0x4048_0000_0000_0000,
        vc2: 0x401b_1d1c_6a0b_7072,
    },
    Checkpoint {
        at: 5000,
        delta: -16777,
        vc1: 0x4048_0000_0000_0000,
        vc2: 0x4030_d96c_056b_1acd,
    },
    Checkpoint {
        at: 9999,
        delta: -16777,
        vc1: 0x4048_0000_0000_0000,
        vc2: 0x403c_77ce_15cb_794f,
    },
];

/// 10,000 steps of 480 frames at 48 kHz with a constant error: engaged on
/// the first step, clamped at 1000 ppm (48 samples per second) at step 880.
/// `sign` mirrors the positive run's checkpoints.
fn constant_error_run(error_ns: i64, sign: f64, want_sum: i64, want_hash: u64) {
    let mut c = Compensation::default();
    set_error(&mut c, error_ns);

    let mut hash = FNV_BASIS;
    let mut sum = 0i64;
    let mut clamp_at = None;
    let mut next = POSITIVE_RUN.iter().peekable();
    for i in 0..10_000 {
        let action = step(&mut c, 48_000, 480);
        let want_action = if i == 0 { 1 } else { 2 };
        assert_eq!(action.to_c(), want_action, "step {i}");
        let delta = action.delta().unwrap_or(DELTA_UNSET);
        sum += i64::from(delta);
        hash = fnv(hash, delta);
        if clamp_at.is_none() && (c.vc1 == 48.0 || c.vc1 == -48.0) {
            clamp_at = Some(i);
        }
        if let Some(point) = next.next_if(|point| point.at == i) {
            let want_delta = if sign < 0.0 {
                -point.delta
            } else {
                point.delta
            };
            assert_eq!(delta, want_delta, "delta at {i}");
            same_bits(c.vc1, mirrored(point.vc1, sign), "vc1");
            same_bits(c.vc2, mirrored(point.vc2, sign), "vc2");
        }
    }
    assert!(c.engaged);
    assert!(next.next().is_none());
    assert_eq!(clamp_at, Some(880));
    assert_eq!(sum, want_sum);
    assert_eq!(hash, want_hash);
}

#[test]
fn steps_constant_positive_error() {
    constant_error_run(50 * MS, 1.0, -161_617_939, 0x96bf_c7d0_d527_cf2a);
}

#[test]
fn steps_constant_negative_error() {
    constant_error_run(-50 * MS, -1.0, 161_617_939, 0xa7e0_e03c_5ad7_4c7c);
}

#[test]
fn steps_larger_steps() {
    let mut c = Compensation::default();
    set_error(&mut c, 50 * MS);
    let mut clamp_at = None;
    let mut hash = FNV_BASIS;
    for i in 0..10_000 {
        let action = step(&mut c, 48_000, 1024);
        hash = fnv(hash, action.delta().unwrap_or(DELTA_UNSET));
        if i == 0 {
            assert_eq!(action, Step::Engage { delta: -65 });
        }
        if i == 100 {
            assert_eq!(action, Step::Set { delta: -5805 });
            same_bits(c.vc1, f64::from_bits(0x4030_9bce_35ad_5489), "vc1");
            same_bits(c.vc2, f64::from_bits(0x3fc5_f255_28e9_0579), "vc2");
        }
        if clamp_at.is_none() && c.vc1 == 48.0 {
            clamp_at = Some(i);
        }
    }
    assert_eq!(clamp_at, Some(412));
    same_bits(c.vc2, f64::from_bits(0x4044_9c5f_876d_8e60), "vc2");
    assert_eq!(hash, 0x80b1_2b66_47e3_dd07);
}

#[test]
fn steps_tiny_error() {
    let mut c = Compensation::default();
    set_error(&mut c, DEAD_BAND_NS + 1);
    for i in 0..10_000 {
        let want = if i == 0 {
            Step::Engage { delta: 0 }
        } else {
            Step::Set { delta: 0 }
        };
        assert_eq!(step(&mut c, 44_100, 441), want, "step {i}");
    }
    same_bits(c.vc1, f64::from_bits(0x3ec8_0630_02b9_7c4b), "vc1");
    same_bits(c.vc2, f64::from_bits(0x3eb6_3f09_bac9_c1de), "vc2");
}

#[test]
fn steps_engages_late() {
    let mut c = Compensation::default();
    set_error(&mut c, 5 * MS);
    for i in 0..100 {
        assert_eq!(
            seen(step(&mut c, 48_000, 1024)),
            (0, DELTA_UNSET),
            "step {i}"
        );
    }
    set_error(&mut c, 30 * MS);
    assert_eq!(step(&mut c, 48_000, 1024), Step::Engage { delta: -33 });
    same_bits(c.vc1, f64::from_bits(0x3fb7_de91_2df4_dec7), "vc1");
    assert_eq!(step(&mut c, 48_000, 1024), Step::Set { delta: -65 });
    same_bits(c.vc2, f64::from_bits(0x3ef3_7985_dbf8_4f16), "vc2");
    let mut last = Step::Idle;
    for _ in 102..300 {
        last = step(&mut c, 48_000, 1024);
    }
    assert_eq!(last, Step::Set { delta: -5111 });
    same_bits(c.vc1, f64::from_bits(0x402d_3eff_6fec_e1fb), "vc1");
    same_bits(c.vc2, f64::from_bits(0x3fd3_d7c0_afc0_d9da), "vc2");
}

#[test]
fn steps_stays_engaged() {
    let mut c = Compensation::default();
    set_error(&mut c, 50 * MS);
    let mut hash = FNV_BASIS;
    for i in 0..3000 {
        if i == 200 {
            set_error(&mut c, 0);
        }
        let action = step(&mut c, 48_000, 480);
        hash = fnv(hash, action.delta().unwrap_or(DELTA_UNSET));
        if i > 0 {
            assert_eq!(action.to_c(), 2, "step {i}");
        }
        match i {
            199 => assert_eq!(action, Step::Set { delta: -5433 }),
            200 => assert_eq!(action, Step::Set { delta: -5427 }),
            1000 => {
                assert_eq!(action, Step::Set { delta: -2192 });
                same_bits(c.vc1, f64::from_bits(0x4019_1510_ee1d_fb19), "vc1");
                same_bits(c.vc2, f64::from_bits(0x3feb_a31e_0cfb_7284), "vc2");
            }
            2999 => assert_eq!(action, Step::Set { delta: -566 }),
            _ => {}
        }
    }
    assert!(c.engaged);
    assert_eq!(c.error_ns, 0);
    assert_eq!(hash, 0x2c65_bfa8_50c5_9e6b);
}

#[test]
fn steps_zero_frames() {
    let mut c = Compensation::default();
    set_error(&mut c, 50 * MS);
    assert_eq!(step(&mut c, 48_000, 0), Step::Engage { delta: 0 });
    same_bits(c.vc1, 0.0, "vc1");
    same_bits(c.vc2, 0.0, "vc2");
}

#[test]
fn steps_output_frames() {
    let cases = [
        (0, 2),
        (1, 3),
        (511, 513),
        (512, 515),
        (1023, 1026),
        (1024, 1028),
        (48_000, 48_095),
        (1 << 20, (1 << 20) + 2050),
    ];
    for (estimated, padded) in cases {
        assert_eq!(output_frames(estimated, false), estimated);
        assert_eq!(output_frames(estimated, true), padded);
    }
    // characterized, not endorsed: a negative estimate is padded toward 0
    assert_eq!(output_frames(-1024, true), -1024);
    assert_eq!(output_frames(-1, true), 1);
}

#[test]
fn steps_counters() {
    let mut c = Compensation {
        enabled: true,
        ..Compensation::default()
    };
    count(&mut c, 480, 470);
    assert_eq!((c.input_samples, c.output_samples), (0, 0));

    c.engaged = true;
    count(&mut c, 480, 470);
    count(&mut c, 1024, 0);
    assert_eq!((c.input_samples, c.output_samples), (1504, 470));

    c.input_samples = u64::MAX;
    count(&mut c, 2, 1);
    assert_eq!((c.input_samples, c.output_samples), (1, 471));
}

#[test]
fn steps_drift_report() {
    let mut c = Compensation {
        input_samples: 48_000,
        output_samples: 48_000,
        ..Compensation::default()
    };
    assert_eq!(drift_report(&c, 48_000), None);

    c.engaged = true;
    c.output_samples = 0;
    assert_eq!(drift_report(&c, 48_000), None);

    c.output_samples = 48_000;
    same_bits(
        drift_report(&c, 48_000).expect("a report"),
        48_000.0,
        "same rate",
    );

    c.input_samples = 48_048;
    let rate = drift_report(&c, 48_000).expect("a report");
    // divided first, so a hair under 48048
    same_bits(rate, f64::from_bits(0x40e7_75ff_ffff_ffff), "fast device");

    c.input_samples = 0;
    same_bits(drift_report(&c, 44_100).expect("a report"), 0.0, "no input");

    c.input_samples = 44_101;
    c.output_samples = 48_000;
    same_bits(
        drift_report(&c, 48_000).expect("a report"),
        44_101.0 / 48_000.0 * 48_000.0,
        "rate change",
    );
}

/// Errors around the band, the ends and the values the loop sees.
const EDGE_ERRORS: [i64; 21] = [
    0,
    1,
    -1,
    DEAD_BAND_NS - 1,
    -(DEAD_BAND_NS - 1),
    DEAD_BAND_NS,
    -DEAD_BAND_NS,
    DEAD_BAND_NS + 1,
    -(DEAD_BAND_NS + 1),
    20 * MS,
    -20 * MS,
    50 * MS,
    -50 * MS,
    1_000 * MS,
    -1_000 * MS,
    10_000 * MS,
    -10_000 * MS,
    i64::MAX,
    i64::MIN,
    i64::MAX - DEAD_BAND_NS,
    i64::MIN + DEAD_BAND_NS,
];

#[test]
fn edge_dead_band_matches_c() {
    for enabled in [false, true] {
        for error in EDGE_ERRORS {
            let start = Compensation {
                enabled,
                vc1: 1.5,
                vc2: -0.5,
                ..Compensation::default()
            };
            let (mut got, mut want) = (start, start);
            set_error(&mut got, error);
            model::set_error(&mut want, error);
            same_state(&got, &want);
        }
    }
}

#[test]
fn edge_steps_match_c() {
    let rates = [
        1,
        7,
        8000,
        44_100,
        48_000,
        96_000,
        192_000,
        16_777_216,
        u32::MAX,
    ];
    let frames = [0, 1, 480, 1024, 4096, 48_000, u32::MAX];
    for error in EDGE_ERRORS {
        for rate in rates {
            for in_frames in frames {
                for engaged in [false, true] {
                    let start = Compensation {
                        enabled: true,
                        engaged,
                        ..Compensation::default()
                    };
                    let (mut got, mut want) = (start, start);
                    set_error(&mut got, error);
                    model::set_error(&mut want, error);
                    for i in 0..8 {
                        let a = step(&mut got, rate, in_frames);
                        let b = model::step(&mut want, rate, in_frames);
                        assert_eq!(
                            a, b,
                            "error {error} rate {rate} frames {in_frames} step {i}"
                        );
                        same_state(&got, &want);
                    }
                }
            }
        }
    }
}

/// `lround()` rounds half away from zero: at 2^24 Hz the delta is the
/// correction itself, so 2.5 samples per second is a delta of -3, not -2.
#[test]
fn edge_delta_rounds_half_away_from_zero() {
    for (vc1, delta) in [
        (2.5, -3),
        (-2.5, 3),
        (0.5, -1),
        (-0.5, 1),
        (1.5, -2),
        (2.4999, -2),
    ] {
        let mut c = Compensation {
            enabled: true,
            engaged: true,
            vc1,
            vc2: vc1,
            ..Compensation::default()
        };
        assert_eq!(
            step(&mut c, 16_777_216, 0),
            Step::Set { delta },
            "vc1 {vc1}"
        );
    }
}

/// A zero input rate, which `swr_init()` rejects before any step: the states
/// turn NaN and the delta is 0, as glibc's `lround()` truncated to `int`.
#[test]
fn edge_zero_input_rate() {
    let mut c = Compensation::default();
    set_error(&mut c, 50 * MS);
    let mut want = c;
    assert_eq!(step(&mut c, 0, 480), Step::Engage { delta: 0 });
    assert_eq!(model::step(&mut want, 0, 480), Step::Engage { delta: 0 });
    assert!(c.vc1.is_nan() && c.vc2.is_nan());
    same_state(&c, &want);
    assert_eq!(step(&mut c, 0, 480), Step::Set { delta: 0 });
}

#[test]
fn edge_output_frames_ends() {
    for estimated in [
        i32::MIN,
        i32::MIN + 1,
        -513,
        -512,
        -511,
        -2,
        -1,
        0,
        1,
        510,
        511,
        512,
        513,
        i32::MAX - 2,
        i32::MAX,
    ] {
        for engaged in [false, true] {
            assert_eq!(
                output_frames(estimated, engaged),
                model::output_frames(estimated, engaged),
                "{estimated} {engaged}"
            );
        }
    }
    // the C's int overflow, wrapped
    assert_eq!(output_frames(i32::MAX, true), i32::MIN + 4_194_304);
}

#[test]
fn edge_counters_wrap() {
    let mut c = Compensation {
        engaged: true,
        input_samples: u64::MAX - 1,
        output_samples: u64::MAX,
        ..Compensation::default()
    };
    count(&mut c, u32::MAX, i32::MAX);
    assert_eq!(c.input_samples, u64::from(u32::MAX) - 2);
    assert_eq!(c.output_samples, u64::from(i32::MAX.cast_unsigned()) - 1);
    // a negative count, which the C never passes, is cast as the C casts it
    c.output_samples = 0;
    count(&mut c, 0, -1);
    assert_eq!(c.output_samples, u64::MAX);
}

#[test]
fn edge_drift_report_matches_c() {
    let counts = [
        0,
        1,
        47_999,
        48_000,
        48_001,
        1 << 53,
        (1 << 53) + 1,
        u64::MAX,
    ];
    for engaged in [false, true] {
        for input_samples in counts {
            for output_samples in counts {
                for rate in [0, 1, 44_100, 48_000, u32::MAX] {
                    let c = Compensation {
                        engaged,
                        input_samples,
                        output_samples,
                        ..Compensation::default()
                    };
                    assert_eq!(
                        drift_report(&c, rate).map(f64::to_bits),
                        model::drift_report(&c, rate).map(f64::to_bits)
                    );
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn prop_dead_band(error in any::<i64>(), enabled in any::<bool>()) {
        let start = Compensation { enabled, vc1: 3.0, vc2: 4.0, ..Compensation::default() };
        let (mut got, mut want) = (start, start);
        set_error(&mut got, error);
        model::set_error(&mut want, error);
        prop_assert_eq!(got.error_ns, want.error_ns);
        prop_assert_eq!(bits(got.vc1), bits(want.vc1));
        prop_assert_eq!(bits(got.vc2), bits(want.vc2));
    }

    #[test]
    fn prop_loop_matches_c(
        rate in 8_000u32..=192_000,
        errors in prop::collection::vec((-10_000 * MS..=10_000 * MS, 1u32..=4096, 1usize..=64), 1..=16),
    ) {
        let mut got = Compensation::default();
        let mut want = got;
        for (error, in_frames, steps) in errors {
            set_error(&mut got, error);
            model::set_error(&mut want, error);
            for _ in 0..steps {
                prop_assert_eq!(step(&mut got, rate, in_frames), model::step(&mut want, rate, in_frames));
                prop_assert_eq!(bits(got.vc1), bits(want.vc1));
                prop_assert_eq!(bits(got.vc2), bits(want.vc2));
                prop_assert_eq!(got.engaged, want.engaged);
            }
        }
    }

    #[test]
    fn prop_output_frames(estimated in 0i32..=(1 << 20), engaged in any::<bool>()) {
        prop_assert_eq!(output_frames(estimated, engaged), model::output_frames(estimated, engaged));
    }

    #[test]
    fn prop_counters(
        start in any::<(u64, u64)>(),
        engaged in any::<bool>(),
        in_frames in any::<u32>(),
        converted in 0i32..=i32::MAX,
    ) {
        let c = Compensation { engaged, input_samples: start.0, output_samples: start.1, ..Compensation::default() };
        let (mut got, mut want) = (c, c);
        count(&mut got, in_frames, converted);
        model::count(&mut want, in_frames, converted);
        prop_assert_eq!((got.input_samples, got.output_samples), (want.input_samples, want.output_samples));
    }
}

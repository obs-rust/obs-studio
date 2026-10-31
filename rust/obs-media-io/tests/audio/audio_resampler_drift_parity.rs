//! Tier 3: `libobs/media-io/audio-resampler-drift.c.h` against its port.
//!
//! The oracle (`rust/obs-c-oracle/oracle/audio_resampler_drift.c`) is the
//! unchanged C file with its nine functions renamed `oracle_*`, built with
//! `-ffp-contract=off` as libobs builds the file. Each case runs the oracle,
//! the C ABI shims (`ffi::audio_resampler_drift`, what the hybrid libobs
//! links) and the safe core on the same inputs and compares everything they
//! return and write, the doubles bit for bit:
//!
//! - the constants and the layout of `struct audio_resampler_comp`;
//! - every sample format from -2 to 20 and the ends of `int`;
//! - the upmix row for 0 to 9 channels and `UINT32_MAX`, written or not;
//! - the dead band over the edge errors and every `int64_t`;
//! - the loop filter: proptest runs of up to 64 errors within +-10 s, at
//!   input rates of 8,000 to 192,000 Hz and 1 to 4096 frames per step,
//!   about 100,000 steps at the full level; both states and every delta
//!   after every step, and whether it engaged;
//! - the padding over 0 to 2^20 (every value at the full level) and the
//!   ends of `int`, engaged or not;
//! - the counters and the drift report over the ends of their counts.
//!
//! No intentional difference. Left out, as the C leaves them undefined or
//! never meets them: an estimate whose padding overflows `int` (the core
//! wraps) and a zero input rate (NaN states; `swr_init()` rejects the rate
//! first, and the Tier 1 tests pin what the core does). A null pointer,
//! which the C dereferences, is checked on the Rust side only: the shims
//! read nothing and write nothing.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use core::ffi::c_int;
use std::mem::{align_of, offset_of, size_of};

use obs_c_oracle::audio_resampler_drift as c;
use obs_media_io::audio_io::MAX_AUDIO_CHANNELS;
use obs_media_io::audio_resampler_drift::{
    self as core_, Action, C1, C2, Compensation, DEAD_BAND_NS, DISTANCE, GAIN, MAX_PPM, fmt,
};
use obs_media_io::ffi::audio_resampler_drift as shim;
use proptest::prelude::*;

const MS: i64 = 1_000_000;

fn to_c(s: &Compensation) -> c::Comp {
    c::Comp {
        enabled: s.enabled,
        engaged: s.engaged,
        error_ns: s.error_ns,
        vc1: s.vc1,
        vc2: s.vc2,
        input_samples: s.input_samples,
        output_samples: s.output_samples,
    }
}

/// The state's fields, the doubles as bits.
fn key_c(s: &c::Comp) -> (bool, bool, i64, u64, u64, u64, u64) {
    (
        s.enabled,
        s.engaged,
        s.error_ns,
        s.vc1.to_bits(),
        s.vc2.to_bits(),
        s.input_samples,
        s.output_samples,
    )
}

fn key(s: &Compensation) -> (bool, bool, i64, u64, u64, u64, u64) {
    key_c(&to_c(s))
}

/// The three sides of one comparison.
struct Sides {
    oracle: c::Comp,
    shim: Compensation,
    core: Compensation,
}

impl Sides {
    fn new(start: Compensation) -> Self {
        Self {
            oracle: to_c(&start),
            shim: start,
            core: start,
        }
    }

    fn check(&self, what: &str) -> Result<(), TestCaseError> {
        prop_assert_eq!(key(&self.shim), key_c(&self.oracle), "{}: shim state", what);
        prop_assert_eq!(key(&self.core), key_c(&self.oracle), "{}: core state", what);
        Ok(())
    }

    fn set_error(&mut self, error_ns: i64) -> Result<(), TestCaseError> {
        // SAFETY: a valid, writable struct on both sides.
        unsafe {
            c::oracle_audio_resampler_comp_set_error(&mut self.oracle, error_ns);
            shim::audio_resampler_comp_set_error(&mut self.shim, error_ns);
        }
        core_::set_error(&mut self.core, error_ns);
        self.check("set_error")
    }

    /// One step on all three, then (unless idle) the delta, as
    /// `audio_resampler_resample()` asks them; returns the oracle's action
    /// and delta.
    fn step(&mut self, input_freq: u32, in_frames: u32) -> Result<(u32, c_int), TestCaseError> {
        const UNSET: c_int = 0x5eed;
        // SAFETY: valid, writable structs.
        let (c_action, s_action) = unsafe {
            (
                c::oracle_audio_resampler_comp_step(&mut self.oracle, input_freq, in_frames),
                shim::audio_resampler_comp_step(&mut self.shim, input_freq, in_frames),
            )
        };
        let core_action = core_::step(&mut self.core, input_freq, in_frames);
        prop_assert_eq!(
            s_action,
            c_action,
            "shim step at {} Hz, {} frames",
            input_freq,
            in_frames
        );
        prop_assert_eq!(core_action.to_c(), c_action, "core action");
        self.check("step")?;
        if c_action == Action::Idle.to_c() {
            return Ok((c_action, UNSET));
        }
        // SAFETY: valid structs.
        let (c_delta, s_delta) = unsafe {
            (
                c::oracle_audio_resampler_comp_delta(&self.oracle, input_freq),
                shim::audio_resampler_comp_delta(&self.shim, input_freq),
            )
        };
        prop_assert_eq!(s_delta, c_delta, "shim delta at {} Hz", input_freq);
        prop_assert_eq!(core_::delta(&self.core, input_freq), c_delta, "core delta");
        Ok((c_action, c_delta))
    }

    fn count(&mut self, in_frames: u32, converted: c_int) -> Result<(), TestCaseError> {
        // SAFETY: valid, writable structs.
        unsafe {
            c::oracle_audio_resampler_comp_count(&mut self.oracle, in_frames, converted);
            shim::audio_resampler_comp_count(&mut self.shim, in_frames, converted);
        }
        core_::count(&mut self.core, in_frames, converted);
        self.check("count")
    }

    /// The drift report as `audio_resampler_destroy()` asks it: the gate,
    /// then the rate only when it is open.
    fn report(&self, output_freq: u32) -> Result<(), TestCaseError> {
        // SAFETY: valid structs.
        let (c_ok, s_ok) = unsafe {
            (
                c::oracle_audio_resampler_comp_reports_drift(&self.oracle),
                shim::audio_resampler_comp_reports_drift(&self.shim),
            )
        };
        prop_assert_eq!(s_ok, c_ok, "shim report gate");
        prop_assert_eq!(core_::reports_drift(&self.core), c_ok, "core report gate");
        if !c_ok {
            return Ok(());
        }
        // SAFETY: valid structs.
        let (c_rate, s_rate) = unsafe {
            (
                c::oracle_audio_resampler_comp_input_rate(&self.oracle, output_freq),
                shim::audio_resampler_comp_input_rate(&self.shim, output_freq),
            )
        };
        prop_assert_eq!(s_rate.to_bits(), c_rate.to_bits(), "shim rate");
        prop_assert_eq!(
            core_::input_rate(&self.core, output_freq).to_bits(),
            c_rate.to_bits(),
            "core rate"
        );
        Ok(())
    }
}

/// Runs a proptest-style check outside proptest, failing the test.
fn run(result: Result<(), TestCaseError>) {
    if let Err(e) = result {
        panic!("{e}");
    }
}

#[test]
fn constants_and_layout_match_c() {
    // SAFETY: the oracle reads its own table.
    let int = |i: usize| unsafe { c::oracle_audio_resampler_drift_int(i) };
    // SAFETY: as above.
    let double = |i: usize| unsafe { c::oracle_audio_resampler_drift_double(i) };
    let ints: [i64; 24] = [
        DEAD_BAND_NS,
        i64::from(DISTANCE),
        i64::from(fmt::U8),
        i64::from(fmt::S16),
        i64::from(fmt::S32),
        i64::from(fmt::FLT),
        i64::from(fmt::U8P),
        i64::from(fmt::S16P),
        i64::from(fmt::S32P),
        i64::from(fmt::FLTP),
        i64::try_from(MAX_AUDIO_CHANNELS).unwrap(),
        i64::from(Action::Idle.to_c()),
        i64::from(Action::Engage.to_c()),
        i64::from(Action::Set.to_c()),
        i64::try_from(size_of::<u32>()).unwrap(),
        i64::try_from(size_of::<Compensation>()).unwrap(),
        i64::try_from(align_of::<Compensation>()).unwrap(),
        i64::try_from(offset_of!(Compensation, enabled)).unwrap(),
        i64::try_from(offset_of!(Compensation, engaged)).unwrap(),
        i64::try_from(offset_of!(Compensation, error_ns)).unwrap(),
        i64::try_from(offset_of!(Compensation, vc1)).unwrap(),
        i64::try_from(offset_of!(Compensation, vc2)).unwrap(),
        i64::try_from(offset_of!(Compensation, input_samples)).unwrap(),
        i64::try_from(offset_of!(Compensation, output_samples)).unwrap(),
    ];
    for (i, (want, name)) in ints.into_iter().zip(c::INT_NAMES).enumerate() {
        assert_eq!(want, int(i), "{name}");
    }
    assert_eq!(int(ints.len()), -1);
    assert_eq!(size_of::<c::Comp>(), size_of::<Compensation>());
    for (i, (want, name)) in [MAX_PPM, GAIN, C1, C2]
        .into_iter()
        .zip(c::DOUBLE_NAMES)
        .enumerate()
    {
        assert_eq!(want.to_bits(), double(i).to_bits(), "{name}");
    }
}

#[test]
fn sample_formats_match_c() {
    for format in (-2..=20).chain([c_int::MIN, c_int::MAX]) {
        // SAFETY: the oracle takes any value.
        let want = unsafe { c::oracle_audio_resampler_sample_format(format) };
        assert_eq!(
            shim::audio_resampler_sample_format(format),
            want,
            "shim {format}"
        );
        assert_eq!(core_::sample_format(format), want, "core {format}");
    }
}

#[test]
fn upmix_rows_match_c() {
    for ch in (0..=9).chain([64, u32::MAX]) {
        let mut c_row = [-1.0f64; MAX_AUDIO_CHANNELS];
        let mut s_row = [-1.0f64; MAX_AUDIO_CHANNELS];
        // SAFETY: rows of MAX_AUDIO_CHANNELS writable doubles.
        let (c_ok, s_ok) = unsafe {
            (
                c::oracle_audio_resampler_upmix_row(ch, c_row.as_mut_ptr()),
                shim::audio_resampler_upmix_row(ch, &mut s_row),
            )
        };
        assert_eq!(s_ok, c_ok, "{ch} channels");
        assert_eq!(
            s_row.map(f64::to_bits),
            c_row.map(f64::to_bits),
            "{ch} channels"
        );
        assert_eq!(core_::upmix_row(ch).is_some(), c_ok, "{ch} channels");
        if let Some(row) = core_::upmix_row(ch) {
            assert_eq!(
                row.map(f64::to_bits),
                c_row.map(f64::to_bits),
                "{ch} channels"
            );
        }
    }
}

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
fn dead_band_edges_match_c() {
    for enabled in [false, true] {
        for error in EDGE_ERRORS {
            let mut sides = Sides::new(Compensation {
                enabled,
                vc1: 2.0,
                vc2: -1.0,
                ..Compensation::default()
            });
            run(sides.set_error(error));
        }
    }
}

/// Eight steps from each edge error, at the edge rates and frame counts,
/// engaged or not.
#[test]
fn step_edges_match_c() {
    for error in EDGE_ERRORS {
        for rate in [
            1,
            7,
            8_000,
            44_100,
            48_000,
            96_000,
            192_000,
            16_777_216,
            u32::MAX,
        ] {
            for in_frames in [0, 1, 480, 1024, 4096, 48_000, u32::MAX] {
                for engaged in [false, true] {
                    let mut sides = Sides::new(Compensation {
                        enabled: true,
                        engaged,
                        ..Compensation::default()
                    });
                    run(sides.set_error(error));
                    for _ in 0..8 {
                        run(sides.step(rate, in_frames).map(|_| ()));
                    }
                }
            }
        }
    }
}

/// Half-way deltas round away from zero on every side.
#[test]
fn delta_rounding_matches_c() {
    for vc1 in [
        0.5,
        -0.5,
        1.5,
        -1.5,
        2.5,
        -2.5,
        2.499_999_999,
        16_777.5,
        -16_777.5,
    ] {
        let mut sides = Sides::new(Compensation {
            enabled: true,
            engaged: true,
            vc1,
            vc2: vc1,
            ..Compensation::default()
        });
        run(sides.step(16_777_216, 0).map(|_| ()));
    }
}

#[test]
fn output_frames_match_c() {
    let ends = [
        0,
        1,
        -1,
        -511,
        -512,
        -513,
        510,
        511,
        512,
        513,
        1023,
        1024,
        48_000,
        i32::MAX - 4_194_305,
        c_int::MIN + 4_194_304,
    ];
    let sweep = (0..=(1 << 20))
        .filter(|&i: &c_int| obs_c_oracle::parity::keep(u64::from(i.cast_unsigned())));
    for estimated in sweep.chain(ends) {
        for engaged in [false, true] {
            // SAFETY: the oracle takes any value that does not overflow.
            let want = unsafe { c::oracle_audio_resampler_output_frames(estimated, engaged) };
            assert_eq!(
                shim::audio_resampler_output_frames(estimated, engaged),
                want,
                "{estimated}"
            );
            assert_eq!(
                core_::output_frames(estimated, engaged),
                want,
                "{estimated}"
            );
        }
    }
}

#[test]
fn counters_and_report_edges_match_c() {
    let counts = [
        0,
        1,
        47_999,
        48_000,
        48_001,
        1 << 53,
        (1 << 53) + 1,
        u64::MAX - 1,
        u64::MAX,
    ];
    for engaged in [false, true] {
        for input_samples in counts {
            for output_samples in counts {
                for (in_frames, converted) in [(0, 0), (480, 470), (u32::MAX, c_int::MAX)] {
                    let mut sides = Sides::new(Compensation {
                        enabled: true,
                        engaged,
                        input_samples,
                        output_samples,
                        ..Compensation::default()
                    });
                    for rate in [0, 1, 44_100, 48_000, u32::MAX] {
                        run(sides.report(rate));
                    }
                    run(sides.count(in_frames, converted));
                    run(sides.report(48_000));
                }
            }
        }
    }
}

/// A null pointer, which the C dereferences: the shims read and write
/// nothing (Rust side only).
#[test]
fn null_pointers_are_left_alone() {
    // SAFETY: null pointers, which the shims accept.
    unsafe {
        assert!(shim::audio_resampler_upmix_row(2, core::ptr::null_mut()));
        assert!(!shim::audio_resampler_upmix_row(0, core::ptr::null_mut()));
        shim::audio_resampler_comp_set_error(core::ptr::null_mut(), 50 * MS);
        shim::audio_resampler_comp_count(core::ptr::null_mut(), 480, 480);
        assert_eq!(
            shim::audio_resampler_comp_step(core::ptr::null_mut(), 48_000, 480),
            0
        );
        assert_eq!(
            shim::audio_resampler_comp_delta(core::ptr::null(), 48_000),
            0
        );
        assert!(!shim::audio_resampler_comp_reports_drift(core::ptr::null()));
        assert!(shim::audio_resampler_comp_input_rate(core::ptr::null(), 48_000).is_nan());
    }
}

/// One packet of a run: the error the source reports before it (if any),
/// its frames, and the frames converted.
fn packet() -> impl Strategy<Value = (Option<i64>, u32, c_int)> {
    (
        prop_oneof![
            2 => Just(None),
            1 => (-10_000 * MS..=10_000 * MS).prop_map(Some),
            1 => (-12 * MS..=12 * MS).prop_map(Some),
        ],
        1u32..=4096,
        0..=4400,
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(20_000)))]

    #[test]
    fn dead_band_matches_c(error in any::<i64>(), enabled in any::<bool>()) {
        let mut sides = Sides::new(Compensation { enabled, vc1: 3.0, vc2: 4.0, ..Compensation::default() });
        sides.set_error(error)?;
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(20_000)))]

    /// The delta alone, over every correction the clamp allows at any rate.
    #[test]
    fn delta_matches_c(rate in 1u32..=u32::MAX, fraction in -1.0f64..=1.0) {
        let vc1 = fraction * MAX_PPM * 1e-6 * f64::from(rate);
        let sides = Sides::new(Compensation { enabled: true, engaged: true, vc1, ..Compensation::default() });
        // SAFETY: valid structs.
        let (c_delta, s_delta) = unsafe {
            (
                c::oracle_audio_resampler_comp_delta(&sides.oracle, rate),
                shim::audio_resampler_comp_delta(&sides.shim, rate),
            )
        };
        prop_assert_eq!(s_delta, c_delta);
        prop_assert_eq!(core_::delta(&sides.core, rate), c_delta);
    }
}

proptest! {
    // About 100,000 steps at the full level.
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(700)))]

    /// The resampler's own order: the error, the step, the conversion's
    /// count; the report at the end.
    #[test]
    fn loop_matches_c(
        rate in 8_000u32..=192_000,
        output_rate in prop::sample::select(vec![8_000u32, 44_100, 48_000, 96_000, 192_000]),
        packets in prop::collection::vec((packet(), 1usize..=8), 1..=64),
    ) {
        let mut sides = Sides::new(Compensation::default());
        let mut engaged_at = None;
        for (i, ((error, in_frames, converted), repeat)) in packets.into_iter().enumerate() {
            if let Some(error) = error {
                sides.set_error(error)?;
            }
            for _ in 0..repeat {
                if sides.oracle.enabled {
                    let (action, _) = sides.step(rate, in_frames)?;
                    if action == 1 {
                        prop_assert!(engaged_at.is_none(), "engaged twice");
                        engaged_at = Some(i);
                    }
                }
                sides.count(in_frames, converted)?;
            }
        }
        sides.report(output_rate)?;
    }
}

//! Tier 3: the original extracted `obs_set_video_levels()` versus the shim on
//! one recording fixture core (oracle/set_video_levels.c), through the
//! real-named adapters. A script of calls runs on both from the same core;
//! compared: both levels after every call, every core byte at the end, and
//! every `blog` call (level, format string and the bits of each `%g`
//! argument), so the warning text is the same byte for byte. Calls: valid,
//! rejected (0, -0, negative, NaN, both infinities), mixed and repeated, and
//! no core at all.
//!
//! Excluded: a core without a graphics subsystem (the original asserts; the
//! shim alone is checked to read, write and log nothing there), an
//! uninitialized or dead core, and signaling NaNs in the random cases (32-bit
//! x86 float returns may quiet them). No intentional differences. Mutants show the
//! probe catches each broken step.
use core::ffi::{CStr, c_char, c_int};
use obs_core::ffi::set_video_levels::obs_set_video_levels;
use proptest::prelude::*;

type Set = unsafe extern "C" fn(f32, f32);
unsafe extern "C" {
    fn oracle_set_video_levels_probe(
        candidate: Set,
        core: bool,
        sdr: f32,
        hdr: f32,
        calls: *const f32,
        n: usize,
    ) -> u32;
    fn oracle_set_video_levels_original_log(
        core: bool,
        sdr: f32,
        hdr: f32,
        white: f32,
        peak: f32,
        levels: *mut c_int,
        formats: *mut *const c_char,
        bits: *mut u64,
        max: c_int,
    ) -> c_int;
    fn oracle_set_video_levels_no_graphics(candidate: Set, white: f32, peak: f32) -> bool;
    fn oracle_set_video_levels_adapters() -> bool;
    fn obs_set_video_levels_has_core() -> bool;
    fn obs_set_video_levels_store_sdr_white_level(level: f32);
    fn obs_set_video_levels_store_hdr_nominal_peak_level(level: f32);
    fn obs_set_video_levels_sdr_white_level() -> f32;
    fn obs_set_video_levels_hdr_nominal_peak_level() -> f32;
    fn obs_set_video_levels_warn_sdr_white_level(rejected: f32, kept: f32);
    fn obs_set_video_levels_warn_hdr_nominal_peak_level(rejected: f32, kept: f32);
}

const LOG_WARNING: c_int = 200;

fn probe(candidate: Set, core: bool, start: (f32, f32), calls: &[(f32, f32)]) -> u32 {
    let flat: Vec<f32> = calls.iter().flat_map(|&(w, p)| [w, p]).collect();
    // SAFETY: the oracle owns the fixture core every adapter reaches; the
    // calls outlive the probe.
    unsafe {
        oracle_set_video_levels_probe(
            candidate,
            core,
            start.0,
            start.1,
            flat.as_ptr(),
            calls.len(),
        )
    }
}

fn shim(core: bool, start: (f32, f32), calls: &[(f32, f32)]) -> u32 {
    probe(obs_set_video_levels, core, start, calls)
}

/// The original's log of one call: level, format and the two `%g` values.
fn original_log(core: bool, start: (f32, f32), call: (f32, f32)) -> Vec<(c_int, String, f64, f64)> {
    let mut levels = [0 as c_int; 4];
    let mut formats = [core::ptr::null::<c_char>(); 4];
    let mut bits = [0u64; 8];
    // SAFETY: each buffer holds `max` records.
    let n = unsafe {
        oracle_set_video_levels_original_log(
            core,
            start.0,
            start.1,
            call.0,
            call.1,
            levels.as_mut_ptr(),
            formats.as_mut_ptr(),
            bits.as_mut_ptr(),
            4,
        )
    };
    let n = usize::try_from(n).unwrap();
    (0..n)
        .map(|i| {
            // SAFETY: a string literal of the original.
            let format = unsafe { CStr::from_ptr(formats[i]) }.to_str().unwrap();
            (
                levels[i],
                format.to_owned(),
                f64::from_bits(bits[2 * i]),
                f64::from_bits(bits[2 * i + 1]),
            )
        })
        .collect()
}

const SDR: &str = "obs_set_video_levels: ignoring SDR white level %g, keeping %g";
const HDR: &str = "obs_set_video_levels: ignoring HDR nominal peak level %g, keeping %g";
const NO_CORE: &str = "obs_set_video_levels: no core, ignoring the levels";

#[test]
fn real_adapters_read_the_real_fields() {
    // SAFETY: the oracle arms and clears its own fixture.
    assert!(unsafe { oracle_set_video_levels_adapters() });
}

#[test]
fn original_c_is_the_characterized_setter() {
    let start = (200.0, 700.0);
    assert!(original_log(true, start, (250.0, 600.0)).is_empty());
    assert_eq!(
        original_log(true, start, (-1.0, 800.0)),
        [(LOG_WARNING, SDR.to_owned(), -1.0, 200.0)]
    );
    assert_eq!(
        original_log(true, start, (250.0, 0.0)),
        [(LOG_WARNING, HDR.to_owned(), 0.0, 700.0)]
    );
    assert_eq!(
        original_log(true, start, (-2.5, -3.5)),
        [
            (LOG_WARNING, SDR.to_owned(), -2.5, 200.0),
            (LOG_WARNING, HDR.to_owned(), -3.5, 700.0),
        ]
    );
    assert_eq!(
        original_log(false, start, (-1.0, 800.0)),
        [(LOG_WARNING, NO_CORE.to_owned(), 0.0, 0.0)]
    );
}

const BAD: [f32; 6] = [0.0, -0.0, -1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY];

#[test]
fn characterized_edges() {
    let valid = [
        (300.0, 1000.0),
        (80.0, 80.0),
        (1.0, 10000.0),
        (f32::MAX, f32::MAX),
        (f32::MIN_POSITIVE, f32::MIN_POSITIVE),
        (4_294_967_295.0, 4_294_967_295.0),
        (f32::from_bits(1), 1.0),
    ];
    for core in [true, false] {
        assert_eq!(shim(core, (300.0, 1000.0), &valid), 0);
        for bad in BAD {
            let calls = [(200.0, 600.0), (bad, 700.0), (250.0, bad), (bad, bad)];
            assert_eq!(shim(core, (300.0, 1000.0), &calls), 0, "{bad}");
            // Repeated rejections keep the stored level.
            assert_eq!(shim(core, (300.0, 1000.0), &[(bad, bad); 3]), 0, "{bad}");
        }
        for white in BAD {
            for peak in BAD {
                assert_eq!(shim(core, (200.0, 600.0), &[(white, peak)]), 0);
            }
        }
    }
    // A stored level that is itself out of range is still named as kept.
    assert_eq!(shim(true, (f32::NAN, -0.0), &[(0.0, 0.0), (5.0, 5.0)]), 0);
}

#[test]
fn shim_without_graphics_touches_nothing() {
    for (white, peak) in [(200.0, 600.0), (-1.0, 0.0), (f32::NAN, 700.0)] {
        // SAFETY: the oracle's fixture core, without graphics.
        assert!(unsafe { oracle_set_video_levels_no_graphics(obs_set_video_levels, white, peak) });
    }
}

/// Stores both levels without the rule.
unsafe extern "C" fn store_unchecked(white: f32, peak: f32) {
    // SAFETY: the oracle's fixture adapters.
    unsafe {
        if obs_set_video_levels_has_core() {
            obs_set_video_levels_store_sdr_white_level(white);
            obs_set_video_levels_store_hdr_nominal_peak_level(peak);
        }
    }
}

/// Rejects silently.
unsafe extern "C" fn reject_silently(white: f32, peak: f32) {
    // SAFETY: the oracle's fixture adapters.
    unsafe {
        if obs_set_video_levels_has_core() {
            if white.is_finite() && white > 0.0 {
                obs_set_video_levels_store_sdr_white_level(white);
            }
            if peak.is_finite() && peak > 0.0 {
                obs_set_video_levels_store_hdr_nominal_peak_level(peak);
            }
        }
    }
}

/// Logs the HDR warning before the SDR one.
unsafe extern "C" fn hdr_first(white: f32, peak: f32) {
    // SAFETY: the oracle's fixture adapters.
    unsafe {
        if !obs_set_video_levels_has_core() {
            return;
        }
        let ok = |l: f32| l.is_finite() && l > 0.0;
        if !ok(peak) {
            obs_set_video_levels_warn_hdr_nominal_peak_level(
                peak,
                obs_set_video_levels_hdr_nominal_peak_level(),
            );
        } else {
            obs_set_video_levels_store_hdr_nominal_peak_level(peak);
        }
        if !ok(white) {
            obs_set_video_levels_warn_sdr_white_level(
                white,
                obs_set_video_levels_sdr_white_level(),
            );
        } else {
            obs_set_video_levels_store_sdr_white_level(white);
        }
    }
}

/// Names the new level as the kept one.
unsafe extern "C" fn wrong_kept(white: f32, peak: f32) {
    // SAFETY: the oracle's fixture adapters.
    unsafe {
        if !obs_set_video_levels_has_core() {
            return;
        }
        let ok = |l: f32| l.is_finite() && l > 0.0;
        if ok(white) {
            obs_set_video_levels_store_sdr_white_level(white);
        } else {
            obs_set_video_levels_warn_sdr_white_level(white, white);
        }
        if ok(peak) {
            obs_set_video_levels_store_hdr_nominal_peak_level(peak);
        } else {
            obs_set_video_levels_warn_hdr_nominal_peak_level(peak, peak);
        }
    }
}

#[test]
fn mutants_are_caught() {
    let start = (300.0, 1000.0);
    assert_ne!(probe(store_unchecked, true, start, &[(-1.0, 700.0)]), 0);
    assert_ne!(probe(reject_silently, true, start, &[(-1.0, 700.0)]), 0);
    assert_ne!(probe(hdr_first, true, start, &[(-1.0, -2.0)]), 0);
    assert_ne!(probe(wrong_kept, true, start, &[(0.0, 700.0)]), 0);
    // No core: every mutant skips the warning.
    assert_ne!(probe(store_unchecked, false, start, &[(1.0, 1.0)]), 0);
}

/// Any level, signaling NaNs left out: 32-bit x86 float returns may quiet
/// them on either side.
fn level() -> impl Strategy<Value = f32> {
    prop_oneof![
        any::<f32>().prop_filter("signaling NaN", |l| !l.is_nan()
            || l.to_bits() & 0x0040_0000 != 0),
        Just(0.0f32),
        Just(-0.0f32),
        Just(f32::NAN),
        Just(f32::INFINITY),
        Just(f32::NEG_INFINITY),
        Just(f32::MAX),
        Just(f32::MIN_POSITIVE),
        (1.0f32..20000.0),
        (-20000.0f32..0.0),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn any_calls_match_original_c(
        core in prop::bool::weighted(0.9),
        start in (level(), level()),
        calls in prop::collection::vec((level(), level()), 0..16),
    ) {
        prop_assert_eq!(shim(core, start, &calls), 0);
    }
}

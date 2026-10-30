//! Tier 3: the shim against the original `obs-audio-controls.c.h`.
//!
//! The curves and `obs_mul_to_db` / `obs_db_to_mul` go through the public
//! fader API. A null `get_nr_channels`, `set_peak_meter_type`, or
//! `db_to_def` is not called. The source, the volume and destroy signals and
//! the capture callback are file-local and need a live source, so they stay
//! with the unchanged cmocka test. The two SSE peak kernels are compared
//! directly: the oracle exports `get_sample_peak()` and `get_true_peak()` as
//! `oracle_get_sample_peak`/`oracle_get_true_peak` (#3003), bit for bit,
//! NaN samples included (#3005: `_mm_max_ps` returns its second operand when
//! either is NaN).
//!
//! `struct obs_audio_info` is the one repr(C) value the shim writes. The
//! fader and the volmeter are opaque in the header.

use std::ffi::c_void;
use std::mem::{align_of, offset_of, size_of};
use std::ptr;

use obs_audio_controls::ffi::audio_controls::{
    ObsAudioInfo, obs_db_to_mul, obs_fader_create, obs_fader_db_to_def, obs_fader_destroy,
    obs_fader_get_db, obs_fader_get_deflection, obs_fader_get_mul, obs_fader_set_db,
    obs_fader_set_deflection, obs_fader_set_mul, obs_mul_to_db, obs_volmeter_create,
    obs_volmeter_destroy,
};
use obs_audio_controls::peak;
use obs_c_oracle::audio_controls::{
    oracle_get_sample_peak, oracle_get_true_peak, oracle_obs_audio_info_align,
    oracle_obs_audio_info_size, oracle_obs_audio_info_speakers_offset, oracle_obs_db_to_mul,
    oracle_obs_fader_create, oracle_obs_fader_db_to_def, oracle_obs_fader_destroy,
    oracle_obs_fader_get_db, oracle_obs_fader_get_deflection, oracle_obs_fader_get_mul,
    oracle_obs_fader_set_db, oracle_obs_fader_set_deflection, oracle_obs_fader_set_mul,
    oracle_obs_mul_to_db, oracle_obs_volmeter_create, oracle_obs_volmeter_destroy,
};
use proptest::prelude::*;

/// Finite values within 0.0001. NaNs match each other. Infinities match by bits.
fn same(a: f32, b: f32) -> bool {
    if a.is_nan() && b.is_nan() {
        return true;
    }
    if a.is_infinite() || b.is_infinite() {
        return a.to_bits() == b.to_bits();
    }
    (a - b).abs() <= 0.0001
}

fn assert_same(what: &str, rust: f32, c: f32) {
    assert!(same(rust, c), "{what}: rust {rust} oracle {c}");
}

struct Faders {
    rust: *mut c_void,
    c: *mut c_void,
}

impl Drop for Faders {
    fn drop(&mut self) {
        // SAFETY: each pointer is null or a fader this test created.
        unsafe {
            if !self.rust.is_null() {
                obs_fader_destroy(self.rust);
                self.rust = ptr::null_mut();
            }
            if !self.c.is_null() {
                oracle_obs_fader_destroy(self.c);
                self.c = ptr::null_mut();
            }
        }
    }
}

fn open(kind: i32) -> Faders {
    // SAFETY: create returns null or a fader. Drop destroys both.
    let pair = unsafe {
        Faders {
            rust: obs_fader_create(kind),
            c: oracle_obs_fader_create(kind),
        }
    };
    assert_eq!(pair.rust.is_null(), pair.c.is_null(), "kind {kind} create");
    assert!(!pair.rust.is_null(), "kind {kind} create returned null");
    pair
}

fn check_deflection(kind: i32, def: f32) {
    let pair = open(kind);
    // SAFETY: both faders are live until `pair` drops.
    unsafe {
        let rust_ok = obs_fader_set_deflection(pair.rust, def);
        let c_ok = oracle_obs_fader_set_deflection(pair.c, def);
        assert_eq!(rust_ok, c_ok, "kind {kind} set_deflection {def}");
        assert_same(
            "db",
            obs_fader_get_db(pair.rust),
            oracle_obs_fader_get_db(pair.c),
        );
        assert_same(
            "deflection",
            obs_fader_get_deflection(pair.rust),
            oracle_obs_fader_get_deflection(pair.c),
        );
        assert_same(
            "mul",
            obs_fader_get_mul(pair.rust),
            oracle_obs_fader_get_mul(pair.c),
        );
        let rust_curve = obs_fader_db_to_def(pair.rust);
        let c_curve = oracle_obs_fader_db_to_def(pair.c);
        assert_same("db_to_def(0)", rust_curve(0.0), c_curve(0.0));
        let db = obs_fader_get_db(pair.rust);
        assert_same("db_to_def(db)", rust_curve(db), c_curve(db));
    }
}

fn check_db(kind: i32, db: f32) {
    let pair = open(kind);
    // SAFETY: both faders are live until `pair` drops.
    unsafe {
        let rust_ok = obs_fader_set_db(pair.rust, db);
        let c_ok = oracle_obs_fader_set_db(pair.c, db);
        assert_eq!(rust_ok, c_ok, "kind {kind} set_db {db}");
        assert_same(
            "db",
            obs_fader_get_db(pair.rust),
            oracle_obs_fader_get_db(pair.c),
        );
        assert_same(
            "deflection",
            obs_fader_get_deflection(pair.rust),
            oracle_obs_fader_get_deflection(pair.c),
        );
        assert_same(
            "mul",
            obs_fader_get_mul(pair.rust),
            oracle_obs_fader_get_mul(pair.c),
        );
    }
}

fn check_mul(kind: i32, mul: f32) {
    let pair = open(kind);
    // SAFETY: both faders are live until `pair` drops.
    unsafe {
        let rust_ok = obs_fader_set_mul(pair.rust, mul);
        let c_ok = oracle_obs_fader_set_mul(pair.c, mul);
        assert_eq!(rust_ok, c_ok, "kind {kind} set_mul {mul}");
        assert_same(
            "db",
            obs_fader_get_db(pair.rust),
            oracle_obs_fader_get_db(pair.c),
        );
        assert_same(
            "mul",
            obs_fader_get_mul(pair.rust),
            oracle_obs_fader_get_mul(pair.c),
        );
    }
}

fn check_convert(mul: f32, db: f32) {
    // SAFETY: both functions are pure.
    unsafe {
        assert_same("mul_to_db", obs_mul_to_db(mul), oracle_obs_mul_to_db(mul));
        assert_same("db_to_mul", obs_db_to_mul(db), oracle_obs_db_to_mul(db));
    }
}

#[test]
fn audio_info_matches_oracle() {
    // SAFETY: the oracle reports the C struct. No pointer is dereferenced.
    unsafe {
        assert_eq!(size_of::<ObsAudioInfo>(), oracle_obs_audio_info_size());
        assert_eq!(align_of::<ObsAudioInfo>(), oracle_obs_audio_info_align());
        assert_eq!(
            offset_of!(ObsAudioInfo, speakers),
            oracle_obs_audio_info_speakers_offset()
        );
        assert_eq!(offset_of!(ObsAudioInfo, samples_per_sec), 0);
    }
}

#[test]
fn curves_match_oracle() {
    // Cubic 0.5 is -18.0618 dB. A square instead of a cube fails here.
    check_deflection(0, 0.5);
    let points = [
        0.0, 0.001, 0.025, 0.075, 0.15, 0.3, 0.5, 0.75, 1.0, 1.5, -0.25,
    ];
    for kind in 0..3 {
        for def in points {
            check_deflection(kind, def);
        }
    }
    let dbs = [
        0.0,
        -9.0,
        -18.0618,
        -20.0,
        -30.0,
        -40.0,
        -50.0,
        -60.0,
        -96.0,
        -100.0,
        -114.0,
        -146.4,
        -1000.0,
        1.0,
        f32::NEG_INFINITY,
        f32::INFINITY,
        f32::NAN,
    ];
    for kind in 0..3 {
        for db in dbs {
            check_db(kind, db);
        }
    }
    for kind in 0..3 {
        for mul in [0.0, 0.5, 1.0, 2.0, -1.0, f32::NAN, f32::INFINITY] {
            check_mul(kind, mul);
        }
    }
    for mul in [0.0, 0.5, 1.0, 2.0, f32::NAN] {
        check_convert(mul, mul);
    }
    check_convert(1.0, 0.0);
    check_convert(0.0, f32::NEG_INFINITY);
    check_convert(0.5, f32::NAN);
}

#[test]
fn bad_type_matches_oracle() {
    // SAFETY: type 99 creates no fader. The meter is kept, then destroyed.
    unsafe {
        assert!(obs_fader_create(99).is_null());
        assert!(oracle_obs_fader_create(99).is_null());
        let rust = obs_volmeter_create(99);
        let c = oracle_obs_volmeter_create(99);
        assert!(!rust.is_null());
        assert!(!c.is_null());
        obs_volmeter_destroy(rust);
        oracle_obs_volmeter_destroy(c);
    }
}

proptest! {
    #[test]
    fn random_deflection_matches_oracle(def in 0.0f32..=1.0) {
        for kind in 0..3 {
            check_deflection(kind, def);
        }
    }

    #[test]
    fn random_db_matches_oracle(db in -200.0f32..=20.0) {
        for kind in 0..3 {
            check_db(kind, db);
        }
    }
}

/// One tick of samples, 16-byte aligned for the C's `_mm_load_ps`.
#[repr(C, align(16))]
struct Samples([f32; 64]);

/// Both kernels on both sides, compared as bits (a NaN result must match).
fn check_peaks(previous: [f32; 4], samples: &[f32]) {
    let mut aligned = Samples([0.0; 64]);
    aligned.0[..samples.len()].copy_from_slice(samples);
    let frames = samples.len();
    // SAFETY: `previous` holds 4 floats, `aligned` 64 floats, 16-byte aligned.
    let (c_sample, c_true) = unsafe {
        (
            oracle_get_sample_peak(previous.as_ptr(), aligned.0.as_ptr(), frames),
            oracle_get_true_peak(previous.as_ptr(), aligned.0.as_ptr(), frames),
        )
    };
    let rust_sample = peak::sample_peak(previous, &aligned.0, frames);
    let rust_true = peak::true_peak(previous, &aligned.0, frames);
    assert_eq!(
        rust_sample.to_bits(),
        c_sample.to_bits(),
        "sample_peak: rust {rust_sample} c {c_sample} for {previous:?} {samples:?}"
    );
    assert_eq!(
        rust_true.to_bits(),
        c_true.to_bits(),
        "true_peak: rust {rust_true} c {c_true} for {previous:?} {samples:?}"
    );
}

/// A NaN in a lane's last group: `_mm_max_ps(peak, NaN)` is NaN, and the
/// final `fmaxf` skips it, so the C drops that lane's earlier 0.5 (#3005).
#[test]
fn nan_in_the_last_group_matches_c() {
    check_peaks([0.0; 4], &[0.5, 0.0, 0.0, 0.0, f32::NAN, 0.0, 0.0, 0.0]);
    check_peaks([0.0; 4], &[0.0, 0.0, 0.0, 0.25, 0.0, 0.0, 0.0, f32::NAN]);
    check_peaks([f32::NAN, 0.0, 0.0, 0.0], &[0.0; 4]);
}

/// A sample: mostly audio range, sometimes NaN, infinity or a large value.
fn sample() -> impl Strategy<Value = f32> {
    prop_oneof![
        8 => -1.5f32..=1.5,
        1 => Just(f32::NAN),
        1 => prop_oneof![Just(f32::INFINITY), Just(f32::NEG_INFINITY), Just(-0.0f32), Just(1.0e30f32)],
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(2000)))]

    #[test]
    fn random_peaks_match_c(
        previous in prop::array::uniform4(sample()),
        samples in prop::collection::vec(sample(), 0..=64),
    ) {
        check_peaks(previous, &samples);
    }
}

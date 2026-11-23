//! Tier 3: the original `obs_reset_audio()` (oracle `reset_audio.c`) and the
//! Rust shim against one recording `obs_reset_audio2` stand-in, which the
//! shim reaches through the oracle's copy of the real
//! `obs_reset_audio_fields()` adapter. For each rate, layout and stand-in
//! answer, both must make exactly one call carrying the rate and layout with
//! `max_buffering_ms` and `fixed_buffering` zeroed, and return the answer.
//! Four mutants of the shim must fail.
use core::ffi::c_int;
use obs_core::ffi::audio_info_queries::{obs_audio_info, obs_audio_info2};
use obs_core::ffi::reset_audio::{Reset2, obs_reset_audio};
use proptest::prelude::*;

type ResetFn = unsafe extern "C" fn(*const obs_audio_info) -> bool;

unsafe extern "C" {
    fn oracle_reset_audio_check(
        samples_per_sec: u32,
        speakers: i32,
        result: bool,
        shim: ResetFn,
    ) -> bool;
    fn obs_reset_audio_fields() -> Reset2;
}

fn check(samples_per_sec: u32, speakers: c_int, result: bool, shim: ResetFn) -> bool {
    // SAFETY: the oracle runs the original and `shim` on a stack info
    // against its thread-local stand-in; `shim` reads only that info.
    unsafe { oracle_reset_audio_check(samples_per_sec, speakers, result, shim) }
}

/// The adapter's stand-in, called with `oai2`.
unsafe fn reset2(oai2: &obs_audio_info2) -> bool {
    // SAFETY: the oracle's adapter returns its recording stand-in, and the
    // oracle check set its record for this thread.
    unsafe { obs_reset_audio_fields()(oai2) }
}

unsafe extern "C" fn mutant_fixed(oai: *const obs_audio_info) -> bool {
    // SAFETY: the oracle passes a readable info.
    let oai = unsafe { &*oai };
    let oai2 = obs_audio_info2 {
        samples_per_sec: oai.samples_per_sec,
        speakers: oai.speakers,
        max_buffering_ms: 0,
        fixed_buffering: true,
    };
    // SAFETY: as in `reset2`.
    unsafe { reset2(&oai2) }
}

unsafe extern "C" fn mutant_max(oai: *const obs_audio_info) -> bool {
    // SAFETY: the oracle passes a readable info.
    let oai = unsafe { &*oai };
    let oai2 = obs_audio_info2 {
        samples_per_sec: oai.samples_per_sec,
        speakers: oai.speakers,
        max_buffering_ms: 1,
        fixed_buffering: false,
    };
    // SAFETY: as in `reset2`.
    unsafe { reset2(&oai2) }
}

unsafe extern "C" fn mutant_twice(oai: *const obs_audio_info) -> bool {
    // SAFETY: the oracle passes a readable info for both calls.
    unsafe {
        obs_reset_audio(oai);
        obs_reset_audio(oai)
    }
}

unsafe extern "C" fn mutant_true(oai: *const obs_audio_info) -> bool {
    // SAFETY: the oracle passes a readable info.
    unsafe { obs_reset_audio(oai) };
    true
}

const CASES: [(u32, c_int); 7] = [
    (8000, 1),
    (22050, 3),
    (44100, 6),
    (48000, 2),
    (96000, 8),
    (0, 0),
    (u32::MAX, -1),
];

#[test]
fn shim_matches_the_original() {
    for (rate, speakers) in CASES {
        for result in [false, true] {
            assert!(
                check(rate, speakers, result, obs_reset_audio),
                "{rate} {speakers} {result}"
            );
        }
    }
}

#[test]
fn mutants_are_caught() {
    let mutants: [(&str, ResetFn); 4] = [
        ("fixed buffering", mutant_fixed),
        ("non-zero maximum", mutant_max),
        ("two calls", mutant_twice),
        ("always true", mutant_true),
    ];
    for (name, mutant) in mutants {
        let caught = CASES.iter().any(|&(rate, speakers)| {
            [false, true]
                .iter()
                .any(|&result| !check(rate, speakers, result, mutant))
        });
        assert!(caught, "mutant not caught: {name}");
    }
}

proptest! {
    #[test]
    fn shim_matches_the_original_on_any_info(rate in any::<u32>(), speakers in any::<c_int>(), result in any::<bool>()) {
        prop_assert!(check(rate, speakers, result, obs_reset_audio));
    }
}

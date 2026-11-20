//! Original-C parity for the audio-info queries (#5363): the thin ABI shims
//! and real field adapters against the extracted C, on one real-header core
//! filled with any byte. Compared: the result, the caller's sentinel-filled
//! output bytes, the audio_output_get_info call log, the core bytes and the
//! current core pointer. Excludes a dead core and concurrent writes to the
//! audio fields, matching the C contract. No intentional differences.
use core::ffi::c_int;
use core::mem::{align_of, offset_of, size_of};

use obs_core::ffi::audio_info_queries::{
    obs_audio_info, obs_audio_info2, obs_get_audio_info, obs_get_audio_info2,
};
use obs_media_io::ffi::audio_io::audio_output_info;
use proptest::prelude::*;

type InfoFn = unsafe extern "C" fn(*mut obs_audio_info) -> bool;
type Info2Fn = unsafe extern "C" fn(*mut obs_audio_info2) -> bool;

unsafe extern "C" {
    fn oracle_audio_info_queries_layout(out: *mut usize);
    fn oracle_audio_info_queries_probe(
        which: c_int,
        cand1: InfoFn,
        cand2: Info2Fn,
        mode: c_int,
        out_null: bool,
        fill: u8,
        rate: u32,
        speakers: c_int,
        fixed: bool,
        ticks: c_int,
        original: *mut bool,
    ) -> u32;
}

#[test]
fn real_header_layout() {
    let mut layout = [0usize; 13];
    // SAFETY: C writes exactly thirteen values into this array.
    unsafe { oracle_audio_info_queries_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<obs_audio_info>(),
            align_of::<obs_audio_info>(),
            offset_of!(obs_audio_info, samples_per_sec),
            offset_of!(obs_audio_info, speakers),
            size_of::<obs_audio_info2>(),
            align_of::<obs_audio_info2>(),
            offset_of!(obs_audio_info2, samples_per_sec),
            offset_of!(obs_audio_info2, speakers),
            offset_of!(obs_audio_info2, max_buffering_ms),
            offset_of!(obs_audio_info2, fixed_buffering),
            offset_of!(audio_output_info, samples_per_sec),
            offset_of!(audio_output_info, speakers),
            size_of::<c_int>(),
        ]
    );
}

struct Case {
    mode: c_int,
    out_null: bool,
    fill: u8,
    rate: u32,
    speakers: c_int,
    fixed: bool,
    ticks: c_int,
}

/// Runs both queries' original and shim; returns the original's results.
fn probe_with(case: &Case, cand1: InfoFn, cand2: Info2Fn) -> ([bool; 2], [u32; 2]) {
    let mut original = [false; 2];
    let mut diff = [0u32; 2];
    for (which, (orig, d)) in original.iter_mut().zip(diff.iter_mut()).enumerate() {
        let which = c_int::try_from(which).unwrap() + 1;
        // SAFETY: the oracle owns a live real-header core, installs it as the
        // thread's current core for the call and passes the candidates a
        // sentinel-filled output or NULL.
        *d = unsafe {
            oracle_audio_info_queries_probe(
                which,
                cand1,
                cand2,
                case.mode,
                case.out_null,
                case.fill,
                case.rate,
                case.speakers,
                case.fixed,
                case.ticks,
                orig,
            )
        };
    }
    (original, diff)
}

fn check(case: &Case) {
    let (original, diff) = probe_with(case, obs_get_audio_info, obs_get_audio_info2);
    assert_eq!(
        diff,
        [0, 0],
        "mode {} out_null {} fill {:#04x} rate {} speakers {} fixed {} ticks {}",
        case.mode,
        case.out_null,
        case.fill,
        case.rate,
        case.speakers,
        case.fixed,
        case.ticks
    );
    // The original's results are the C decision table.
    let ok = case.mode == 2 && !case.out_null;
    assert_eq!(original, [ok, ok]);
}

#[test]
fn characterized_cases_match_original_c() {
    for mode in 0..3 {
        for out_null in [false, true] {
            for (rate, speakers, fixed, ticks) in [
                (48000, 2, false, 1),
                (44100, 6, true, 45),
                (96000, 4, true, 0),
                (8000, 1, false, i32::MAX),
                (0, 0, false, 0),
                (u32::MAX, 8, true, -1),
                (48000, -1, false, i32::MIN),
            ] {
                for fill in [0x00, 0xa5, 0xff] {
                    check(&Case {
                        mode,
                        out_null,
                        fill,
                        rate,
                        speakers,
                        fixed,
                        ticks,
                    });
                }
            }
        }
    }
}

// Mutants of the shim, each of which the probe must catch.
unsafe extern "C" fn mutant_info_ignores_audio(oai: *mut obs_audio_info) -> bool {
    if oai.is_null() {
        return false;
    }
    // SAFETY: non-NULL caller output.
    unsafe {
        oai.write(obs_audio_info {
            samples_per_sec: 48000,
            speakers: 2,
        })
    };
    true
}

unsafe extern "C" fn mutant_info2_skips_inner_read(oai2: *mut obs_audio_info2) -> bool {
    if oai2.is_null() {
        // The original still reads the info into its local first.
        return false;
    }
    // SAFETY: forwarded unchanged.
    unsafe { obs_get_audio_info2(oai2) }
}

unsafe extern "C" fn mutant_info2_clears_output_on_failure(oai2: *mut obs_audio_info2) -> bool {
    // SAFETY: forwarded unchanged.
    let ok = unsafe { obs_get_audio_info2(oai2) };
    if !ok && !oai2.is_null() {
        // SAFETY: non-NULL caller output.
        unsafe {
            oai2.write(obs_audio_info2 {
                samples_per_sec: 0,
                speakers: 0,
                max_buffering_ms: 0,
                fixed_buffering: false,
            });
        }
    }
    ok
}

unsafe extern "C" fn mutant_info2_rounds_ms(oai2: *mut obs_audio_info2) -> bool {
    // SAFETY: forwarded unchanged.
    let ok = unsafe { obs_get_audio_info2(oai2) };
    if ok {
        // SAFETY: the shim succeeded, so `oai2` is a written caller output.
        unsafe { (*oai2).max_buffering_ms = (*oai2).max_buffering_ms.saturating_add(1) };
    }
    ok
}

#[test]
fn mutants_are_caught() {
    let with_audio = Case {
        mode: 2,
        out_null: false,
        fill: 0xa5,
        rate: 44100,
        speakers: 6,
        fixed: true,
        ticks: 45,
    };
    let no_audio = Case {
        mode: 1,
        ..with_audio
    };
    let null_out = Case {
        out_null: true,
        ..with_audio
    };

    let (_, diff) = probe_with(&no_audio, mutant_info_ignores_audio, obs_get_audio_info2);
    assert_ne!(diff[0], 0, "info that ignores the audio output");
    let (_, diff) = probe_with(&null_out, obs_get_audio_info, mutant_info2_skips_inner_read);
    assert_ne!(diff[1], 0, "info2 that skips the inner info read");
    let (_, diff) = probe_with(
        &no_audio,
        obs_get_audio_info,
        mutant_info2_clears_output_on_failure,
    );
    assert_ne!(diff[1], 0, "info2 that clears its output on failure");
    let (_, diff) = probe_with(&with_audio, obs_get_audio_info, mutant_info2_rounds_ms);
    assert_ne!(diff[1], 0, "info2 with a changed buffering time");
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn random_cores_match_original_c(
        mode in 0..3 as c_int,
        out_null in any::<bool>(),
        fill in any::<u8>(),
        rate in any::<u32>(),
        speakers in any::<c_int>(),
        fixed in any::<bool>(),
        ticks in any::<c_int>(),
    ) {
        check(&Case { mode, out_null, fill, rate, speakers, fixed, ticks });
    }
}

//! Tier 3: the Rust C ABI shims and safe core behave exactly like the
//! original C, compiled as an oracle.
//!
//! No intentional differences from C. Parity is bit-exact: `vec3_rotate`'s
//! SSE dot product is the only vector code, and `obs_graphics::vec3::Vec3`
//! sums its lanes in the SSE order.
//!
//! The inputs are three small enums, so besides proptest over arbitrary
//! `c_int`s the tests sweep every enumerator plus values just outside each
//! enum, with each combination of null range pointers. The sweep covers
//! every input of `test/cmocka/test_video_matrices.c`.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use core::ffi::c_int;
use core::ptr;

use obs_c_oracle::video_matrices as c;
use obs_media_io::ffi::video_matrices as rs;
use obs_media_io::video_io::{
    VideoColorspace, VideoFormat, VideoRangeType, video_colorspace_to_c, video_format_to_c,
    video_range_type_to_c,
};
use obs_media_io::video_matrices::{ColorParameters, parameters, parameters_for_format};
use proptest::prelude::*;

/// Fill for the out-buffers, so an entry a function does not write shows up.
const SENTINEL: f32 = f32::from_bits(0x5a5a_5a5a);

/// Everything one call returns or writes, as raw bits.
#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    success: bool,
    matrix: [u32; 16],
    range_min: [u32; 3],
    range_max: [u32; 3],
}

/// Which range pointers a call passes as null.
#[derive(Debug, Clone, Copy)]
struct Nulls {
    range_min: bool,
    range_max: bool,
}

const ALL_NULLS: [Nulls; 4] = [
    Nulls {
        range_min: false,
        range_max: false,
    },
    Nulls {
        range_min: true,
        range_max: false,
    },
    Nulls {
        range_min: false,
        range_max: true,
    },
    Nulls {
        range_min: true,
        range_max: true,
    },
];

fn bits<const N: usize>(values: [f32; N]) -> [u32; N] {
    values.map(f32::to_bits)
}

/// Calls `f` with sentinel-filled buffers, passing null for the range
/// pointers `nulls` selects, and records the result.
fn run(nulls: Nulls, f: impl FnOnce(*mut f32, *mut f32, *mut f32) -> bool) -> Outcome {
    let mut matrix = [SENTINEL; 16];
    let mut range_min = [SENTINEL; 3];
    let mut range_max = [SENTINEL; 3];
    let min_ptr = if nulls.range_min {
        ptr::null_mut()
    } else {
        range_min.as_mut_ptr()
    };
    let max_ptr = if nulls.range_max {
        ptr::null_mut()
    } else {
        range_max.as_mut_ptr()
    };
    let success = f(matrix.as_mut_ptr(), min_ptr, max_ptr);
    Outcome {
        success,
        matrix: bits(matrix),
        range_min: bits(range_min),
        range_max: bits(range_max),
    }
}

fn outcome_of(params: &ColorParameters) -> Outcome {
    Outcome {
        success: true,
        matrix: bits(params.matrix),
        range_min: bits(params.range_min),
        range_max: bits(params.range_max),
    }
}

/// The shim and oracle `video_format_get_parameters` agree.
fn check_get(cs: c_int, range: c_int, nulls: Nulls) -> Result<(), TestCaseError> {
    // SAFETY: the buffers `run` passes hold 16 and 3 floats, or are null
    // for the range pointers, as both functions require.
    let want = run(nulls, |m, mn, mx| unsafe {
        c::oracle_video_format_get_parameters(cs, range, m, mn, mx)
    });
    // SAFETY: as above.
    let got = run(nulls, |m, mn, mx| unsafe {
        rs::video_format_get_parameters(cs, range, m, mn, mx)
    });
    prop_assert_eq!(got, want, "cs {}, range {}, {:?}", cs, range, nulls);
    Ok(())
}

/// The shim and oracle `video_format_get_parameters_for_format` agree.
fn check_for_format(
    cs: c_int,
    range: c_int,
    format: c_int,
    nulls: Nulls,
) -> Result<(), TestCaseError> {
    // SAFETY: as in `check_get`.
    let want = run(nulls, |m, mn, mx| unsafe {
        c::oracle_video_format_get_parameters_for_format(cs, range, format, m, mn, mx)
    });
    // SAFETY: as above.
    let got = run(nulls, |m, mn, mx| unsafe {
        rs::video_format_get_parameters_for_format(cs, range, format, m, mn, mx)
    });
    prop_assert_eq!(
        got,
        want,
        "cs {}, range {}, format {}, {:?}",
        cs,
        range,
        format,
        nulls
    );
    Ok(())
}

/// Every C value of an enum, plus two on each side of it.
fn sweep(values: impl Iterator<Item = c_int>) -> Vec<c_int> {
    let values: Vec<c_int> = values.collect();
    let min = *values.iter().min().unwrap();
    let max = *values.iter().max().unwrap();
    (min - 2..=max + 2).collect()
}

fn colorspaces() -> Vec<c_int> {
    sweep(VideoColorspace::ALL.into_iter().map(video_colorspace_to_c))
}

fn ranges() -> Vec<c_int> {
    sweep(VideoRangeType::ALL.into_iter().map(video_range_type_to_c))
}

fn formats() -> Vec<c_int> {
    sweep(VideoFormat::ALL.into_iter().map(video_format_to_c))
}

#[test]
fn every_get_parameters_input_matches_c_oracle() {
    for cs in colorspaces() {
        for range in ranges() {
            for nulls in ALL_NULLS {
                check_get(cs, range, nulls).unwrap();
            }
        }
    }
}

#[test]
fn every_get_parameters_for_format_input_matches_c_oracle() {
    for cs in colorspaces() {
        for range in ranges() {
            for format in formats() {
                for nulls in ALL_NULLS {
                    check_for_format(cs, range, format, nulls).unwrap();
                }
            }
        }
    }
}

/// The safe core, without the shim, matches the oracle on every enumerator.
#[test]
fn safe_core_matches_c_oracle() {
    let both = Nulls {
        range_min: false,
        range_max: false,
    };
    for cs in VideoColorspace::ALL {
        for range in VideoRangeType::ALL {
            let (cs_c, range_c) = (video_colorspace_to_c(cs), video_range_type_to_c(range));
            // SAFETY: as in `check_get`.
            let want = run(both, |m, mn, mx| unsafe {
                c::oracle_video_format_get_parameters(cs_c, range_c, m, mn, mx)
            });
            assert_eq!(outcome_of(&parameters(cs, range)), want, "{cs:?} {range:?}");

            for format in VideoFormat::ALL {
                let format_c = video_format_to_c(format);
                // SAFETY: as above.
                let want = run(both, |m, mn, mx| unsafe {
                    c::oracle_video_format_get_parameters_for_format(
                        cs_c, range_c, format_c, m, mn, mx,
                    )
                });
                assert_eq!(
                    outcome_of(&parameters_for_format(cs, range, format)),
                    want,
                    "{cs:?} {range:?} {format:?}"
                );
            }
        }
    }
}

fn nulls_strategy() -> impl Strategy<Value = Nulls> {
    prop::sample::select(&ALL_NULLS[..])
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(10_000)))]

    #[test]
    fn arbitrary_get_parameters_input_matches_c_oracle(
        cs in any::<c_int>(),
        range in any::<c_int>(),
        nulls in nulls_strategy(),
    ) {
        check_get(cs, range, nulls)?;
    }

    #[test]
    fn arbitrary_get_parameters_for_format_input_matches_c_oracle(
        cs in any::<c_int>(),
        range in any::<c_int>(),
        format in any::<c_int>(),
        nulls in nulls_strategy(),
    ) {
        check_for_format(cs, range, format, nulls)?;
    }
}

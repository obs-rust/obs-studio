//! Tier 2 layout and Tier 3 parity: the shims of
//! `obs_output::ffi::encoder_gpu_rescale` (and the safe core under them)
//! against the original `libobs/obs-encoder-gpu-rescale.c.h`, compiled as
//! `oracle_obs_encoder_gpu_rescale_*` (obs-c-oracle,
//! oracle/encoder_gpu_rescale.c).
//!
//! Every output is compared exactly: the target field by field, the gate and
//! the match as bools, the release step as its value, the reference count
//! and its "last" flag.
//!
//! Runs: the constants and the layout of the two structs and the enum; the
//! gate and the target over a grid of 25,600 requests (scale types, scaled
//! sizes at the `u32` edges, enum values including the `i32` extremes),
//! with and without media and against three video outputs; the match over
//! three targets with each of eight conditions changed alone and every pair
//! of them; every release input; reference counts at the `long` edges; and
//! proptests over full-range requests, outputs and counts (biased to the
//! "none" values, to equal outputs and to counts near 0).
//!
//! Excluded: a reference count of `LONG_MIN`, undefined in the C (its
//! decrement overflows). No intentional differences.
//! `test_encoder_gpu_rescale_steps` runs the C test against the C; the
//! real-libobs rescale tests run against the hybrid libobs, where these
//! shims replace the C.

use std::ffi::{c_int, c_long, c_void};
use std::mem::{align_of, offset_of, size_of};

use obs_c_oracle::encoder_gpu_rescale::{
    CONSTANT_NAMES, LAYOUT_NAMES, Output as OracleOutput, oracle_encoder_gpu_rescale_constant,
    oracle_encoder_gpu_rescale_layout, oracle_obs_encoder_gpu_rescale_matches,
    oracle_obs_encoder_gpu_rescale_release_refs, oracle_obs_encoder_gpu_rescale_release_step,
    oracle_obs_encoder_gpu_rescale_target, oracle_obs_encoder_gpu_rescale_wanted,
};
use obs_output::encoder_gpu_rescale::{
    self as core, OBS_SCALE_DISABLE, Output, Release, Request, VIDEO_CS_DEFAULT, VIDEO_FORMAT_NONE,
    VIDEO_RANGE_DEFAULT,
};
use obs_output::ffi::encoder_gpu_rescale::{
    OBS_ENCODER_GPU_RESCALE_RELEASE_DROP, OBS_ENCODER_GPU_RESCALE_RELEASE_SKIP,
    OBS_ENCODER_GPU_RESCALE_RELEASE_STOP, obs_encoder_gpu_rescale_matches,
    obs_encoder_gpu_rescale_output, obs_encoder_gpu_rescale_release_refs,
    obs_encoder_gpu_rescale_release_step, obs_encoder_gpu_rescale_request,
    obs_encoder_gpu_rescale_target, obs_encoder_gpu_rescale_wanted,
};
use proptest::prelude::*;

const GRID_ENUMS: [i32; 8] = [-2, -1, 0, 1, 2, 3, i32::MAX, i32::MIN];
const GRID_SIZES: [u32; 5] = [0, 1, 1080, 0x8000_0000, u32::MAX];

fn c_request(r: &Request) -> obs_encoder_gpu_rescale_request {
    obs_encoder_gpu_rescale_request {
        scale_type: r.scale_type,
        scaled_width: r.scaled_width,
        scaled_height: r.scaled_height,
        format: r.format,
        colorspace: r.colorspace,
        range: r.range,
    }
}

fn c_output(o: &Output) -> obs_encoder_gpu_rescale_output {
    obs_encoder_gpu_rescale_output {
        width: o.width,
        height: o.height,
        format: o.format,
        colorspace: o.colorspace,
        range: o.range,
    }
}

fn from_shim(o: obs_encoder_gpu_rescale_output) -> Output {
    Output {
        width: o.width,
        height: o.height,
        format: o.format,
        colorspace: o.colorspace,
        range: o.range,
    }
}

fn from_oracle(o: OracleOutput) -> Output {
    Output {
        width: o.width,
        height: o.height,
        format: o.format,
        colorspace: o.colorspace,
        range: o.range,
    }
}

fn release_value(r: Release) -> u32 {
    match r {
        Release::Skip => OBS_ENCODER_GPU_RESCALE_RELEASE_SKIP,
        Release::Stop => OBS_ENCODER_GPU_RESCALE_RELEASE_STOP,
        Release::Drop => OBS_ENCODER_GPU_RESCALE_RELEASE_DROP,
    }
}

fn check_wanted(has_media: bool, r: &Request) {
    let cr = c_request(r);
    let ptr: *const obs_encoder_gpu_rescale_request = &cr;
    // SAFETY: a readable request on the stack.
    let shim = unsafe { obs_encoder_gpu_rescale_wanted(has_media, ptr) };
    // SAFETY: as above; the C sees the same layout (layout_matches_c).
    let c = unsafe { oracle_obs_encoder_gpu_rescale_wanted(has_media, ptr.cast::<c_void>()) };
    assert_eq!(shim, c, "wanted(media {has_media}, {r:?})");
    assert_eq!(
        core::wanted(has_media, r),
        c,
        "core wanted(media {has_media}, {r:?})"
    );
}

fn check_target(r: &Request, video: &Output) {
    let (cr, cv) = (c_request(r), c_output(video));
    let (rp, vp): (
        *const obs_encoder_gpu_rescale_request,
        *const obs_encoder_gpu_rescale_output,
    ) = (&cr, &cv);
    // SAFETY: readable structs on the stack.
    let shim = from_shim(unsafe { obs_encoder_gpu_rescale_target(rp, vp) });
    // SAFETY: as above.
    let c = from_oracle(unsafe {
        oracle_obs_encoder_gpu_rescale_target(rp.cast::<c_void>(), vp.cast::<c_void>())
    });
    assert_eq!(shim, c, "target({r:?}, {video:?})");
    assert_eq!(core::target(r, video), c, "core target({r:?}, {video:?})");
}

fn check_matches(
    same_view: bool,
    mix_scale_type: i32,
    mix: &Output,
    scale_type: i32,
    target: &Output,
) {
    let (cm, ct) = (c_output(mix), c_output(target));
    let (mp, tp): (
        *const obs_encoder_gpu_rescale_output,
        *const obs_encoder_gpu_rescale_output,
    ) = (&cm, &ct);
    // SAFETY: readable outputs on the stack.
    let shim =
        unsafe { obs_encoder_gpu_rescale_matches(same_view, mix_scale_type, mp, scale_type, tp) };
    // SAFETY: as above.
    let c = unsafe {
        oracle_obs_encoder_gpu_rescale_matches(
            same_view,
            mix_scale_type,
            mp.cast::<c_void>(),
            scale_type,
            tp.cast::<c_void>(),
        )
    };
    let what = format!("matches({same_view}, {mix_scale_type}, {mix:?}, {scale_type}, {target:?})");
    assert_eq!(shim, c, "{what}");
    assert_eq!(
        core::matches(same_view, mix_scale_type, mix, scale_type, target),
        c,
        "core {what}"
    );
}

/// The C's `long` as `i64` (`long` is 32 bits on Windows, where this widens).
#[allow(clippy::useless_conversion)]
fn widen(v: c_long) -> i64 {
    i64::from(v)
}

fn check_refs(refs: c_long) {
    let (mut shim_last, mut c_last) = (false, true);
    // SAFETY: writable bools on the stack.
    let shim = unsafe { obs_encoder_gpu_rescale_release_refs(refs, &raw mut shim_last) };
    // SAFETY: as above.
    let c = unsafe { oracle_obs_encoder_gpu_rescale_release_refs(refs, &raw mut c_last) };
    assert_eq!((shim, shim_last), (c, c_last), "release_refs({refs})");
    let (after, last) = core::release_refs(widen(refs));
    assert_eq!(
        (after, last),
        (widen(c), c_last),
        "core release_refs({refs})"
    );
}

fn grid_requests() -> impl Iterator<Item = Request> {
    GRID_ENUMS.into_iter().flat_map(|scale_type| {
        GRID_SIZES.into_iter().flat_map(move |scaled_width| {
            GRID_SIZES.into_iter().flat_map(move |scaled_height| {
                GRID_ENUMS.into_iter().flat_map(move |format| {
                    [0, 1, -1, i32::MIN]
                        .into_iter()
                        .flat_map(move |colorspace| {
                            [0, 2, -1, i32::MAX].into_iter().map(move |range| Request {
                                scale_type,
                                scaled_width,
                                scaled_height,
                                format,
                                colorspace,
                                range,
                            })
                        })
                })
            })
        })
    })
}

#[test]
fn layout_matches_c() {
    let rust = [
        size_of::<obs_encoder_gpu_rescale_request>(),
        align_of::<obs_encoder_gpu_rescale_request>(),
        offset_of!(obs_encoder_gpu_rescale_request, scale_type),
        offset_of!(obs_encoder_gpu_rescale_request, scaled_width),
        offset_of!(obs_encoder_gpu_rescale_request, scaled_height),
        offset_of!(obs_encoder_gpu_rescale_request, format),
        offset_of!(obs_encoder_gpu_rescale_request, colorspace),
        offset_of!(obs_encoder_gpu_rescale_request, range),
        size_of::<obs_encoder_gpu_rescale_output>(),
        align_of::<obs_encoder_gpu_rescale_output>(),
        offset_of!(obs_encoder_gpu_rescale_output, width),
        offset_of!(obs_encoder_gpu_rescale_output, height),
        offset_of!(obs_encoder_gpu_rescale_output, format),
        offset_of!(obs_encoder_gpu_rescale_output, colorspace),
        offset_of!(obs_encoder_gpu_rescale_output, range),
        size_of::<u32>(),
    ];
    for (i, (name, rust)) in LAYOUT_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a plain value lookup.
        let c = unsafe { oracle_encoder_gpu_rescale_layout(i) };
        assert_eq!(rust, c, "{name}");
    }
    // SAFETY: as above.
    assert_eq!(
        unsafe { oracle_encoder_gpu_rescale_layout(LAYOUT_NAMES.len()) },
        usize::MAX
    );
    // The oracle's mirror of the returned struct has the same layout.
    assert_eq!(
        size_of::<OracleOutput>(),
        size_of::<obs_encoder_gpu_rescale_output>()
    );
    assert_eq!(
        align_of::<OracleOutput>(),
        align_of::<obs_encoder_gpu_rescale_output>()
    );
    assert_eq!(size_of::<c_int>(), size_of::<i32>());
}

#[test]
fn constants_match_c() {
    let rust: [i64; 7] = [
        i64::from(OBS_SCALE_DISABLE),
        i64::from(VIDEO_FORMAT_NONE),
        i64::from(VIDEO_CS_DEFAULT),
        i64::from(VIDEO_RANGE_DEFAULT),
        i64::from(OBS_ENCODER_GPU_RESCALE_RELEASE_SKIP),
        i64::from(OBS_ENCODER_GPU_RESCALE_RELEASE_STOP),
        i64::from(OBS_ENCODER_GPU_RESCALE_RELEASE_DROP),
    ];
    for (i, (name, rust)) in CONSTANT_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a plain value lookup.
        let c = unsafe { oracle_encoder_gpu_rescale_constant(i) };
        assert_eq!(rust, c, "{name}");
    }
    // SAFETY: as above.
    assert_eq!(
        unsafe { oracle_encoder_gpu_rescale_constant(CONSTANT_NAMES.len()) },
        -1
    );
}

#[test]
fn wanted_and_target_grid() {
    let videos = [
        Output {
            width: 1920,
            height: 1080,
            format: 2,
            colorspace: 2,
            range: 1,
        },
        Output::default(),
        Output {
            width: u32::MAX,
            height: 1,
            format: -1,
            colorspace: i32::MAX,
            range: i32::MIN,
        },
    ];
    let mut cases = 0_u32;
    for r in grid_requests() {
        check_wanted(false, &r);
        check_wanted(true, &r);
        for v in &videos {
            check_target(&r, v);
        }
        cases = cases.saturating_add(1);
    }
    assert_eq!(cases, 25_600);
}

#[test]
fn matches_each_condition_and_pairs() {
    let targets = [
        Output {
            width: 1280,
            height: 720,
            format: 2,
            colorspace: 2,
            range: 1,
        },
        Output::default(),
        Output {
            width: u32::MAX,
            height: u32::MAX,
            format: i32::MAX,
            colorspace: i32::MIN,
            range: -1,
        },
    ];
    let change = |view: bool, scale: i32, o: Output, k: u32| -> (bool, i32, Output) {
        let mut m = o;
        let (mut view, mut scale) = (view, scale);
        match k {
            0 => view = false,
            1 => scale = scale.wrapping_add(1),
            2 => m.width = m.width.wrapping_add(1),
            3 => m.height = m.height.wrapping_add(1),
            4 => m.format = m.format.wrapping_add(1),
            5 => m.colorspace = m.colorspace.wrapping_add(1),
            6 => m.range = m.range.wrapping_add(1),
            7 => scale = OBS_SCALE_DISABLE,
            _ => {}
        }
        (view, scale, m)
    };
    for t in targets {
        for scale_type in [2, -1, i32::MAX] {
            check_matches(true, scale_type, &t, scale_type, &t);
            for a in 0..8 {
                let (view, scale, m) = change(true, scale_type, t, a);
                check_matches(view, scale, &m, scale_type, &t);
                for b in 0..8 {
                    let (view_b, scale_b, mb) = change(view, scale, m, b);
                    check_matches(view_b, scale_b, &mb, scale_type, &t);
                }
            }
        }
    }
}

#[test]
fn release_every_input_and_edges() {
    for is_media in [false, true] {
        for encoder_only in [false, true] {
            let shim = obs_encoder_gpu_rescale_release_step(is_media, encoder_only);
            // SAFETY: plain values.
            let c = unsafe { oracle_obs_encoder_gpu_rescale_release_step(is_media, encoder_only) };
            assert_eq!(shim, c, "release_step({is_media}, {encoder_only})");
            assert_eq!(release_value(core::release_step(is_media, encoder_only)), c);
        }
    }
    for refs in [
        c_long::MIN.wrapping_add(1),
        -2,
        -1,
        0,
        1,
        2,
        3,
        1000,
        c_long::MAX,
    ] {
        check_refs(refs);
    }
}

fn arb_enum() -> impl Strategy<Value = i32> {
    prop_oneof![Just(0), 0..8_i32, any::<i32>()]
}

fn arb_size() -> impl Strategy<Value = u32> {
    prop_oneof![Just(0_u32), 1..4096_u32, any::<u32>()]
}

fn arb_request() -> impl Strategy<Value = Request> {
    (
        arb_enum(),
        arb_size(),
        arb_size(),
        arb_enum(),
        arb_enum(),
        arb_enum(),
    )
        .prop_map(
            |(scale_type, scaled_width, scaled_height, format, colorspace, range)| Request {
                scale_type,
                scaled_width,
                scaled_height,
                format,
                colorspace,
                range,
            },
        )
}

fn arb_output() -> impl Strategy<Value = Output> {
    (arb_size(), arb_size(), arb_enum(), arb_enum(), arb_enum()).prop_map(
        |(width, height, format, colorspace, range)| Output {
            width,
            height,
            format,
            colorspace,
            range,
        },
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(4096)))]

    #[test]
    fn wanted_and_target_proptest(r in arb_request(), has_media in any::<bool>(), video in arb_output()) {
        check_wanted(has_media, &r);
        check_target(&r, &video);
    }

    #[test]
    fn matches_proptest(
        same_view in any::<bool>(),
        mix_scale in arb_enum(),
        scale in arb_enum(),
        target in arb_output(),
        mix in arb_output(),
        equal in any::<bool>(),
    ) {
        let mix = if equal { target } else { mix };
        let mix_scale = if equal { scale } else { mix_scale };
        check_matches(same_view, mix_scale, &mix, scale, &target);
    }

    #[test]
    fn release_refs_proptest(refs in prop_oneof![-4..8_i64, any::<i64>()]) {
        let refs = c_long::try_from(refs).unwrap_or(1);
        prop_assume!(refs != c_long::MIN);
        check_refs(refs);
    }
}

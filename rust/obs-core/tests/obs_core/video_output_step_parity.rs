//! Tier 2 layout and Tier 3 parity: the shims of
//! `obs_core::ffi::video_output_step` (and the safe core under them)
//! against the original `libobs/obs-video-output-step.c.h`, compiled as
//! `oracle_obs_video_step_*` (obs-c-oracle, oracle/video_output_step.c).
//!
//! Every call runs the shim, the C and the safe core, and compares the
//! results exactly: each struct field by field (the `size_t` channel count
//! and the `int` ring values included), the queue action as its C value,
//! the bools as bools.
//!
//! Runs: the constants and the layout of the six structs; exhaustive over
//! every flag of every function, with the activation counters at the
//! `long` and `int` edges, the channel counts at 0 to 10 and the `size_t`
//! edges, the queue sizes at 0, 1, one frame and the `size_t` edges against
//! counts from -3 to 5 and the `int` edges, the count left and the ring over
//! every `int` from -200,000 to 200,000 and the `int` edges; and proptests
//! over full-range counters, channel counts, sizes, counts and ring values.
//!
//! The C is undefined where its `int` arithmetic overflows:
//! `obs_video_step_queue_count(INT_MIN)`, and `obs_video_step_ring()` of
//! `INT_MIN` (prev) and `INT_MAX` (next). Those three inputs are not given
//! to the C; there the shim is compared with the core, which wraps. That is
//! the one intentional difference, unreachable (libobs passes counts of 1
//! and more, and `cur_texture` is only ever 0 or set by the ring).
//! `test_video_output_step_steps` runs the C test against the C; the
//! real-libobs video and encoder tests run against the hybrid libobs, where
//! these shims replace the C.

use core::ffi::{c_int, c_long, c_void};
use std::mem::{align_of, offset_of, size_of};

use obs_c_oracle::video_output_step::{
    CONSTANT_NAMES, LAYOUT_NAMES, OracleActivation, OracleRender, OracleRenderTargets, OracleRing,
    OracleStage, oracle_obs_video_step_activation, oracle_obs_video_step_download,
    oracle_obs_video_step_output, oracle_obs_video_step_queue, oracle_obs_video_step_queue_copy_uv,
    oracle_obs_video_step_queue_count, oracle_obs_video_step_render,
    oracle_obs_video_step_render_targets, oracle_obs_video_step_ring, oracle_obs_video_step_stage,
    oracle_video_output_step_constant, oracle_video_output_step_layout,
};
use obs_core::ffi::video_output_step::{
    OBS_VIDEO_STEP_QUEUE_COPY, OBS_VIDEO_STEP_QUEUE_DUPLICATE, OBS_VIDEO_STEP_QUEUE_SWAP,
    obs_video_step_activation, obs_video_step_activation_input, obs_video_step_download,
    obs_video_step_output, obs_video_step_queue, obs_video_step_queue_action,
    obs_video_step_queue_copy_uv, obs_video_step_queue_count, obs_video_step_render,
    obs_video_step_render_targets, obs_video_step_ring, obs_video_step_stage, queue_step_to_c,
};
use obs_core::video_output_step::{self as step, CHANNELS, TEXTURES};
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// one call on all three sides

fn activation_c(a: OracleActivation) -> obs_video_step_activation {
    obs_video_step_activation {
        clear_base: a.clear_base,
        clear_raw: a.clear_raw,
        clear_gpu: a.clear_gpu,
        raw_active: a.raw_active,
        gpu_active: a.gpu_active,
        active: a.active,
    }
}

fn check_activation(raw_was: bool, gpu_was: bool, was: bool, raw: c_long, gpu: c_long) {
    let input = obs_video_step_activation_input {
        raw_was_active: raw_was,
        gpu_was_active: gpu_was,
        was_active: was,
        raw_active: raw,
        gpu_encoder_active: gpu,
    };
    // SAFETY: a local input, read during the call.
    let rust = unsafe { obs_video_step_activation(&raw const input) };
    // SAFETY: the same input, laid out as the C's (checked below).
    let c = activation_c(unsafe {
        oracle_obs_video_step_activation((&raw const input).cast::<c_void>())
    });
    assert_eq!(rust, c, "activation {input:?}");
    let core = obs_video_step_activation::from(step::activation(&input.into()));
    assert_eq!(core, c, "core activation {input:?}");
}

fn stage_c(s: OracleStage) -> obs_video_step_stage {
    obs_video_step_stage {
        staged: s.staged,
        from_output: s.from_output,
        channels: s.channels,
    }
}

fn check_stage(conversion: bool, converted: bool, channels: usize) {
    let rust = obs_video_step_stage(conversion, converted, channels);
    // SAFETY: a plain value function.
    let c = stage_c(unsafe { oracle_obs_video_step_stage(conversion, converted, channels) });
    assert_eq!(rust, c, "stage({conversion}, {converted}, {channels})");
    let core = obs_video_step_stage::from(step::stage(conversion, converted, channels));
    assert_eq!(core, c, "core stage({conversion}, {converted}, {channels})");
}

fn check_queue(avail: usize, queue: usize, count: c_int, raw: bool) -> obs_video_step_queue_action {
    let rust = obs_video_step_queue(avail, queue, count, raw);
    // SAFETY: a plain value function.
    let c = unsafe { oracle_obs_video_step_queue(avail, queue, count, raw) };
    assert_eq!(rust, c, "queue({avail}, {queue}, {count}, {raw})");
    let core = queue_step_to_c(step::queue_step(avail, queue, count, raw));
    assert_eq!(core, c, "core queue({avail}, {queue}, {count}, {raw})");
    c
}

/// The C's `--count` is undefined only at `INT_MIN`.
fn check_queue_count(count: c_int) {
    let rust = obs_video_step_queue_count(count);
    assert_eq!(step::queue_count(count), rust, "core queue_count({count})");
    if count != c_int::MIN {
        // SAFETY: a plain value function, defined for this count.
        let c = unsafe { oracle_obs_video_step_queue_count(count) };
        assert_eq!(rust, c, "queue_count({count})");
    }
}

fn ring_c(r: OracleRing) -> obs_video_step_ring {
    obs_video_step_ring {
        prev: r.prev,
        next: r.next,
    }
}

/// The C's ring is undefined only at `INT_MIN` and `INT_MAX`.
fn check_ring(cur: c_int) {
    let rust = obs_video_step_ring(cur);
    assert_eq!(
        obs_video_step_ring::from(step::ring(cur)),
        rust,
        "core ring({cur})"
    );
    if cur != c_int::MIN && cur != c_int::MAX {
        // SAFETY: a plain value function, defined for this value.
        let c = ring_c(unsafe { oracle_obs_video_step_ring(cur) });
        assert_eq!(rust, c, "ring({cur})");
    }
}

fn check_flags(raw: bool, gpu: bool, conversion: bool, ready: bool) {
    let targets = obs_video_step_render_targets(raw, gpu);
    // SAFETY: plain value functions, for the rest of this block too.
    let c: OracleRenderTargets = unsafe { oracle_obs_video_step_render_targets(raw, gpu) };
    assert_eq!(
        (targets.outputs, targets.encode),
        (c.outputs, c.encode),
        "render_targets({raw}, {gpu})"
    );
    assert_eq!(
        obs_video_step_render_targets::from(step::render_targets(raw, gpu)),
        targets
    );

    let render = obs_video_step_render(raw, gpu, conversion);
    // SAFETY: as above.
    let c: OracleRender = unsafe { oracle_obs_video_step_render(raw, gpu, conversion) };
    assert_eq!(
        (render.convert, render.encode, render.stage),
        (c.convert, c.encode, c.stage),
        "render({raw}, {gpu}, {conversion})"
    );
    assert_eq!(
        obs_video_step_render::from(step::render_steps(raw, gpu, conversion)),
        render
    );

    // SAFETY: as above.
    let (download, output, uv) = unsafe {
        (
            oracle_obs_video_step_download(raw),
            oracle_obs_video_step_output(raw, ready),
            oracle_obs_video_step_queue_copy_uv(gpu),
        )
    };
    assert_eq!(obs_video_step_download(raw), download, "download({raw})");
    assert_eq!(step::download(raw), download);
    assert_eq!(
        obs_video_step_output(raw, ready),
        output,
        "output({raw}, {ready})"
    );
    assert_eq!(step::output(raw, ready), output);
    assert_eq!(obs_video_step_queue_copy_uv(gpu), uv, "copy_uv({gpu})");
    assert_eq!(step::copy_uv(gpu), uv);
}

// ---------------------------------------------------------------------------
// constants and layout

fn constant(name: &str) -> i64 {
    let which = CONSTANT_NAMES
        .iter()
        .position(|n| *n == name)
        .expect("constant name");
    // SAFETY: an index read.
    unsafe { oracle_video_output_step_constant(which) }
}

fn layout(name: &str) -> usize {
    let which = LAYOUT_NAMES
        .iter()
        .position(|n| *n == name)
        .expect("layout name");
    // SAFETY: an index read.
    unsafe { oracle_video_output_step_layout(which) }
}

#[test]
fn parity_constants() {
    assert_eq!(constant("OBS_VIDEO_STEP_TEXTURES"), i64::from(TEXTURES));
    assert_eq!(
        constant("OBS_VIDEO_STEP_CHANNELS"),
        i64::try_from(CHANNELS).expect("fits")
    );
    assert_eq!(
        constant("OBS_VIDEO_STEP_QUEUE_DUPLICATE"),
        i64::from(OBS_VIDEO_STEP_QUEUE_DUPLICATE)
    );
    assert_eq!(
        constant("OBS_VIDEO_STEP_QUEUE_COPY"),
        i64::from(OBS_VIDEO_STEP_QUEUE_COPY)
    );
    assert_eq!(
        constant("OBS_VIDEO_STEP_QUEUE_SWAP"),
        i64::from(OBS_VIDEO_STEP_QUEUE_SWAP)
    );
    // SAFETY: an index read, out of range.
    assert_eq!(
        unsafe { oracle_video_output_step_constant(CONSTANT_NAMES.len()) },
        -1
    );
}

#[test]
fn parity_layout() {
    macro_rules! check {
        ($ty:ty, $c:literal, [$($field:ident),*]) => {{
            assert_eq!(layout(concat!("sizeof(struct ", $c, ")")), size_of::<$ty>(), "sizeof {}", $c);
            assert_eq!(layout(concat!("alignof(struct ", $c, ")")), align_of::<$ty>(), "alignof {}", $c);
            $(
                assert_eq!(
                    layout(concat!("offsetof(struct ", $c, ", ", stringify!($field), ")")),
                    offset_of!($ty, $field),
                    "offsetof {}.{}", $c, stringify!($field)
                );
            )*
        }};
    }
    check!(
        obs_video_step_activation_input,
        "obs_video_step_activation_input",
        [
            raw_was_active,
            gpu_was_active,
            was_active,
            raw_active,
            gpu_encoder_active
        ]
    );
    check!(
        obs_video_step_activation,
        "obs_video_step_activation",
        [
            clear_base, clear_raw, clear_gpu, raw_active, gpu_active, active
        ]
    );
    check!(
        obs_video_step_stage,
        "obs_video_step_stage",
        [staged, from_output, channels]
    );
    check!(
        obs_video_step_render_targets,
        "obs_video_step_render_targets",
        [outputs, encode]
    );
    check!(
        obs_video_step_render,
        "obs_video_step_render",
        [convert, encode, stage]
    );
    check!(obs_video_step_ring, "obs_video_step_ring", [prev, next]);
    assert_eq!(
        layout("sizeof(enum obs_video_step_queue_action)"),
        size_of::<obs_video_step_queue_action>()
    );
    assert_eq!(layout("sizeof(long)"), size_of::<c_long>());
    // The oracle's mirrors are the shims' layout too.
    assert_eq!(size_of::<OracleStage>(), size_of::<obs_video_step_stage>());
    assert_eq!(size_of::<OracleRing>(), size_of::<obs_video_step_ring>());
    // SAFETY: an index read, out of range.
    assert_eq!(
        unsafe { oracle_video_output_step_layout(LAYOUT_NAMES.len()) },
        usize::MAX
    );
}

// ---------------------------------------------------------------------------
// exhaustive

/// The `long` edges, and the `int` ones (the same on a 32-bit `long`).
fn longs() -> Vec<c_long> {
    vec![
        c_long::MIN,
        c_long::MIN + 1,
        c_long::from(i32::MIN),
        -3,
        -2,
        -1,
        0,
        1,
        2,
        3,
        c_long::from(i32::MAX) - 1,
        c_long::from(i32::MAX),
        c_long::MAX - 1,
        c_long::MAX,
    ]
}

const COUNTS: [c_int; 13] = [
    c_int::MIN,
    c_int::MIN + 1,
    -3,
    -2,
    -1,
    0,
    1,
    2,
    3,
    4,
    5,
    c_int::MAX - 1,
    c_int::MAX,
];

const SIZES: [usize; 10] = [
    0,
    1,
    47,
    48,
    49,
    96,
    0x7fff_ffff,
    usize::MAX / 2,
    usize::MAX - 1,
    usize::MAX,
];

#[test]
fn parity_activation_exhaustive() {
    let longs = longs();
    let mut calls = 0usize;
    for was in 0..8u32 {
        for &raw in &longs {
            for &gpu in &longs {
                check_activation(was & 1 != 0, was & 2 != 0, was & 4 != 0, raw, gpu);
                calls += 1;
            }
        }
    }
    assert_eq!(calls, 8 * longs.len() * longs.len());
}

#[test]
fn parity_stage_exhaustive() {
    let mut channels: Vec<usize> = (0..=10).collect();
    channels.extend([
        0x7fff_ffff,
        0xffff_ffff,
        usize::MAX / 2,
        usize::MAX - 1,
        usize::MAX,
    ]);
    for conversion in [false, true] {
        for converted in [false, true] {
            for &n in &channels {
                check_stage(conversion, converted, n);
            }
        }
    }
}

#[test]
fn parity_queue_exhaustive() {
    let mut seen = [0usize; 3];
    for &avail in &SIZES {
        for &queue in &SIZES {
            for &count in &COUNTS {
                for raw in [false, true] {
                    let action = check_queue(avail, queue, count, raw);
                    let slot = usize::try_from(action).expect("small");
                    *seen.get_mut(slot).expect("an action") += 1;
                }
            }
        }
    }
    // Every action is reached.
    assert!(seen.iter().all(|&n| n > 0), "{seen:?}");
}

#[test]
fn parity_queue_count_and_ring_range() {
    for v in -200_000..=200_000 {
        check_queue_count(v);
        check_ring(v);
    }
    for &v in &COUNTS {
        check_queue_count(v);
        check_ring(v);
    }
}

#[test]
fn parity_flags_exhaustive() {
    for bits in 0..16u32 {
        check_flags(bits & 1 != 0, bits & 2 != 0, bits & 4 != 0, bits & 8 != 0);
    }
}

/// The one intentional difference: where the C overflows, the port wraps.
#[test]
fn parity_wraps_where_the_c_is_undefined() {
    assert_eq!(obs_video_step_queue_count(c_int::MIN), c_int::MAX);
    assert_eq!(
        obs_video_step_ring(c_int::MIN),
        obs_video_step_ring {
            prev: c_int::MAX,
            next: c_int::MIN + 1,
        }
    );
    assert_eq!(
        obs_video_step_ring(c_int::MAX),
        obs_video_step_ring {
            prev: c_int::MAX - 1,
            next: c_int::MIN,
        }
    );
}

// ---------------------------------------------------------------------------
// proptests

proptest! {
    #![proptest_config(ProptestConfig::with_cases(8192))]

    #[test]
    fn parity_activation_full_range(raw_was: bool, gpu_was: bool, was: bool, raw: c_long, gpu: c_long) {
        check_activation(raw_was, gpu_was, was, raw, gpu);
    }

    #[test]
    fn parity_stage_full_range(conversion: bool, converted: bool, channels: usize) {
        check_stage(conversion, converted, channels);
    }

    #[test]
    fn parity_queue_full_range(
        avail in prop_oneof![Just(0usize), Just(48usize), any::<usize>()],
        queue in prop_oneof![Just(0usize), Just(48usize), any::<usize>()],
        count in prop_oneof![-2..=4, any::<c_int>()],
        raw: bool,
    ) {
        check_queue(avail, queue, count, raw);
    }

    #[test]
    fn parity_queue_count_full_range(count: c_int) {
        check_queue_count(count);
    }

    #[test]
    fn parity_ring_full_range(cur in prop_oneof![-4..=4, any::<c_int>()]) {
        check_ring(cur);
    }
}

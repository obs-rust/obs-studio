//! Tier 1: `obs_core::video_output_step`, the per-frame video output step
//! decisions.
//!
//! The `steps_*` cases port `test/cmocka/test_video_output_step_steps.c`
//! 1:1: its 19 cases under the same names, in the same order, with the same
//! values and expectations, the "characterized, not endorsed" ones
//! included. The C calls the hidden `obs_video_step_*` functions; here the
//! safe core is called directly. The C's `struct obs_video_step_stage`
//! (staged, from output, channels) is [`Stage`] here, read back through its
//! three accessors.
//!
//! The `edge_*` cases are new: proptests over full-range counters, sizes,
//! counts and ring values against models written from the C's expressions
//! (with the C's undefined overflows taken as wrapping), and a walk of the
//! ring over many frames.

use core::ffi::{c_int, c_long};

use obs_core::video_output_step::{
    Activation, ActivationInput, CHANNELS, QueueStep, RenderSteps, RenderTargets, Ring, Stage,
    TEXTURES, activation, copy_uv, download, output, queue_count, queue_step, render_steps,
    render_targets, ring, stage,
};
use proptest::prelude::*;

/// `sizeof(struct obs_tex_frame)` on LP64; any non-zero size.
const FRAME: usize = 48;

// ---------------------------------------------------------------------------
// helpers

#[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
fn check_activation(
    what: &str,
    got: Activation,
    base: bool,
    raw: bool,
    gpu: bool,
    raw_active: bool,
    gpu_active: bool,
    active: bool,
) {
    let want = Activation {
        clear_base: base,
        clear_raw: raw,
        clear_gpu: gpu,
        raw_active,
        gpu_active,
        active,
    };
    assert_eq!(got, want, "{what}");
}

fn check_stage(what: &str, got: Stage, staged: bool, from_output: bool, channels: usize) {
    assert_eq!(
        (got.staged(), got.from_output(), got.channels()),
        (staged, from_output, channels),
        "{what}: {got:?}"
    );
}

fn check_queue(avail: usize, queue: usize, count: c_int, raw: bool, expected: QueueStep) {
    assert_eq!(
        queue_step(avail, queue, count, raw),
        expected,
        "queue avail {avail} queue {queue} count {count} raw {raw}"
    );
}

fn check_ring(cur: c_int, prev: c_int, next: c_int) {
    assert_eq!(ring(cur), Ring { prev, next }, "ring {cur}");
}

fn input(raw_was: bool, gpu_was: bool, was: bool, raw: c_long, gpu: c_long) -> ActivationInput {
    ActivationInput {
        raw_was_active: raw_was,
        gpu_was_active: gpu_was,
        was_active: was,
        raw_active: raw,
        gpu_encoder_active: gpu,
    }
}

// ---------------------------------------------------------------------------
// update_active_state()

#[test]
fn steps_constants() {
    assert_eq!(TEXTURES, 2);
    assert_eq!(CHANNELS, 3);
}

#[test]
fn steps_activation_every_combination() {
    let values: [c_long; 8] = [c_long::MIN, -2, -1, 0, 1, 2, 3, c_long::MAX];
    for was in 0..8u32 {
        for &r in &values {
            for &g in &values {
                let i = input(was & 1 != 0, was & 2 != 0, was & 4 != 0, r, g);
                let raw = r > 0;
                let gpu = g > 0;
                let any = raw || gpu;
                check_activation(
                    &format!("was {was:03b} raw {r} gpu {g}"),
                    activation(&i),
                    !i.was_active && any,
                    !i.raw_was_active && raw,
                    !i.gpu_was_active && gpu,
                    raw,
                    gpu,
                    any,
                );
            }
        }
    }
}

#[test]
fn steps_activation_edges() {
    let mut i = ActivationInput {
        raw_active: 1,
        ..ActivationInput::default()
    };
    check_activation(
        "raw starts",
        activation(&i),
        true,
        true,
        false,
        true,
        false,
        true,
    );

    i = input(true, false, true, 1, 0);
    check_activation(
        "raw stays",
        activation(&i),
        false,
        false,
        false,
        true,
        false,
        true,
    );

    i.gpu_encoder_active = 1;
    check_activation(
        "gpu starts with raw on",
        activation(&i),
        false,
        false,
        true,
        true,
        true,
        true,
    );

    i = input(false, false, false, 0, 2);
    check_activation(
        "gpu starts alone",
        activation(&i),
        true,
        false,
        true,
        false,
        true,
        true,
    );

    i = input(true, true, true, 0, 0);
    check_activation(
        "both stop",
        activation(&i),
        false,
        false,
        false,
        false,
        false,
        false,
    );

    i = input(false, false, false, -1, c_long::MIN);
    check_activation(
        "negative counters",
        activation(&i),
        false,
        false,
        false,
        false,
        false,
        false,
    );
}

/// Characterized, not endorsed: the was flags are taken as they are.
#[test]
fn steps_activation_inconsistent_was_flags() {
    let i = input(true, false, false, 1, 0);
    check_activation(
        "raw was, any was not",
        activation(&i),
        true,
        false,
        false,
        true,
        false,
        true,
    );

    let i = input(false, false, true, 0, 1);
    check_activation(
        "any was, gpu was not",
        activation(&i),
        false,
        false,
        true,
        false,
        true,
        true,
    );
}

// ---------------------------------------------------------------------------
// stage_output_texture()

#[test]
fn steps_stage_without_conversion() {
    for channels in 0..=CHANNELS + 1 {
        for converted in [false, true] {
            check_stage(
                &format!("no conversion channels {channels} converted {converted}"),
                stage(false, converted, channels),
                true,
                true,
                1,
            );
        }
    }
    check_stage(
        "no conversion SIZE_MAX",
        stage(false, true, usize::MAX),
        true,
        true,
        1,
    );
}

#[test]
fn steps_stage_with_conversion() {
    for channels in 0..=CHANNELS {
        check_stage(
            &format!("converted channels {channels}"),
            stage(true, true, channels),
            true,
            false,
            channels,
        );
    }
}

#[test]
fn steps_stage_not_converted() {
    for channels in 0..=CHANNELS {
        check_stage(
            &format!("unconverted channels {channels}"),
            stage(true, false, channels),
            false,
            false,
            0,
        );
    }
}

/// Characterized, not endorsed: a count past `NUM_CHANNELS` passes through.
#[test]
fn steps_stage_count_past_channels() {
    check_stage("converted 4", stage(true, true, 4), true, false, 4);
    check_stage(
        "converted SIZE_MAX",
        stage(true, true, usize::MAX),
        true,
        false,
        usize::MAX,
    );
}

// ---------------------------------------------------------------------------
// queue_frame()

#[test]
fn steps_queue_no_free_frame() {
    let counts = [c_int::MIN, -1, 0, 1, 2, 3, c_int::MAX];
    for &count in &counts {
        for raw in [false, true] {
            check_queue(0, 0, count, raw, QueueStep::Duplicate);
            check_queue(0, FRAME, count, raw, QueueStep::Duplicate);
            check_queue(0, 3 * FRAME, count, raw, QueueStep::Duplicate);
        }
    }
}

#[test]
fn steps_queue_free_and_queued() {
    for raw in [false, true] {
        let single = if raw {
            QueueStep::Copy
        } else {
            QueueStep::Swap
        };
        check_queue(FRAME, FRAME, 1, raw, single);
        check_queue(FRAME, FRAME, 2, raw, QueueStep::Duplicate);
        check_queue(FRAME, FRAME, 3, raw, QueueStep::Duplicate);
        check_queue(2 * FRAME, 3 * FRAME, 1, raw, single);
        check_queue(2 * FRAME, 3 * FRAME, 3, raw, QueueStep::Duplicate);
    }
}

#[test]
fn steps_queue_free_and_empty() {
    check_queue(FRAME, 0, 1, false, QueueStep::Swap);
    check_queue(FRAME, 0, 1, true, QueueStep::Copy);
    for count in 2..=3 {
        check_queue(FRAME, 0, count, false, QueueStep::Copy);
        check_queue(FRAME, 0, count, true, QueueStep::Copy);
    }
}

/// Characterized, not endorsed: counts of 0 and below act as 1, and sizes
/// are only tested for 0.
#[test]
fn steps_queue_odd_inputs() {
    check_queue(FRAME, FRAME, 0, false, QueueStep::Swap);
    check_queue(FRAME, FRAME, -1, true, QueueStep::Copy);
    check_queue(FRAME, FRAME, c_int::MIN, false, QueueStep::Swap);
    check_queue(FRAME, FRAME, c_int::MAX, false, QueueStep::Duplicate);
    check_queue(1, 0, 1, false, QueueStep::Swap);
    check_queue(1, 1, 2, false, QueueStep::Duplicate);
    check_queue(usize::MAX, usize::MAX, 2, true, QueueStep::Duplicate);
    check_queue(usize::MAX, 0, c_int::MAX, false, QueueStep::Copy);
}

#[test]
fn steps_queue_copy_uv() {
    assert!(copy_uv(false));
    assert!(!copy_uv(true));
}

#[test]
fn steps_queue_count() {
    for count in 1..=3 {
        let mut left = count;
        let mut frames = 0;
        loop {
            left = queue_count(left);
            frames += 1;
            if left == 0 || frames >= 10 {
                break;
            }
        }
        assert_eq!(frames, count, "count {count}");
    }
    assert_eq!(queue_count(1), 0);
    assert_eq!(queue_count(2), 1);
    assert_eq!(queue_count(3), 2);
    // Characterized, not endorsed.
    assert_eq!(queue_count(0), -1);
    assert_eq!(queue_count(c_int::MAX), c_int::MAX - 1);
}

// ---------------------------------------------------------------------------
// render_video()

#[test]
fn steps_render_targets() {
    for raw in [false, true] {
        for gpu in [false, true] {
            let outputs = raw || gpu;
            assert_eq!(
                render_targets(raw, gpu),
                RenderTargets {
                    outputs,
                    encode: outputs && gpu,
                },
                "targets raw {raw} gpu {gpu}"
            );
        }
    }
    assert_eq!(render_targets(false, false), RenderTargets::default());
}

#[test]
fn steps_render_every_combination() {
    for raw in [false, true] {
        for gpu in [false, true] {
            for conversion in [false, true] {
                assert_eq!(
                    render_steps(raw, gpu, conversion),
                    RenderSteps {
                        convert: conversion,
                        encode: gpu,
                        stage: raw,
                    },
                    "render raw {raw} gpu {gpu} conversion {conversion}"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// output_frame()

#[test]
fn steps_ring_wrap() {
    check_ring(0, TEXTURES - 1, 1);
    check_ring(TEXTURES - 1, TEXTURES - 2, 0);

    let mut cur = 0;
    for frame in 0..2 * TEXTURES {
        let r = ring(cur);
        assert_eq!(
            r.prev,
            (cur + TEXTURES - 1) % TEXTURES,
            "frame {frame}: prev of {cur}"
        );
        cur = r.next;
    }
    assert_eq!(cur, 0);
}

/// Characterized, not endorsed: only exactly `NUM_TEXTURES` wraps.
#[test]
fn steps_ring_outside() {
    check_ring(TEXTURES, TEXTURES - 1, TEXTURES + 1);
    check_ring(-1, -2, 0);
    check_ring(c_int::MAX - 1, c_int::MAX - 2, c_int::MAX);
    check_ring(c_int::MIN + 1, c_int::MIN, c_int::MIN + 2);
}

#[test]
fn steps_download_and_output() {
    assert!(download(true));
    assert!(!download(false));
    assert!(output(true, true));
    assert!(!output(true, false));
    assert!(!output(false, true));
    assert!(!output(false, false));
}

// ---------------------------------------------------------------------------
// edges

/// The ring's two ends, where the C's `cur_texture - 1` and
/// `++cur_texture` overflow (undefined): the port wraps.
#[test]
fn edge_ring_int_ends() {
    check_ring(c_int::MIN, c_int::MAX, c_int::MIN + 1);
    check_ring(c_int::MAX, c_int::MAX - 1, c_int::MIN);
}

/// `--count` at `INT_MIN` overflows in the C (undefined): the port wraps.
#[test]
fn edge_queue_count_int_min() {
    assert_eq!(queue_count(c_int::MIN), c_int::MAX);
}

/// A thousand frames go round the ring: each downloads the texture
/// rendered the frame before.
#[test]
fn edge_ring_walk() {
    let mut cur = 0;
    let mut last = None;
    for _ in 0..1000 {
        let r = ring(cur);
        assert!((0..TEXTURES).contains(&r.next));
        if let Some(rendered) = last {
            assert_eq!(r.prev, rendered);
        }
        last = Some(cur);
        cur = r.next;
    }
}

/// The C's `queue_frame()` decision, written out.
fn queue_model(avail: usize, queue: usize, count: c_int, raw: bool) -> QueueStep {
    let duplicate = avail == 0 || (queue != 0 && count > 1);
    if duplicate {
        QueueStep::Duplicate
    } else if raw || count > 1 {
        QueueStep::Copy
    } else {
        QueueStep::Swap
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    #[test]
    fn edge_activation_full_range(
        raw_was: bool,
        gpu_was: bool,
        was: bool,
        raw: c_long,
        gpu: c_long,
    ) {
        let got = activation(&input(raw_was, gpu_was, was, raw, gpu));
        let any = raw > 0 || gpu > 0;
        prop_assert_eq!(got, Activation {
            clear_base: !was && any,
            clear_raw: !raw_was && raw > 0,
            clear_gpu: !gpu_was && gpu > 0,
            raw_active: raw > 0,
            gpu_active: gpu > 0,
            active: any,
        });
    }

    #[test]
    fn edge_stage_full_range(conversion: bool, converted: bool, channels: usize) {
        let got = stage(conversion, converted, channels);
        let want = if !conversion {
            (true, true, 1)
        } else if converted {
            (true, false, channels)
        } else {
            (false, false, 0)
        };
        prop_assert_eq!((got.staged(), got.from_output(), got.channels()), want);
    }

    #[test]
    fn edge_queue_full_range(
        avail in prop_oneof![Just(0usize), Just(FRAME), any::<usize>()],
        queue in prop_oneof![Just(0usize), Just(FRAME), any::<usize>()],
        count in prop_oneof![-2..=4, any::<c_int>()],
        raw: bool,
    ) {
        prop_assert_eq!(queue_step(avail, queue, count, raw), queue_model(avail, queue, count, raw));
    }

    #[test]
    fn edge_queue_count_full_range(count: c_int) {
        prop_assert_eq!(queue_count(count), count.wrapping_sub(1));
    }

    #[test]
    fn edge_ring_full_range(cur: c_int) {
        let prev = if cur == 0 { TEXTURES - 1 } else { cur.wrapping_sub(1) };
        let next = match cur.wrapping_add(1) {
            TEXTURES => 0,
            n => n,
        };
        prop_assert_eq!(ring(cur), Ring { prev, next });
    }

    #[test]
    fn edge_render_and_frame(raw: bool, gpu: bool, conversion: bool, ready: bool, share: bool) {
        prop_assert_eq!(render_targets(raw, gpu).outputs, raw || gpu);
        prop_assert_eq!(render_targets(raw, gpu).encode, (raw || gpu) && gpu);
        prop_assert_eq!(render_steps(raw, gpu, conversion), RenderSteps { convert: conversion, encode: gpu, stage: raw });
        prop_assert_eq!(download(raw), raw);
        prop_assert_eq!(output(raw, ready), raw && ready);
        prop_assert_eq!(copy_uv(share), !share);
    }
}

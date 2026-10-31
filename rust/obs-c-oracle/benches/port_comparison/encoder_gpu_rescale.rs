//! obs-output: the encoder GPU rescale mix decisions
//! (`libobs/obs-encoder-gpu-rescale.c.h`). `maybe_set_up_gpu_rescale()`
//! runs the gates and the target once per encoder start, and the match once
//! per mix in its search (twice when it creates a mix);
//! `maybe_clear_encoder_core_video_mix()` runs the release step per mix and
//! the reference count once per encoder shutdown. Each side runs 1024
//! encoder starts and shutdowns against four mixes over fixed inputs and
//! folds the results into a checksum both must match.

use super::pair;

use core::ffi::{c_long, c_void};

use criterion::Criterion;
use obs_c_oracle::encoder_gpu_rescale::{
    oracle_obs_encoder_gpu_rescale_matches, oracle_obs_encoder_gpu_rescale_release_refs,
    oracle_obs_encoder_gpu_rescale_release_step, oracle_obs_encoder_gpu_rescale_target,
    oracle_obs_encoder_gpu_rescale_wanted,
};
use obs_output::ffi::encoder_gpu_rescale::{
    obs_encoder_gpu_rescale_matches, obs_encoder_gpu_rescale_output, obs_encoder_gpu_rescale_release_refs,
    obs_encoder_gpu_rescale_release_step, obs_encoder_gpu_rescale_request, obs_encoder_gpu_rescale_target,
    obs_encoder_gpu_rescale_wanted,
};
use std::hint::black_box;

const N: u32 = 1024;

/// The main canvas's video (1920x1080 NV12, 709, partial).
const VIDEO: obs_encoder_gpu_rescale_output = obs_encoder_gpu_rescale_output {
    width: 1920,
    height: 1080,
    format: 2,
    colorspace: 2,
    range: 1,
};

/// The mixes in `obs->video.mixes`: the main mix and three encoder-only
/// ones (720p, 480p, 720p I420), with their scale types.
const MIXES: [(i32, obs_encoder_gpu_rescale_output); 4] = [
    (2, VIDEO),
    (
        2,
        obs_encoder_gpu_rescale_output {
            width: 1280,
            height: 720,
            format: 2,
            colorspace: 2,
            range: 1,
        },
    ),
    (
        3,
        obs_encoder_gpu_rescale_output {
            width: 854,
            height: 480,
            format: 2,
            colorspace: 2,
            range: 1,
        },
    ),
    (
        2,
        obs_encoder_gpu_rescale_output {
            width: 1280,
            height: 720,
            format: 1,
            colorspace: 2,
            range: 1,
        },
    ),
];

/// Encoder `i`'s request: no rescale, 720p, 480p bilinear, or I420 720p.
fn requests() -> Vec<obs_encoder_gpu_rescale_request> {
    (0..N)
        .map(|i| match i % 4 {
            0 => obs_encoder_gpu_rescale_request::default(),
            1 => obs_encoder_gpu_rescale_request {
                scale_type: 2,
                scaled_width: 1280,
                scaled_height: 720,
                ..Default::default()
            },
            2 => obs_encoder_gpu_rescale_request {
                scale_type: 3,
                scaled_width: 854,
                scaled_height: 480,
                ..Default::default()
            },
            _ => obs_encoder_gpu_rescale_request {
                scale_type: 2,
                scaled_width: 1280,
                scaled_height: 720,
                format: 1,
                ..Default::default()
            },
        })
        .collect()
}

fn checksum(acc: u64, wanted: bool, found: usize, t: &obs_encoder_gpu_rescale_output, step: u32, refs: c_long, last: bool) -> u64 {
    acc.wrapping_mul(31)
        .wrapping_add(u64::from(wanted))
        .wrapping_add(found as u64)
        .wrapping_add(u64::from(t.width) ^ u64::from(t.height))
        .wrapping_add(u64::from(t.format.unsigned_abs()))
        .wrapping_add(u64::from(step))
        .wrapping_add(u64::from(refs == 1))
        .wrapping_add(u64::from(last))
}

fn rust_side(reqs: &[obs_encoder_gpu_rescale_request]) -> u64 {
    reqs.iter().fold(0, |acc, r| {
        let r = black_box(*r);
        let video = black_box(VIDEO);
        // SAFETY: live locals.
        let wanted = unsafe { obs_encoder_gpu_rescale_wanted(true, &raw const r) };
        // SAFETY: as above.
        let t = unsafe { obs_encoder_gpu_rescale_target(&raw const r, &raw const video) };
        let mut found = MIXES.len();
        if wanted {
            for (k, (scale, mix)) in MIXES.iter().enumerate() {
                // SAFETY: as above.
                if unsafe { obs_encoder_gpu_rescale_matches(true, *scale, mix, r.scale_type, &raw const t) } {
                    found = k;
                    break;
                }
            }
        }
        let step = obs_encoder_gpu_rescale_release_step(true, found != 0);
        let mut last = false;
        // SAFETY: as above.
        let refs = unsafe { obs_encoder_gpu_rescale_release_refs(2, &raw mut last) };
        checksum(acc, wanted, found, &t, step, refs, last)
    })
}

fn c_side(reqs: &[obs_encoder_gpu_rescale_request]) -> u64 {
    reqs.iter().fold(0, |acc, r| {
        let r = black_box(*r);
        let video = black_box(VIDEO);
        let rp = (&raw const r).cast::<c_void>();
        // SAFETY: live locals with the C's layout (Tier 3 checks it).
        let wanted = unsafe { oracle_obs_encoder_gpu_rescale_wanted(true, rp) };
        // SAFETY: as above.
        let t = unsafe { oracle_obs_encoder_gpu_rescale_target(rp, (&raw const video).cast::<c_void>()) };
        let t = obs_encoder_gpu_rescale_output {
            width: t.width,
            height: t.height,
            format: t.format,
            colorspace: t.colorspace,
            range: t.range,
        };
        let mut found = MIXES.len();
        if wanted {
            for (k, (scale, mix)) in MIXES.iter().enumerate() {
                // SAFETY: as above.
                if unsafe {
                    oracle_obs_encoder_gpu_rescale_matches(
                        true,
                        *scale,
                        (mix as *const obs_encoder_gpu_rescale_output).cast::<c_void>(),
                        r.scale_type,
                        (&raw const t).cast::<c_void>(),
                    )
                } {
                    found = k;
                    break;
                }
            }
        }
        // SAFETY: plain values.
        let step = unsafe { oracle_obs_encoder_gpu_rescale_release_step(true, found != 0) };
        let mut last = false;
        // SAFETY: as above.
        let refs = unsafe { oracle_obs_encoder_gpu_rescale_release_refs(2, &raw mut last) };
        checksum(acc, wanted, found, &t, step, refs, last)
    })
}

pub fn benchmarks(criterion: &mut Criterion) {
    let reqs = requests();
    pair(criterion, "encoder_gpu_rescale/start_and_release/1024", || rust_side(&reqs), || c_side(&reqs));
}

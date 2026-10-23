//! obs-output: the encoder region of interest checks and scaling
//! (`libobs/obs-encoder-roi.c.h`). `obs_encoder_add_roi()` runs
//! `obs_encoder_roi_admit` once per ROI a plugin, script or the frontend
//! adds; `obs_encoder_enum_roi()` runs `obs_encoder_roi_scale_factors`
//! once and `obs_encoder_roi_scale` once per stored ROI each time an
//! encoder rebuilds its ROI map (when `roi_increment` moved). Each side
//! runs the calls for 16 ROIs.

use super::pair;

use core::ffi::c_void;

use criterion::Criterion;
use obs_c_oracle::encoder_roi::{
    oracle_obs_encoder_roi_admit, oracle_obs_encoder_roi_scale,
    oracle_obs_encoder_roi_scale_factors,
};
use obs_output::encoder_roi::{OBS_ENCODER_CAP_ROI, Roi};
use obs_output::ffi::encoder_roi::{
    obs_encoder_roi_admit, obs_encoder_roi_scale, obs_encoder_roi_scale_factors,
};
use std::hint::black_box;

/// ROI `i`: a rectangle of a 1920 x 1080 frame, some too small or upside
/// down, with priorities across [-1, 1] and one past it.
fn rois() -> Vec<Roi> {
    (0..16u32)
        .map(|i| Roi {
            top: 37 * i,
            bottom: 37 * i + 8 + 13 * (i % 5),
            left: 101 * i,
            right: if i % 7 == 6 { 50 } else { 101 * i + 64 },
            priority: -1.0 + 0.1375 * i as f32,
        })
        .collect()
}

fn rust_admit(rois: &[Roi]) -> u32 {
    rois.iter().fold(0, |acc, roi| {
        let roi = black_box(*roi);
        // SAFETY: a live ROI.
        acc + u32::from(unsafe { obs_encoder_roi_admit(OBS_ENCODER_CAP_ROI, &raw const roi) })
    })
}

fn c_admit(rois: &[Roi]) -> u32 {
    rois.iter().fold(0, |acc, roi| {
        let roi = black_box(*roi);
        // SAFETY: as above; the oracle reads it as struct obs_encoder_roi.
        acc + u32::from(unsafe {
            oracle_obs_encoder_roi_admit(OBS_ENCODER_CAP_ROI, (&raw const roi).cast::<c_void>())
        })
    })
}

fn sum(roi: &Roi) -> u64 {
    u64::from(roi.top)
        + u64::from(roi.bottom)
        + u64::from(roi.left)
        + u64::from(roi.right)
        + u64::from(roi.priority.to_bits())
}

fn rust_scale(rois: &[Roi]) -> u64 {
    let (mut x, mut y) = (0.0f32, 0.0f32);
    // SAFETY: two writable floats.
    let ok = unsafe {
        obs_encoder_roi_scale_factors(
            black_box(1280),
            black_box(720),
            black_box(1920),
            black_box(1080),
            &raw mut x,
            &raw mut y,
        )
    };
    assert!(ok);
    rois.iter().rev().fold(0, |acc, roi| {
        let roi = black_box(*roi);
        let mut scaled = Roi::default();
        // SAFETY: a readable ROI and a writable one.
        unsafe { obs_encoder_roi_scale(&raw const roi, x, y, &raw mut scaled) };
        acc + sum(&scaled)
    })
}

fn c_scale(rois: &[Roi]) -> u64 {
    let (mut x, mut y) = (0.0f32, 0.0f32);
    // SAFETY: as above.
    let ok = unsafe {
        oracle_obs_encoder_roi_scale_factors(
            black_box(1280),
            black_box(720),
            black_box(1920),
            black_box(1080),
            &raw mut x,
            &raw mut y,
        )
    };
    assert!(ok);
    rois.iter().rev().fold(0, |acc, roi| {
        let roi = black_box(*roi);
        let mut scaled = Roi::default();
        // SAFETY: as above.
        unsafe {
            oracle_obs_encoder_roi_scale(
                (&raw const roi).cast::<c_void>(),
                x,
                y,
                (&raw mut scaled).cast::<c_void>(),
            );
        }
        acc + sum(&scaled)
    })
}

pub fn benchmarks(criterion: &mut Criterion) {
    let rois = rois();
    pair(
        criterion,
        "encoder_roi/admit/16",
        || rust_admit(&rois),
        || c_admit(&rois),
    );
    pair(
        criterion,
        "encoder_roi/scale/16",
        || rust_scale(&rois),
        || c_scale(&rois),
    );
}

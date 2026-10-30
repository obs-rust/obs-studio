//! obs-source: the volume multiply kernels
//! (`libobs/obs-source-volume-mul.c.h`, #3080), for the `audio_mix`
//! category of #3009 and #3007. Every audio tick multiplies each source's
//! audio once per mix it is in: one volume over the whole mix
//! (`multiply_output_audio()`, a block of 1024 frames times the channels),
//! or a volume per frame for each channel when an action falls inside the
//! tick (`multiply_vol_data()`). One call here is one mix of one tick,
//! scaled down and back up (by 0.5 then 2, or by 2^-k then 2^k per frame),
//! so the samples never drift and the next call sees the same data.
//! Before timing, the samples of both sides are compared bit for bit.

use super::pair;
use criterion::Criterion;
use obs_c_oracle::source_volume_mul::{
    oracle_obs_source_volume_mul_block, oracle_obs_source_volume_mul_data,
};
use obs_source::ffi::source_volume_mul::{obs_source_volume_mul_block, obs_source_volume_mul_data};
use std::hint::black_box;

/// `AUDIO_OUTPUT_FRAMES`.
const FRAMES: usize = 1024;

type Block = unsafe extern "C" fn(*mut f32, usize, f32);
type Data = unsafe extern "C" fn(*mut f32, *const f32, usize);

fn samples(len: usize) -> Vec<f32> {
    (0..len).map(|i| ((i % 97) as f32 - 48.0) / 16.0).collect()
}

/// The samples' bits, the fixture both sides must match.
fn bits(buf: &[f32]) -> Vec<u32> {
    buf.iter().map(|s| s.to_bits()).collect()
}

fn block(criterion: &mut Criterion, name: &str, channels: usize) {
    let run = |f: Block| {
        let mut buf = samples(FRAMES * channels);
        move || {
            let out = black_box(buf.as_mut_ptr());
            // SAFETY: `buf` holds FRAMES * channels samples.
            unsafe {
                f(out, FRAMES * channels, black_box(0.5));
                f(out, FRAMES * channels, black_box(2.0));
            }
            bits(&buf[..8])
        }
    };
    let (mut rust, mut c) = (run(obs_source_volume_mul_block), run(oracle_obs_source_volume_mul_block));
    // The fixture: one pass over the whole buffer on both sides.
    let (mut r, mut o) = (samples(FRAMES * channels), samples(FRAMES * channels));
    // SAFETY: as above.
    unsafe {
        obs_source_volume_mul_block(r.as_mut_ptr(), r.len(), 0.316_227_77);
        oracle_obs_source_volume_mul_block(o.as_mut_ptr(), o.len(), 0.316_227_77);
    }
    assert_eq!(bits(&r), bits(&o), "benchmark fixture differs: {name}");
    pair(criterion, name, &mut rust, &mut c);
}

fn data(criterion: &mut Criterion, name: &str, channels: usize) {
    let down: Vec<f32> = (0..FRAMES).map(|i| 1.0 / (1u32 << (i % 4)) as f32).collect();
    let up: Vec<f32> = (0..FRAMES).map(|i| (1u32 << (i % 4)) as f32).collect();
    let run = |f: Data| {
        let mut buf = samples(FRAMES * channels);
        let (down, up) = (down.clone(), up.clone());
        move || {
            for ch in 0..channels {
                let out = black_box(buf[ch * FRAMES..].as_mut_ptr());
                // SAFETY: each channel holds FRAMES samples, each volume
                // array FRAMES volumes.
                unsafe {
                    f(out, down.as_ptr(), FRAMES);
                    f(out, up.as_ptr(), FRAMES);
                }
            }
            bits(&buf[..8])
        }
    };
    let (mut rust, mut c) = (run(obs_source_volume_mul_data), run(oracle_obs_source_volume_mul_data));
    let ramp: Vec<f32> = (0..FRAMES).map(|i| i as f32 / (FRAMES - 1) as f32).collect();
    let (mut r, mut o) = (samples(FRAMES), samples(FRAMES));
    // SAFETY: FRAMES samples and volumes.
    unsafe {
        obs_source_volume_mul_data(r.as_mut_ptr(), ramp.as_ptr(), FRAMES);
        oracle_obs_source_volume_mul_data(o.as_mut_ptr(), ramp.as_ptr(), FRAMES);
    }
    assert_eq!(bits(&r), bits(&o), "benchmark fixture differs: {name}");
    pair(criterion, name, &mut rust, &mut c);
}

pub fn benchmarks(criterion: &mut Criterion) {
    block(criterion, "audio_mix/source_volume_block/stereo", 2);
    block(criterion, "audio_mix/source_volume_block/8_channels", 8);
    data(criterion, "audio_mix/source_volume_data/stereo", 2);
}

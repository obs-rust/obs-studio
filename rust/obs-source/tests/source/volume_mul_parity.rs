//! Tier 3 parity: the shims of `obs_source::ffi::source_volume_mul` (and the
//! safe core under them) against the original
//! `libobs/obs-source-volume-mul.c.h`, compiled as
//! `oracle_obs_source_volume_mul_*` (obs-c-oracle,
//! oracle/source_volume_mul.c).
//!
//! Every call runs the shim, the C and the core on copies of one buffer of
//! two mixes laid out as `allocate_audio_output_buffer()` lays them out (8
//! consecutive channels of 1024 frames each), and compares the whole buffer
//! bit for bit, the samples past the count included.
//!
//! Runs: every count from 0 to 70 and the tick sizes of 1 to 8 channels;
//! every sample and volume over 21 special and edge values (each sample
//! value against each volume, and a ramp of volumes); random buffers and
//! volumes over all bit patterns; and proptests over any count, any
//! samples and any volumes.
//!
//! No intentional differences. A NaN sample times a NaN volume carries one
//! of the two payloads, and which one the C gives depends on how it is
//! compiled (the oracle is built like libobs, but vectorized and scalar
//! code order the operands differently); there any NaN matches.
//!
//! `test_source_volume_mul_steps` runs the C test against the C;
//! `test_source_volume_mix` runs the real libobs mix, where these shims
//! replace the C in the hybrid libobs.

#![allow(clippy::as_conversions, clippy::cast_possible_truncation)]

use obs_c_oracle::source_volume_mul::{
    oracle_obs_source_volume_mul_block, oracle_obs_source_volume_mul_data,
};
use obs_source::ffi::source_volume_mul::{obs_source_volume_mul_block, obs_source_volume_mul_data};
use obs_source::volume_mul;
use proptest::prelude::*;

const FRAMES: usize = 1024;
const CHANNELS: usize = 8;
const LEN: usize = 2 * FRAMES * CHANNELS;

/// Special and edge values, as bit patterns: zeros, ones, infinities, NaNs
/// with payloads and signs, the smallest denormal and normal, the largest
/// finite, 0.5, 2, 20 and a non-power-of-two.
const EDGES: [u32; 21] = [
    0x0000_0000,
    0x8000_0000,
    0x3f80_0000,
    0xbf80_0000,
    0x7f80_0000,
    0xff80_0000,
    0x7fc0_0000,
    0xffc0_0000,
    0x7fc0_0123,
    0x7f80_0001,
    0xff80_4567,
    0x0000_0001,
    0x8000_0001,
    0x0080_0000,
    0x7f7f_ffff,
    0xff7f_ffff,
    0x3f00_0000,
    0x4000_0000,
    0x41a0_0000,
    0x3ea1_e89b,
    0x4049_0fdb,
];

#[derive(Clone, Copy)]
enum Kernel {
    Block(f32),
    Data,
}

/// Runs `kernel` on the first `count` samples of three copies of `buf`
/// (shim, C, core) and checks each copy against the C.
fn check(buf: &[f32], vol: &[f32], kernel: Kernel, count: usize) {
    let mut shim = buf.to_vec();
    let mut c = buf.to_vec();
    let mut core = buf.to_vec();
    match kernel {
        Kernel::Block(v) => {
            // SAFETY: `count` samples of each buffer are writable.
            unsafe {
                obs_source_volume_mul_block(shim.as_mut_ptr(), count, v);
                oracle_obs_source_volume_mul_block(c.as_mut_ptr(), count, v);
            }
            volume_mul::mul_block(&mut core[..count], v);
        }
        Kernel::Data => {
            // SAFETY: `count` samples and volumes are there; no overlap.
            unsafe {
                obs_source_volume_mul_data(shim.as_mut_ptr(), vol.as_ptr(), count);
                oracle_obs_source_volume_mul_data(c.as_mut_ptr(), vol.as_ptr(), count);
            }
            volume_mul::mul_data(&mut core[..count], &vol[..count]);
        }
    }
    for i in 0..buf.len() {
        let v = match kernel {
            Kernel::Block(v) => v,
            Kernel::Data => vol.get(i).copied().unwrap_or(0.0),
        };
        let both_nan = i < count && buf[i].is_nan() && v.is_nan();
        for (name, got) in [("shim", shim[i]), ("core", core[i])] {
            let ok = if both_nan {
                got.is_nan() && c[i].is_nan()
            } else {
                got.to_bits() == c[i].to_bits()
            };
            assert!(
                ok,
                "{name}, count {count}, sample {i}: {:08x} * {:08x} gave {:08x}, C {:08x}",
                buf[i].to_bits(),
                v.to_bits(),
                got.to_bits(),
                c[i].to_bits()
            );
        }
    }
}

/// A buffer of plain samples, then a canary, as the C test fills it.
fn plain() -> Vec<f32> {
    let mut buf = vec![-123.25_f32; LEN];
    for (i, s) in buf.iter_mut().take(FRAMES * CHANNELS).enumerate() {
        *s = ((i % 97) as f32 - 48.0) / 16.0 + (i / FRAMES) as f32 * 0.125;
    }
    buf
}

/// splitmix64.
fn rng(state: &mut u64) -> u32 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (z ^ (z >> 31)) as u32
}

#[test]
fn counts_match_c() {
    let buf = plain();
    let vol: Vec<f32> = (0..LEN).map(|i| (i % 13) as f32 / 12.0).collect();
    let mut counts: Vec<usize> = (0..=70).collect();
    counts.extend((1..=CHANNELS).map(|ch| ch * FRAMES));
    counts.extend([FRAMES - 1, FRAMES + 1, 2 * FRAMES * CHANNELS]);
    for count in counts {
        check(&buf, &vol, Kernel::Block(0.5), count);
        check(&buf, &vol, Kernel::Data, count);
    }
}

#[test]
fn edge_values_match_c() {
    // Every edge sample against every edge volume, in vector and tail
    // positions.
    let mut buf = plain();
    for (i, s) in buf.iter_mut().take(FRAMES).enumerate() {
        *s = f32::from_bits(EDGES[i % EDGES.len()]);
    }
    for &v in &EDGES {
        check(&buf, &[], Kernel::Block(f32::from_bits(v)), FRAMES);
        check(&buf, &[], Kernel::Block(f32::from_bits(v)), 37);
    }
    for shift in 0..EDGES.len() {
        let vol: Vec<f32> = (0..LEN)
            .map(|i| f32::from_bits(EDGES[(i / EDGES.len() + shift) % EDGES.len()]))
            .collect();
        check(&buf, &vol, Kernel::Data, FRAMES);
        check(&buf, &vol, Kernel::Data, 37);
    }
}

#[test]
fn random_bits_match_c() {
    let mut state = 3080;
    for round in 0..64 {
        let buf: Vec<f32> = (0..LEN).map(|_| f32::from_bits(rng(&mut state))).collect();
        let vol: Vec<f32> = (0..LEN).map(|_| f32::from_bits(rng(&mut state))).collect();
        let count = 1 + rng(&mut state) as usize % (FRAMES * CHANNELS);
        check(&buf, &vol, Kernel::Block(vol[round]), count);
        check(&buf, &vol, Kernel::Data, count);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(512)))]

    #[test]
    fn any_input_matches_c(
        samples in prop::collection::vec(any::<u32>(), 0..300),
        vol in prop::collection::vec(any::<u32>(), 300),
        v: u32,
        cut in 0_usize..300,
    ) {
        let buf: Vec<f32> = samples.iter().copied().map(f32::from_bits).collect();
        let vol: Vec<f32> = vol.into_iter().map(f32::from_bits).collect();
        let count = cut.min(buf.len());
        check(&buf, &vol, Kernel::Block(f32::from_bits(v)), count);
        check(&buf, &vol, Kernel::Data, count);
    }
}

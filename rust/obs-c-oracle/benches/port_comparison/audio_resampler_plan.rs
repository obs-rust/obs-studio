//! obs-media-io: the resampler's output plan
//! (`libobs/media-io/audio-resampler-plan.c.h`). `audio_resampler_create()`
//! asks the output channels, whether the output is planar, the channel masks
//! (old libswresample), the input channels and the 4.1 tests (newer
//! libswresample) and the mono upmix gate once per resampler: a source, an
//! output or a monitor that starts or changes its format. Each side plans
//! every pair of the eight speaker layouts in planar and packed float, with
//! every one of those answers, folded into a checksum both sides must match.
//! Nothing is allocated with `bmalloc`.

use super::pair;

use core::ffi::c_int;
use criterion::Criterion;
use obs_c_oracle::audio_resampler_plan::{
    oracle_audio_resampler_input_channels, oracle_audio_resampler_is_4point1,
    oracle_audio_resampler_layout_mask, oracle_audio_resampler_output_channels,
    oracle_audio_resampler_planar_output, oracle_audio_resampler_upmixes_mono,
};
use obs_media_io::ffi::audio_resampler_drift::{
    audio_resampler_input_channels, audio_resampler_is_4point1, audio_resampler_layout_mask,
    audio_resampler_output_channels, audio_resampler_planar_output, audio_resampler_upmixes_mono,
};
use std::hint::black_box;

/// The `enum speaker_layout` values.
const LAYOUTS: [c_int; 8] = [0, 1, 2, 3, 4, 5, 6, 8];
/// `AUDIO_FORMAT_FLOAT` and `AUDIO_FORMAT_FLOAT_PLANAR`.
const FORMATS: [c_int; 2] = [4, 8];

fn mix(acc: u64, value: u64) -> u64 {
    acc.wrapping_mul(31).wrapping_add(value)
}

/// One plan's answers, folded.
fn fold(acc: u64, ch: u32, planes: u32, input_ch: c_int, masks: u64, flags: [bool; 3]) -> u64 {
    let acc = mix(acc, u64::from(ch) << 32 | u64::from(planes));
    let acc = mix(acc, u64::from(input_ch.cast_unsigned()) ^ masks);
    mix(
        acc,
        u64::from(flags[0]) | u64::from(flags[1]) << 1 | u64::from(flags[2]) << 2,
    )
}

fn rust_plans() -> u64 {
    let mut acc = 0_u64;
    for src in LAYOUTS {
        for dst in LAYOUTS {
            for format in FORMATS {
                let (src, dst, format) = (black_box(src), black_box(dst), black_box(format));
                let ch = audio_resampler_output_channels(dst);
                let planes = if audio_resampler_planar_output(format) {
                    ch
                } else {
                    1
                };
                let masks = audio_resampler_layout_mask(src)
                    ^ audio_resampler_layout_mask(dst).rotate_left(16);
                let input_ch = audio_resampler_input_channels(src);
                let flags = [
                    audio_resampler_is_4point1(src),
                    audio_resampler_is_4point1(dst),
                    input_ch == 1 && audio_resampler_upmixes_mono(ch),
                ];
                acc = fold(acc, ch, planes, input_ch, masks, flags);
            }
        }
    }
    acc
}

fn c_plans() -> u64 {
    let mut acc = 0_u64;
    for src in LAYOUTS {
        for dst in LAYOUTS {
            for format in FORMATS {
                let (src, dst, format) = (black_box(src), black_box(dst), black_box(format));
                // SAFETY: plain values in, plain values out.
                let (ch, planes, masks, input_ch, flags) = unsafe {
                    let ch = oracle_audio_resampler_output_channels(dst);
                    let planes = if oracle_audio_resampler_planar_output(format) {
                        ch
                    } else {
                        1
                    };
                    let masks = oracle_audio_resampler_layout_mask(src)
                        ^ oracle_audio_resampler_layout_mask(dst).rotate_left(16);
                    let input_ch = oracle_audio_resampler_input_channels(src);
                    let flags = [
                        oracle_audio_resampler_is_4point1(src),
                        oracle_audio_resampler_is_4point1(dst),
                        input_ch == 1 && oracle_audio_resampler_upmixes_mono(ch),
                    ];
                    (ch, planes, masks, input_ch, flags)
                };
                acc = fold(acc, ch, planes, input_ch, masks, flags);
            }
        }
    }
    acc
}

pub fn benchmarks(criterion: &mut Criterion) {
    pair(criterion, "audio_resampler_plan/plans/128", rust_plans, c_plans);
}

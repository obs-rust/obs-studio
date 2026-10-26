//! `libobs/obs-audio-render.c.h` shims against the C oracle. The fixtures
//! are what the audio thread runs every tick: the tree choice for a source
//! seen again (an individual audio source that gets tagged), and the
//! silence check for a monitored source while a capture source records the
//! monitoring device.

use std::hint::black_box;

use criterion::Criterion;
use obs_c_oracle::audio_render as c_render;
use obs_core::audio_render::{SOURCE_AUDIO, TYPE_INPUT};
use obs_core::ffi::audio_render::{
    ObsAudioMonitorDedup, obs_audio_render_silence, obs_audio_render_tree,
};

use crate::pair;

pub fn benchmarks(criterion: &mut Criterion) {
    pair(
        criterion,
        "audio_render/tree_duplicate",
        || {
            obs_audio_render_tree(
                false,
                black_box(true),
                TYPE_INPUT,
                black_box(SOURCE_AUDIO),
                false,
            )
        },
        || {
            // SAFETY: plain values.
            unsafe {
                c_render::oracle_obs_audio_render_tree(
                    false,
                    black_box(true),
                    TYPE_INPUT,
                    black_box(SOURCE_AUDIO),
                    false,
                )
            }
        },
    );

    let rust_dedup = ObsAudioMonitorDedup {
        set: true,
        active: true,
        muted: false,
        volume: 1.0,
        mixers: 0x3f,
    };
    let c_dedup = c_render::MonitorDedup {
        set: true,
        active: true,
        muted: false,
        volume: 1.0,
        mixers: 0x3f,
    };
    pair(
        criterion,
        "audio_render/silence_monitored",
        // SAFETY: a valid pointer.
        || unsafe { obs_audio_render_silence(black_box(&rust_dedup), true, false) },
        // SAFETY: as above, through the C signature.
        || unsafe { c_render::oracle_obs_audio_render_silence(black_box(&c_dedup), true, false) },
    );
}

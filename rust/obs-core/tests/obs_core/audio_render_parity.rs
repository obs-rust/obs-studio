//! Tier 3: the shims against the original `libobs/obs-audio-render.c.h`.
//!
//! The oracle (`rust/obs-c-oracle/oracle/audio_render.c`) is the C file,
//! unmodified, with its four functions renamed. Both sides get the same
//! values through their C signatures, and the results are compared.
//!
//! Runs: the sizes and values of the two enums, the size and offsets of
//! `struct obs_audio_monitor_dedup` and the three copied constants; the tree
//! choice exhaustively over removed × present × duplicated × the source
//! types 0 to 5 and `u32::MAX` × every subset of six flags (the two it reads
//! and four it does not); the silence check over every combination of the
//! five flags × volumes at, around and far from the precision (±0, ±1e-4,
//! ±5e-5, 1e-3, 1, the smallest subnormal, NaN, ±∞); the mix choice over
//! single bits and the edges; the backwards check over the edges of the
//! timestamps; and 50 000 random cases per function.
//!
//! Excluded, the intentional difference, tested on the shim alone below: a
//! null `dedup`. `test_audio_buffering_subsample` compiles the C into
//! itself, so this is what compares the swap; the real-mixer tests run it
//! through libobs.

use core::ptr;

use obs_c_oracle::audio_render::{
    LAYOUT_NAMES, MonitorDedup as CDedup, oracle_audio_render_layout,
    oracle_obs_audio_render_backwards, oracle_obs_audio_render_clear_mixes,
    oracle_obs_audio_render_silence, oracle_obs_audio_render_tree,
};
use obs_core::audio_render::{SOURCE_AUDIO, SOURCE_COMPOSITE, TYPE_INPUT};
use obs_core::ffi::audio_render::{
    OBS_AUDIO_BACKWARDS_IGNORE, OBS_AUDIO_BACKWARDS_NONE, OBS_AUDIO_BACKWARDS_PENDING,
    OBS_AUDIO_TREE_ADD, OBS_AUDIO_TREE_DUPLICATE, OBS_AUDIO_TREE_NONE, OBS_AUDIO_TREE_SKIP,
    ObsAudioMonitorDedup, obs_audio_render_backwards, obs_audio_render_clear_mixes,
    obs_audio_render_silence, obs_audio_render_tree,
};
use proptest::prelude::*;

const START: u64 = 1_000_000_000;

#[test]
fn layout() {
    let values = [
        size_of::<i32>(),
        OBS_AUDIO_TREE_NONE as usize,
        OBS_AUDIO_TREE_SKIP as usize,
        OBS_AUDIO_TREE_ADD as usize,
        OBS_AUDIO_TREE_DUPLICATE as usize,
        size_of::<i32>(),
        OBS_AUDIO_BACKWARDS_NONE as usize,
        OBS_AUDIO_BACKWARDS_PENDING as usize,
        OBS_AUDIO_BACKWARDS_IGNORE as usize,
        size_of::<ObsAudioMonitorDedup>(),
        core::mem::offset_of!(ObsAudioMonitorDedup, set),
        core::mem::offset_of!(ObsAudioMonitorDedup, active),
        core::mem::offset_of!(ObsAudioMonitorDedup, muted),
        core::mem::offset_of!(ObsAudioMonitorDedup, volume),
        core::mem::offset_of!(ObsAudioMonitorDedup, mixers),
        TYPE_INPUT as usize,
        SOURCE_AUDIO as usize,
        SOURCE_COMPOSITE as usize,
    ];
    for (which, (name, value)) in LAYOUT_NAMES.iter().zip(values).enumerate() {
        // SAFETY: a pure C function.
        assert_eq!(
            unsafe { oracle_audio_render_layout(which) },
            value,
            "{name}"
        );
    }
    // The oracle's own mirror has the same layout.
    assert_eq!(size_of::<CDedup>(), size_of::<ObsAudioMonitorDedup>());
}

fn compare_tree(removed: bool, present: bool, kind: u32, flags: u32, duplicated: bool) {
    // SAFETY: plain values.
    let c = unsafe { oracle_obs_audio_render_tree(removed, present, kind, flags, duplicated) };
    assert_eq!(
        obs_audio_render_tree(removed, present, kind, flags, duplicated),
        c,
        "removed={removed} present={present} kind={kind} flags={flags:#x} duplicated={duplicated}"
    );
}

fn compare_silence(d: ObsAudioMonitorDedup, monitoring: bool, is_dedup: bool) {
    let cd = CDedup {
        set: d.set,
        active: d.active,
        muted: d.muted,
        volume: d.volume,
        mixers: d.mixers,
    };
    // SAFETY: valid pointers to structures with the C layout (`layout`).
    let c = unsafe { oracle_obs_audio_render_silence(&cd, monitoring, is_dedup) };
    // SAFETY: as above.
    let rust = unsafe { obs_audio_render_silence(&d, monitoring, is_dedup) };
    assert_eq!(rust, c, "{d:?} monitoring={monitoring} is_dedup={is_dedup}");
}

fn compare_clear(a: u32, b: u32) {
    // SAFETY: plain values.
    let c = unsafe { oracle_obs_audio_render_clear_mixes(a, b) };
    assert_eq!(obs_audio_render_clear_mixes(a, b), c, "{a:#x} {b:#x}");
}

fn compare_backwards(maxed: bool, ts: u64, start: u64, render: bool) {
    // SAFETY: plain values.
    let c = unsafe { oracle_obs_audio_render_backwards(maxed, ts, start, render) };
    assert_eq!(
        obs_audio_render_backwards(maxed, ts, start, render),
        c,
        "maxed={maxed} ts={ts} start={start} render={render}"
    );
}

#[test]
fn tree_exhaustive() {
    // The two flags it reads, video, async, a high bit and the top bit.
    let bits = [
        SOURCE_AUDIO,
        SOURCE_COMPOSITE,
        1 << 0,
        1 << 2,
        1 << 20,
        1 << 31,
    ];
    let kinds = [0, 1, 2, 3, 4, 5, u32::MAX];
    for mask in 0u32..(1 << bits.len()) {
        let flags = bits
            .iter()
            .enumerate()
            .filter(|(i, _)| mask & (1 << i) != 0)
            .fold(0, |f, (_, b)| f | b);
        for kind in kinds {
            for state in 0u8..8 {
                compare_tree(state & 1 != 0, state & 2 != 0, kind, flags, state & 4 != 0);
            }
        }
    }
}

#[test]
fn silence_exhaustive() {
    let volumes = [
        0.0f32,
        -0.0,
        1e-4,
        -1e-4,
        5e-5,
        -5e-5,
        1.0001e-4,
        1e-3,
        1.0,
        f32::from_bits(1),
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
    ];
    for bits in 0u8..32 {
        for volume in volumes {
            let d = ObsAudioMonitorDedup {
                set: bits & 1 != 0,
                active: bits & 2 != 0,
                muted: bits & 4 != 0,
                volume,
                mixers: 0x3f,
            };
            compare_silence(d, bits & 8 != 0, bits & 16 != 0);
        }
    }
}

#[test]
fn clear_mixes_edges() {
    let edges = [0, 1, 0x3f, 0x40, 0x7f, 0xffff_ffc0, u32::MAX];
    for a in edges {
        for b in edges {
            compare_clear(a, b);
        }
    }
    for i in 0..32 {
        for j in 0..32 {
            compare_clear(1 << i, 1 << j);
        }
    }
}

#[test]
fn backwards_edges() {
    let times = [0, 1, START - 1, START, START + 1, u64::MAX - 1, u64::MAX];
    for ts in times {
        for start in times {
            for state in 0u8..4 {
                compare_backwards(state & 1 != 0, ts, start, state & 2 != 0);
            }
        }
    }
}

#[test]
fn null_dedup_silences_nothing() {
    // The intentional difference: the C would dereference it.
    // SAFETY: null is allowed by the shim's contract.
    assert!(!unsafe { obs_audio_render_silence(ptr::null(), true, false) });
}

fn timestamp() -> impl Strategy<Value = u64> {
    prop_oneof![
        (START - 100_000)..(START + 100_000),
        0u64..1000,
        (u64::MAX - 1000)..=u64::MAX,
        any::<u64>(),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(50_000)))]

    #[test]
    fn tree_matches(removed: bool, present: bool, kind in prop_oneof![0u32..6, any::<u32>()], flags: u32, duplicated: bool) {
        compare_tree(removed, present, kind, flags, duplicated);
    }

    #[test]
    fn silence_matches(
        set: bool, active: bool, muted: bool,
        volume in prop_oneof![-1e-3f32..1e-3, any::<f32>()],
        mixers: u32, monitoring: bool, is_dedup: bool,
    ) {
        compare_silence(ObsAudioMonitorDedup { set, active, muted, volume, mixers }, monitoring, is_dedup);
    }

    #[test]
    fn clear_mixes_matches(a: u32, b: u32) {
        compare_clear(a, b);
    }

    #[test]
    fn backwards_matches(maxed: bool, ts in timestamp(), start in timestamp(), render: bool) {
        compare_backwards(maxed, ts, start, render);
    }
}

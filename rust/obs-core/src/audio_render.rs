//! Safe core of `libobs/obs-audio-render.c.h`: the per-source decisions of
//! the render order and render steps of the audio tick.
//!
//! Every audio tick (`audio_callback()` in `obs-audio.c`), libobs walks the
//! active tree of each view channel and puts each source once into the
//! render order; an individual audio source that is reached twice is tagged
//! and mixed as a root node ([`tree`]). While it renders, it silences
//! monitored sources when an Audio Output Capture source records the
//! monitoring device ([`silence`], [`clear_mixes`]), and handles a source
//! that has gone backward in time at maximum buffering ([`backwards`]).
//!
//! This is only the decision for one source, on plain values. The C side
//! (`obs-audio.c.h`) walks the views and the tree, takes the locks and refs,
//! keeps `render_order` and `root_nodes`, clears the output buffers, renders
//! and logs. Nothing here keeps state, takes a lock or calls into libobs.
//!
//! Characterized, not endorsed (`test_audio_buffering_subsample`), and kept
//! as the C has them:
//! - only an individual audio source (an input with audio, not composite)
//!   is tagged; a scene or a transition reached twice is mixed through each
//!   parent;
//! - the capture source's own volume (its fader) within 0.0001 of 0 counts
//!   as muted;
//! - the backwards check is strict on the tick start and skips a source
//!   without a timestamp.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

pub use crate::audio_mix::MAX_AUDIO_MIXES;

/// `OBS_SOURCE_TYPE_INPUT` (`obs-source.h`).
pub const TYPE_INPUT: u32 = 0;
/// `OBS_SOURCE_AUDIO` (`obs-source.h`).
pub const SOURCE_AUDIO: u32 = 1 << 1;
/// `OBS_SOURCE_COMPOSITE` (`obs-source.h`).
pub const SOURCE_COMPOSITE: u32 = 1 << 6;

/// The bits of the [`MAX_AUDIO_MIXES`] mixes in a mixer mask.
const MIXES_MASK: u32 = (1 << MAX_AUDIO_MIXES) - 1;

/// The precision of the fader check, as `should_silence_monitored_source`.
const FADER_PRECISION: f32 = 0.0001;

/// `close_float()` (`graphics/math-defs.h`): `fabsf(f1 - f2) <= precision`,
/// in `f32`. NaN is never close.
fn close_float(f1: f32, f2: f32, precision: f32) -> bool {
    (f1 - f2).abs() <= precision
}

/// `is_individual_audio_source`: an input with audio that is not composite.
fn individual(kind: u32, output_flags: u32) -> bool {
    kind == TYPE_INPUT && output_flags & SOURCE_AUDIO != 0 && output_flags & SOURCE_COMPOSITE == 0
}

/// `enum obs_audio_tree_choice`: what `push_audio_tree2` does with a visited
/// source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TreeChoice {
    /// Nothing: already in the render order and not to be tagged.
    #[default]
    None,
    /// A removed source: skipped.
    Skip,
    /// Not in the render order yet: added (with a reference).
    Add,
    /// An individual audio source reached again: tagged and added to the
    /// root nodes.
    Duplicate,
}

/// `enum obs_audio_backwards_choice`: what the render loop does with a source
/// that has gone backward in time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BackwardsChoice {
    /// Nothing.
    #[default]
    None,
    /// A source with `audio_render`: marked pending.
    Pending,
    /// Another source: its audio is dropped (`ignore_audio`) and it is
    /// rendered again.
    Ignore,
}

/// `struct obs_audio_monitor_dedup`: the fields of the deduplication source
/// (`monitoring_duplicating_source`) that the silence check reads.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MonitorDedup {
    /// A deduplication source is set.
    pub set: bool,
    /// It is active.
    pub active: bool,
    /// It is muted.
    pub muted: bool,
    /// Its volume (fader).
    pub volume: f32,
    /// Its mixer mask.
    pub mixers: u32,
}

/// `push_audio_tree2`'s choice for one visited source.
pub fn tree(
    removed: bool,
    present: bool,
    kind: u32,
    output_flags: u32,
    duplicated: bool,
) -> TreeChoice {
    if removed {
        return TreeChoice::Skip;
    }
    if !present {
        return TreeChoice::Add;
    }
    if individual(kind, output_flags) && !duplicated {
        return TreeChoice::Duplicate;
    }
    TreeChoice::None
}

/// `should_silence_monitored_source`: whether a monitored source is silenced
/// while the deduplication source records the monitoring device.
pub fn silence(dedup: &MonitorDedup, monitoring_enabled: bool, is_dedup_source: bool) -> bool {
    if !dedup.set || !dedup.active {
        return false;
    }
    let fader_muted = close_float(dedup.volume, 0.0, FADER_PRECISION);
    let output_capture_unmuted = !dedup.muted && !fader_muted;
    output_capture_unmuted && monitoring_enabled && !is_dedup_source
}

/// `clear_audio_output_buf`'s choice: the mixes both sources use.
pub fn clear_mixes(dedup_mixers: u32, source_mixers: u32) -> u32 {
    dedup_mixers & source_mixers & MIXES_MASK
}

/// The render loop's check for a source that has gone backward in time.
pub fn backwards(
    maxed: bool,
    audio_ts: u64,
    tick_start: u64,
    audio_render: bool,
) -> BackwardsChoice {
    if !maxed || audio_ts == 0 || audio_ts >= tick_start {
        return BackwardsChoice::None;
    }
    if audio_render {
        BackwardsChoice::Pending
    } else {
        BackwardsChoice::Ignore
    }
}

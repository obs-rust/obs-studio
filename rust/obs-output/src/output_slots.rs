//! Safe core of `libobs/obs-output-slots.c.h`: the encoder slot rules and
//! setter gates of `obs-output.c`.
//!
//! One function per decision, each asked where the C makes it.
//! `obs-output.c` keeps `obs_output_valid()`, the `log_flag_*` checks (they
//! log as they test, so it calls them in their order and returns on their
//! result), `flag_encoded()`, `active()` (an atomic, loaded where it always
//! was), every read of the output and of the encoder, the occupancy masks it
//! builds from the slots, every pointer comparison (the slot that already
//! holds the encoder, the slots that hold it), every `obs_encoder_get_ref()`
//! and `obs_encoder_release()`, `obs_encoder_add_output()` and
//! `_remove_output()`, the caption tracks, `obs_encoder_set_scaled_size()`,
//! `obs_encoder_audio()`, `obs_output_set_mixers()`,
//! `obs_output_remove_encoder()`, `obs_output_set_service()` and every log.
//! It passes plain values here. These keep no state and call nothing in
//! libobs.
//!
//! - `get_first_video_encoder_index()` and `get_first_audio_encoder_index()`
//!   (which `obs_output_audio()` now calls): [`first_slot`], the lowest
//!   occupied slot.
//! - `obs_output_set_mixer()`: [`mixer_in_range`] and [`mixer_mask`].
//! - `obs_output_set_video_encoder2()` and `obs_output_set_audio_encoder()`:
//!   [`type_ok`] (the type gate, for an encoder that is not NULL; also
//!   the slots `obs_output_remove_encoder_internal()` looks in),
//!   [`index_ok`] (after the active gate: below the slot count with the
//!   kind's multitrack flag, else 0 only), and for video at the default
//!   index, [`default_index`] and [`scaled_axis_set`] (the encoder takes the
//!   output's preferred size when both axes are set).
//! - `obs_output_get_video_encoder2()`, `obs_output_get_audio_encoder()` and
//!   `obs_output_set_preferred_size2()`: [`slot_exists`], the index bounds
//!   whatever the flags; `obs_output_set_preferred_size2()` also asks
//!   [`default_index`] for the raw size write.
//!
//! No input comes from outside OBS (the flags from an output plugin, the
//! encoder's type from an encoder plugin, the indexes and sizes from the
//! frontend or a plugin), and nothing here can panic on any input.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

pub use crate::video_checks::MAX_AUDIO_MIXES;
use crate::video_checks::{
    MAX_OUTPUT_AUDIO_ENCODERS as CHECKS_AUDIO_ENCODERS,
    MAX_OUTPUT_VIDEO_ENCODERS as CHECKS_VIDEO_ENCODERS,
};

/// `OBS_OUTPUT_MULTI_TRACK_AUDIO` (`OBS_OUTPUT_MULTI_TRACK`, `obs-output.h`).
pub const OBS_OUTPUT_MULTI_TRACK_AUDIO: u32 = 1 << 4;
/// `OBS_OUTPUT_MULTI_TRACK_VIDEO` (`obs-output.h`).
pub const OBS_OUTPUT_MULTI_TRACK_VIDEO: u32 = 1 << 6;
/// `MAX_OUTPUT_VIDEO_ENCODERS` (`obs-output.h`): the video slots.
pub const MAX_OUTPUT_VIDEO_ENCODERS: usize = 10;
/// `MAX_OUTPUT_AUDIO_ENCODERS` (`obs-output.h`): the audio slots.
pub const MAX_OUTPUT_AUDIO_ENCODERS: usize = 6;
/// `OBS_ENCODER_AUDIO` (`enum obs_encoder_type`, `obs-encoder.h`).
pub const OBS_ENCODER_AUDIO: i32 = 0;
/// `OBS_ENCODER_VIDEO`.
pub const OBS_ENCODER_VIDEO: i32 = 1;

// The slot counts are the ones `video_checks` mirrors for
// `can_begin_data_capture()`.
const _: () = assert!(CHECKS_VIDEO_ENCODERS == 10 && MAX_OUTPUT_VIDEO_ENCODERS == 10);
const _: () = assert!(CHECKS_AUDIO_ENCODERS == 6 && MAX_OUTPUT_AUDIO_ENCODERS == 6);

/// Bit `i` of `mask`; clear from the width of `mask` up.
fn bit_set(mask: u32, i: usize) -> bool {
    u32::try_from(i)
        .ok()
        .and_then(|i| mask.checked_shr(i))
        .is_some_and(|m| m & 1 != 0)
}

/// The two kinds of encoder slot an output has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotKind {
    /// `video_encoders`, [`MAX_OUTPUT_VIDEO_ENCODERS`] of them.
    Video,
    /// `audio_encoders`, [`MAX_OUTPUT_AUDIO_ENCODERS`] of them.
    Audio,
}

impl SlotKind {
    /// The number of slots of this kind.
    #[must_use]
    pub fn count(self) -> usize {
        match self {
            Self::Video => MAX_OUTPUT_VIDEO_ENCODERS,
            Self::Audio => MAX_OUTPUT_AUDIO_ENCODERS,
        }
    }

    /// The flag that gives an output more than one slot of this kind.
    fn multi_track_flag(self) -> u32 {
        match self {
            Self::Video => OBS_OUTPUT_MULTI_TRACK_VIDEO,
            Self::Audio => OBS_OUTPUT_MULTI_TRACK_AUDIO,
        }
    }

    /// The `enum obs_encoder_type` value of an encoder for these slots.
    fn encoder_type(self) -> i32 {
        match self {
            Self::Video => OBS_ENCODER_VIDEO,
            Self::Audio => OBS_ENCODER_AUDIO,
        }
    }
}

/// `get_first_video_encoder_index()` / `get_first_audio_encoder_index()`:
/// the lowest slot below the kind's count whose bit is set in `occupied`
/// (bit `i`: slot `i` holds an encoder), or none. Bits from the count up are
/// ignored.
#[must_use]
pub fn first_slot(kind: SlotKind, occupied: u32) -> Option<usize> {
    (0..kind.count()).find(|&i| bit_set(occupied, i))
}

/// `obs_output_set_mixer()`: `mixer_idx` is below [`MAX_AUDIO_MIXES`];
/// otherwise the C logs and ignores it.
#[must_use]
pub fn mixer_in_range(mixer_idx: usize) -> bool {
    mixer_idx < MAX_AUDIO_MIXES
}

/// `obs_output_set_mixer()`: the mixer mask of `mixer_idx` alone, bit
/// `mixer_idx`. Asked only in range; from the width of `usize` up (where the
/// C's shift is undefined) the mask is 0.
#[must_use]
pub fn mixer_mask(mixer_idx: usize) -> usize {
    u32::try_from(mixer_idx)
        .ok()
        .and_then(|i| 1usize.checked_shl(i))
        .unwrap_or(0)
}

/// `obs_output_set_video_encoder2()` / `obs_output_set_audio_encoder()`, for
/// an encoder that is not NULL: its type fits the kind's slots; otherwise the
/// C logs and refuses it. `obs_output_remove_encoder_internal()` asks it for
/// video, then for audio: the slots the encoder can be in (none for a type
/// that is neither).
#[must_use]
pub fn type_ok(kind: SlotKind, encoder_type: i32) -> bool {
    encoder_type == kind.encoder_type()
}

/// `obs_output_set_video_encoder2()` / `obs_output_set_audio_encoder()`,
/// after the active gate: with the kind's multitrack flag
/// ([`OBS_OUTPUT_MULTI_TRACK_VIDEO`], [`OBS_OUTPUT_MULTI_TRACK_AUDIO`]) in
/// `flags`, `idx` is below the kind's count; without it, `idx` is 0.
#[must_use]
pub fn index_ok(kind: SlotKind, flags: u32, idx: usize) -> bool {
    if flags & kind.multi_track_flag() != 0 {
        idx < kind.count()
    } else {
        idx == 0
    }
}

/// `obs_output_set_video_encoder2()` and `obs_output_set_preferred_size2()`:
/// `idx` is the default index 0, the one that keeps the single-encoder
/// behaviour.
#[must_use]
pub fn default_index(idx: usize) -> bool {
    idx == 0
}

/// `obs_output_set_video_encoder2()` at the default index: one axis of the
/// output's scaled size is set (not 0). The C asks for the width, then for
/// the height only when the width is set.
#[must_use]
pub fn scaled_axis_set(size: u32) -> bool {
    size != 0
}

/// `obs_output_get_video_encoder2()`, `obs_output_get_audio_encoder()` and
/// `obs_output_set_preferred_size2()` (video): `idx` is one of the kind's
/// slots, whatever the flags.
#[must_use]
pub fn slot_exists(kind: SlotKind, idx: usize) -> bool {
    idx < kind.count()
}

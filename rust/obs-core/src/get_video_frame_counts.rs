//! Safe core of `libobs/obs-get-video-frame-counts.c.h`:
//! `obs_get_video_frame_counts()`, which sums the total and skipped frame
//! counters of the main canvas's mix and of every encoder-only mix of the
//! same view (a GPU-rescaling encoder's mix).
//!
//! With no main mix (no main canvas, or before the first video reset) the
//! answer is zero and no lock is taken. Otherwise, under the core's
//! `mixes_mutex`, the mixes are walked in order, the mix count read again
//! before each step as the C loop does, and each mix is put to
//! [`crate::core_setup_rules::mix_step`] in the C's order: the main mix,
//! then `encoder_only_mix`, then the same view as the main mix, then a
//! video output, each field read only when the rule asks for it. The two
//! counters of every counted mix are added with `uint32_t` wrapping. The
//! mutex is released before the caller's outputs are written, each only when
//! it is not NULL. Nothing else is written and no reference is taken.

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

use crate::core_setup_rules::{Mix, mix_step};

/// The core's mixes as the query sees them: the main mix,
/// `obs->video.mixes_mutex`, `mixes.num`, `mixes.array[i]`, the fields the
/// rule asks about and the mix's two `video_t` counters.
pub trait FrameCounts {
    /// A `struct obs_core_video_mix *`, compared by identity.
    type Mix: PartialEq;
    /// `obs->data.main_canvas ? obs->data.main_canvas->mix : NULL`.
    fn main_mix(&mut self) -> Option<Self::Mix>;
    fn lock(&mut self);
    fn unlock(&mut self);
    /// One read of `mixes.num`.
    fn count(&mut self) -> usize;
    /// One read of `mixes.array[index]`, for `index < count()`.
    fn at(&mut self, index: usize) -> Self::Mix;
    /// One read of the mix's `encoder_only_mix`.
    fn encoder_only(&mut self, mix: &Self::Mix) -> bool;
    /// `mix->view == main->view`.
    fn same_view(&mut self, mix: &Self::Mix, main: &Self::Mix) -> bool;
    /// `mix->video != NULL`.
    fn has_video(&mut self, mix: &Self::Mix) -> bool;
    /// `video_output_get_total_frames(mix->video)`.
    fn total_frames(&mut self, mix: &Self::Mix) -> u32;
    /// `video_output_get_skipped_frames(mix->video)`.
    fn skipped_frames(&mut self, mix: &Self::Mix) -> u32;
}

/// The summed `(total, skipped)` frame counts.
pub fn get_video_frame_counts<M: FrameCounts>(mixes: &mut M) -> (u32, u32) {
    let Some(main) = mixes.main_mix() else {
        return (0, 0);
    };
    let (mut total, mut skipped) = (0_u32, 0_u32);
    mixes.lock();
    let mut index = 0_usize;
    while index < mixes.count() {
        let mix = mixes.at(index);
        let mut step = mix_step(Mix::Main, mix == main);
        if step == Mix::EncoderOnly {
            step = mix_step(step, mixes.encoder_only(&mix));
        }
        if step == Mix::View {
            step = mix_step(step, mixes.same_view(&mix, &main));
        }
        if step == Mix::Video {
            step = mix_step(step, mixes.has_video(&mix));
        }
        if step == Mix::Count {
            total = total.wrapping_add(mixes.total_frames(&mix));
            skipped = skipped.wrapping_add(mixes.skipped_frames(&mix));
        }
        // `index < count()`, so this never wraps.
        index = index.wrapping_add(1);
    }
    mixes.unlock();
    (total, skipped)
}

/// Writes each count to its output when there is one, total first.
pub fn write_counts(counts: (u32, u32), total: Option<&mut u32>, skipped: Option<&mut u32>) {
    if let Some(total) = total {
        *total = counts.0;
    }
    if let Some(skipped) = skipped {
        *skipped = counts.1;
    }
}

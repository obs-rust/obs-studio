//! Safe core of `libobs/obs-source-output-cea708.c.h`: the exported caption
//! dispatch `obs_source_output_cea708()`.
//!
//! A source being destroyed returns before anything else is read; a
//! missing caption packet returns before the list mutex is taken. Otherwise
//! the source's caption list is locked, its count read once, and every
//! entry from the last to the first is read from the list as it is at that
//! moment and called, under the mutex, which is released after the last
//! call (when the [`Locked`] guard drops).
//!
//! No intentional differences from C.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

/// The caption list of one live source, locked. Dropping it unlocks.
pub trait Locked {
    /// `caption_cb_list.num`.
    fn count(&self) -> usize;
    /// Reads `caption_cb_list.array[index]` as the list is now and calls it
    /// with the source and the caption packet.
    fn call(&mut self, index: usize);
}

/// One source's state the dispatch reads.
pub trait Source {
    /// The locked caption list.
    type Locked<'a>: Locked
    where
        Self: 'a;
    /// `destroying(source)`.
    fn destroying(&self) -> bool;
    /// Takes `caption_cb_mutex`.
    fn lock(&self) -> Self::Locked<'_>;
}

/// `obs_source_output_cea708(source, captions)`; `has_captions` is
/// `captions != NULL`.
pub fn output<S: Source>(source: &S, has_captions: bool) {
    let _ = (source, has_captions);
    todo!("obs_source_output_cea708 safe core (#5750)")
}

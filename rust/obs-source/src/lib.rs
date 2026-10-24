//! Rust ports of the units extracted from `libobs/obs-source.c`,
//! `libobs/obs-source-deinterlace.c` and `libobs/obs-source-transition.c`,
//! one module per file: so far
//! `libobs/obs-source-async-pacing.c.h`, the async video frame pacing of
//! `get_closest_frame()`; `libobs/obs-source-audio-input.c.h`, the audio
//! input steps of `process_audio()` and `reset_resampler()`;
//! `libobs/obs-source-audio-timing.c.h`, the audio timestamp placement of
//! `source_output_audio_data()`; `libobs/obs-source-color-space.c.h`, the
//! colour space query routing of `obs_source_get_color_space()`;
//! `libobs/obs-source-deinterlace-pacing.c.h`, the frame pacing of
//! double-rate deinterlacing; and `libobs/obs-source-transition-clock.c.h`,
//! the clock and fit decisions of the transitions.
//!
//! The safe cores are [`async_pacing`], [`audio_input`], [`audio_timing`],
//! [`color_space`], [`deinterlace_pacing`] and [`transition_clock`]. The C
//! ABI shims live in
//! [`ffi`].

#![deny(unsafe_code)]

pub mod async_pacing;
pub mod audio_input;
pub mod audio_timing;
pub mod color_space;
pub mod deinterlace_pacing;
pub mod transition_clock;

#[allow(unsafe_code)]
pub mod ffi;

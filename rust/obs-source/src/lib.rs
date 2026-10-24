//! Rust ports of the units extracted from `libobs/obs-source.c`,
//! `libobs/obs-source-deinterlace.c` and `libobs/obs-source-transition.c`,
//! one module per file: so far
//! `libobs/obs-source-async-pacing.c.h`, the async video frame pacing of
//! `get_closest_frame()`; `libobs/obs-source-audio-input.c.h`, the audio
//! input steps of `process_audio()` and `reset_resampler()`;
//! `libobs/obs-source-audio-timing.c.h`, the audio timestamp placement of
//! `source_output_audio_data()`; `libobs/obs-source-color-space.c.h`, the
//! colour space query routing of `obs_source_get_color_space()`;
//! `libobs/obs-source-conversion-plan.c.h`, the async video GPU conversion
//! plan (convert type, planes, technique and linear output);
//! `libobs/obs-source-deinterlace-draw.c.h`, the draw decisions of
//! deinterlacing (the effect, the early return, the linear sRGB flag, the
//! technique and multiplier, the second field and the mode change);
//! `libobs/obs-source-deinterlace-pacing.c.h`, the frame pacing of
//! double-rate deinterlacing; `libobs/obs-source-ptm-gate.c.h`, the
//! push-to-mute and push-to-talk gate of `source_muted()`;
//! `libobs/obs-source-size.c.h`, the size query routing of
//! `obs_source_get_width()` and `obs_source_get_height()`;
//! `libobs/obs-source-transition-clock.c.h`, the clock and fit decisions of
//! the transitions; `libobs/obs-source-transition-render.c.h`, the render
//! and audio step decisions of the transitions; and
//! `libobs/obs-source-volume.c.h`, the volume snap of
//! `get_source_volume()` and the apply decisions of `apply_audio_volume()`.
//!
//! The safe cores are [`async_pacing`], [`audio_input`], [`audio_timing`],
//! [`color_space`], [`conversion_plan`], [`deinterlace_draw`],
//! [`deinterlace_pacing`], [`ptm_gate`], [`size_route`],
//! [`transition_clock`], [`transition_render`] and [`volume_apply`]. The C
//! ABI shims live in [`ffi`].

#![deny(unsafe_code)]

pub mod async_pacing;
pub mod audio_input;
pub mod audio_timing;
pub mod color_space;
pub mod conversion_plan;
pub mod deinterlace_draw;
pub mod deinterlace_pacing;
pub mod draw_plan;
pub mod draw_technique;
pub mod ptm_gate;
pub mod size_route;
pub mod transition_clock;
pub mod transition_render;
pub mod volume_apply;

mod platform;

#[allow(unsafe_code)]
pub mod ffi;

//! Rust ports of the units extracted from `libobs/obs-source.c`,
//! `libobs/obs-source-deinterlace.c` and `libobs/obs-source-transition.c`,
//! one module per file: so far
//! `libobs/obs-source-async-pacing.c.h`, the async video frame pacing of
//! `get_closest_frame()`; `libobs/obs-source-async-upload.c.h`, the async
//! video upload decisions of `set_async_texture_size()`,
//! `update_async_texrender()`, `update_async_textures()`,
//! `check_to_swap_bgrx_bgra()` and `obs_source_update_async_video()`;
//! `libobs/obs-source-audio-input.c.h`, the audio
//! input steps of `process_audio()` and `reset_resampler()`;
//! `libobs/obs-source-audio-render.c.h`, the audio render mix plan of
//! `obs_source_audio_render()`, `custom_audio_render()` and
//! `process_audio_source_tick()`;
//! `libobs/obs-source-audio-timing.c.h`, the audio timestamp placement of
//! `source_output_audio_data()`; `libobs/obs-source-color-space.c.h`, the
//! colour space query routing of `obs_source_get_color_space()`;
//! `libobs/obs-source-conversion-plan.c.h`, the async video GPU conversion
//! plan (convert type, planes, technique and linear output);
//! `libobs/obs-source-deinterlace-draw.c.h`, the draw decisions of
//! deinterlacing (the effect, the early return, the linear sRGB flag, the
//! technique and multiplier, the second field and the mode change);
//! `libobs/obs-source-deinterlace-pacing.c.h`, the frame pacing of
//! double-rate deinterlacing; `libobs/obs-source-deinterlace-select.c.h`,
//! the frame selection of deinterlacing (which queued frames a deinterlaced
//! source drops and takes, and the texture update);
//! `libobs/obs-source-draw-plan.c.h`, the source
//! draw decisions of `rotate_async_video()`,
//! `obs_source_render_async_video()` and `source_render()` (the rotation
//! translate, the linear sRGB flag and non-linear alpha, the technique, the
//! multiplier and the colour space conversion);
//! `libobs/obs-source-frame-copy.c.h`, the async frame copy plan and cache
//! decisions of `copy_frame_data()`, `cache_video()`, `clean_cache()` and the
//! `obs_source_output_video()` families;
//! `libobs/obs-source-media-admission.c.h`, the media action admission
//! decisions of the `obs_source_media_*()` functions and
//! `process_media_actions()` (whether a call is accepted, the getters'
//! fallbacks, and each queued action's callback and signal);
//! `libobs/obs-source-ptm-gate.c.h`, the
//! push-to-mute and push-to-talk gate of `source_muted()`;
//! `libobs/obs-source-render-route.c.h`, the source and filter render
//! routing (which draw a source or a filter takes, the filter bypass, the
//! default effect and the linear sRGB flags);
//! `libobs/obs-source-size.c.h`, the size query routing of
//! `obs_source_get_width()` and `obs_source_get_height()`;
//! `libobs/obs-source-transition-clock.c.h`, the clock and fit decisions of
//! the transitions; `libobs/obs-source-transition-render.c.h`, the render
//! and audio step decisions of the transitions; and
//! `libobs/obs-source-volume.c.h`, the volume snap of
//! `get_source_volume()` and the apply decisions of `apply_audio_volume()`.
//!
//! The safe cores are [`async_pacing`], [`async_upload`], [`audio_input`],
//! [`audio_render_plan`], [`audio_timing`],
//! [`color_space`], [`conversion_plan`], [`deinterlace_draw`],
//! [`deinterlace_pacing`], [`deinterlace_select`], [`draw_plan`], [`frame_copy`], [`media_admission`], [`ptm_gate`], [`render_route`], [`size_route`],
//! [`transition_clock`], [`transition_render`] and [`volume_apply`]; [`draw_technique`] holds the
//! technique and colour space values [`deinterlace_draw`] and [`draw_plan`]
//! share. The C
//! ABI shims live in [`ffi`].

#![deny(unsafe_code)]

pub mod async_pacing;
pub mod async_upload;
pub mod audio_input;
pub mod audio_render_plan;
pub mod audio_timing;
pub mod color_space;
pub mod conversion_plan;
pub mod deinterlace_draw;
pub mod deinterlace_pacing;
pub mod deinterlace_select;
pub mod draw_plan;
pub mod draw_technique;
pub mod frame_copy;
pub mod media_admission;
pub mod ptm_gate;
pub mod render_route;
pub mod size_route;
pub mod transition_clock;
pub mod transition_render;
pub mod volume_apply;

mod platform;

#[allow(unsafe_code)]
pub mod ffi;

//! Rust ports of the units extracted from `libobs/obs-scene.c`, one module
//! per file: `libobs/obs-scene-bounds.c.h`, `libobs/obs-scene-blend.c.h`,
//! `libobs/obs-scene-bounds-place.c.h`, `libobs/obs-scene-coords.c.h`,
//! `libobs/obs-scene-group-transform.c.h`,
//! `libobs/obs-scene-audio-actions.c.h`, `libobs/obs-scene-extent.c.h`,
//! `libobs/obs-scene-crop.c.h`, `libobs/obs-scene-render-plan.c.h` (the
//! per-item render decisions of `render_item` and `render_item_texture`),
//! `libobs/obs-scene-item-names.c.h` (the scene item save/load names and
//! coordinate keys of `scene_load_item` and `scene_save_item`),
//! `libobs/obs-scene-audio-window.c.h` (the audio window decisions of
//! `scene_audio_render`: the source each item is read from, the tick's
//! earliest timestamp, and each item's frames), and
//! `libobs/obs-scene-item-transform.c.h` (the steps of
//! `compute_item_transform`: the crop and cropped size, the bounds test,
//! the draw and box matrices, and the box scale).
//!
//! The safe cores are [`bounds`], [`bounds_place`], [`blend`], [`coords`],
//! [`group_transform`], [`audio_actions`], [`audio_window`], [`extent`],
//! [`crop`], [`render_plan`], [`item_names`], and [`item_transform`]. The C
//! ABI shims live in [`ffi`].

#![deny(unsafe_code)]

pub mod audio_actions;
pub mod audio_window;
pub mod blend;
pub mod bounds;
pub mod bounds_place;
pub mod coords;
pub mod crop;
pub mod extent;
pub mod group_transform;
pub mod item_names;
pub mod item_transform;
mod platform;
pub mod render_plan;

#[allow(unsafe_code)]
pub mod ffi;

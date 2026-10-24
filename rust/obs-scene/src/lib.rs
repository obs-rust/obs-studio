//! Rust ports of the units extracted from `libobs/obs-scene.c`, one module
//! per file: `libobs/obs-scene-bounds.c.h`, `libobs/obs-scene-blend.c.h`,
//! `libobs/obs-scene-bounds-place.c.h`, `libobs/obs-scene-coords.c.h`,
//! `libobs/obs-scene-group-transform.c.h`,
//! `libobs/obs-scene-audio-actions.c.h`, `libobs/obs-scene-extent.c.h`,
//! `libobs/obs-scene-crop.c.h`, `libobs/obs-scene-render-plan.c.h` (the
//! per-item render decisions of `render_item` and `render_item_texture`),
//! and `libobs/obs-scene-item-names.c.h` (the scene item save/load names and
//! coordinate keys of `scene_load_item` and `scene_save_item`).
//!
//! The safe cores are [`bounds`], [`bounds_place`], [`blend`], [`coords`],
//! [`group_transform`], [`audio_actions`], [`extent`], [`crop`],
//! [`render_plan`], and [`item_names`]. The C ABI shims live in [`ffi`].

#![deny(unsafe_code)]

pub mod audio_actions;
pub mod blend;
pub mod bounds;
pub mod bounds_place;
pub mod coords;
pub mod crop;
pub mod extent;
pub mod group_transform;
pub mod item_names;
mod platform;
pub mod render_plan;

#[allow(unsafe_code)]
pub mod ffi;

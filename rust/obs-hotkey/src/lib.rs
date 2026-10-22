//! Rust ports of `libobs/obs-hotkey-name-map.c`,
//! `libobs/obs-hotkey-bindings.c.h`, `libobs/obs-hotkey-translations.c.h`,
//! `libobs/obs-hotkey-press.c.h`, `libobs/obs-nix-hotkey-state.c.h` and
//! `libobs/obs-nix-portal-shortcut-ids.c.h`.
//!
//! The safe cores are [`name_map`], [`bindings`], [`translations`],
//! [`press`], [`nix_hotkey_state`] and [`portal_shortcut_ids`]. The C ABI
//! shims live in [`ffi`].

#![deny(unsafe_code)]

// The lib test harness has no tests of its own, but it still links the
// `no_mangle` shims. Windows rust-lld keeps those shims, so the symbols
// they call have to come from a crate the harness actually references.
#[cfg(test)]
use obs_c_oracle as _; // blog
#[cfg(test)]
use obs_util as _; // base log handler

pub mod bindings;
pub mod name_map;
pub mod nix_hotkey_state;
pub mod portal_shortcut_ids;
pub mod press;
pub mod translations;

#[allow(unsafe_code)]
pub mod ffi;

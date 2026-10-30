//! Rust ports of `libobs/obs-hotkey-name-map.c`,
//! `libobs/obs-hotkey-bindings.c.h`, `libobs/obs-hotkey-translations.c.h`,
//! `libobs/obs-hotkey-press.c.h`, `libobs/obs-hotkey-pair.c.h`,
//! `libobs/obs-nix-hotkey-state.c.h`,
//! `libobs/obs-nix-portal-shortcut-ids.c.h`,
//! `libobs/obs-nix-portal-shortcut-sets.c.h`,
//! `libobs/obs-nix-wayland-keys.c.h`, `libobs/obs-nix-x11-input.c.h`,
//! `libobs/obs-nix-x11-keys.c.h`, `libobs/obs-cocoa-keys.c.h` and
//! `libobs/obs-windows-keys.c.h`.
//!
//! The safe cores are [`name_map`], [`bindings`], [`translations`],
//! [`press`], [`pair`], [`nix_hotkey_state`], [`portal_shortcut_ids`],
//! [`portal_shortcut_sets`], [`nix_wayland_keys`], [`nix_x11_input`],
//! [`nix_x11_keys`], [`cocoa_keys`] and [`windows_keys`]. The C ABI shims
//! live in [`ffi`].

#![deny(unsafe_code)]

// The lib test harness has no tests of its own, but it still links the
// `no_mangle` shims. Windows rust-lld keeps those shims, so the symbols
// they call have to come from a crate the harness actually references.
#[cfg(test)]
use obs_c_oracle as _; // blog
#[cfg(test)]
use obs_util as _; // base log handler

pub mod bindings;
pub mod cocoa_keys;
pub mod name_map;
pub mod nix_hotkey_state;
pub mod nix_wayland_keys;
pub mod nix_x11_input;
pub mod nix_x11_keys;
pub mod pair;
pub mod portal_shortcut_ids;
pub mod portal_shortcut_sets;
pub mod press;
pub mod translations;
pub mod windows_keys;

#[allow(unsafe_code)]
pub mod ffi;

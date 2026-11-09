//! C ABI shims, one module per C file.

pub mod background_app_id;
pub mod background_query;
pub mod binding_getters;
pub mod binding_press;
pub mod bindings;
pub mod cocoa_keys;
pub mod hotkey_name_map;
pub mod identity_getters;
pub mod identity_setters;
pub mod nix_hotkey_state;
pub mod nix_wayland_key_names;
pub mod nix_wayland_keys;
pub mod nix_wayland_seats;
pub mod nix_x11_hotkeys;
pub mod nix_x11_input;
pub mod nix_x11_keys;
pub mod object_getters;
pub mod pair;
pub mod portal_shortcut_ids;
pub mod portal_shortcut_sets;
pub mod portal_shortcut_status;
pub mod press;
pub mod save_load;
pub mod translations;
pub mod windows_keys;

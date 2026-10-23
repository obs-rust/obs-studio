//! obs-util integration tests: platform (one module per former tests/*.rs file, #968).

use crate::common;

#[path = "../../build/gio.rs"]
mod build_gio;
#[cfg(all(unix, not(target_os = "macos")))]
mod dbus_inhibit;
#[cfg(all(feature = "gio", any(target_os = "linux", target_os = "freebsd")))]
mod dbus_inhibit_parity;
mod export_list;
mod gio_probe;
mod nix_inhibit;
mod nix_inhibit_parity;
mod path_extension;
mod path_extension_parity;
mod pipe;
mod pipe_parity;
mod platform_cocoa;
mod platform_cocoa_parity;
mod platform_conversions;
mod platform_conversions_parity;
mod platform_file;
mod platform_file_parity;
mod platform_filename;
mod platform_filename_layout;
mod platform_filename_parity;
mod platform_mkdirs;
mod platform_mkdirs_parity;
mod platform_nix;
mod platform_nix_layout;
mod platform_nix_parity;
mod platform_number;
mod platform_number_parity;
mod platform_time;
mod platform_time_layout;
mod platform_time_parity;
mod platform_windows;
mod platform_windows_layout;
mod platform_windows_parity;
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
mod portal_inhibit;
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
mod portal_inhibit_parity;
mod scratch_cleanup;
mod sleep_duration;

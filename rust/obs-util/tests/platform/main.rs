//! obs-util integration tests: platform (one module per former tests/*.rs file, #968).

use crate::common;

mod export_list;
mod nix_inhibit;
mod nix_inhibit_parity;
mod path_extension;
mod path_extension_parity;
mod pipe;
mod pipe_parity;
mod platform_cocoa;
mod platform_cocoa_parity;
mod platform_file;
mod platform_file_parity;
mod platform_mkdirs;
mod platform_mkdirs_parity;
mod platform_nix;
mod platform_nix_layout;
mod platform_nix_parity;
mod platform_number;
mod platform_number_parity;
mod platform_windows;
mod platform_windows_layout;
mod platform_windows_parity;
mod scratch_cleanup;
mod sleep_duration;

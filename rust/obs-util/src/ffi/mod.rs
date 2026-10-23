//! C ABI shims replacing the original libobs `EXPORT` symbols.

pub mod array_serializer;
pub mod base;
pub mod bitstream;
pub mod bmem;
pub mod buffered_file_serializer;
pub mod cf_parser;
pub mod cf_tokenizer;
pub mod config_file;
pub mod crc32;
pub mod darray;
#[cfg(all(unix, not(target_os = "macos"), feature = "gio"))]
pub mod dbus_inhibit;
pub mod deque;
pub mod dstr;
pub mod dstr_libc;
pub mod file_serializer;
pub mod lexer;
#[cfg(unix)]
pub mod native_sem;
pub mod path_extension;
pub mod pipe;
#[cfg(target_os = "macos")]
pub mod platform_cocoa;
pub mod platform_conversions;
pub mod platform_file;
pub mod platform_filename;
pub mod platform_mkdirs;
#[cfg(unix)]
pub mod platform_nix;
#[cfg(all(unix, not(target_os = "macos")))]
pub mod platform_nix_inhibit;
pub mod platform_number;
pub mod platform_time;
#[cfg(windows)]
pub mod platform_windows;
#[cfg(all(unix, not(target_os = "macos"), feature = "gio"))]
pub mod portal_inhibit;
pub mod profiler;
pub mod profiler_io;
pub mod profiler_snapshot;
pub mod task;
pub mod text_lookup;
pub mod threading;
#[cfg(not(windows))]
pub mod utf8;
#[cfg(windows)]
#[path = "utf8_windows.rs"]
pub mod utf8;

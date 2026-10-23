//! Rust ports of `libobs/util`.
//!
//! Safe cores live at the crate root; the C ABI shims that replace the
//! original `EXPORT` symbols live in [`ffi`]. See
//! `docs/rust-port/testing-policy.md`.
//!
//! Host-specific code lives only in [`platform`] (and the per-OS export set in
//! [`ffi`]); the `ban_platform_cfg_outside_boundary` Dylint rejects host
//! `cfg`s and native OS paths anywhere else.
//!
//! `unsafe_code` is denied for the whole crate. The allow sits on [`ffi`]
//! and on the platform modules that call the OS: the Windows module, the
//! Unix nix module, and the macOS Cocoa module.

#![deny(unsafe_code)]

#[cfg(test)]
use obs_c_oracle as _; // links the oracle's libobs sources and platform stand-ins

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
pub mod deque;
pub mod dstr;
pub mod dstr_libc;
#[allow(unsafe_code)] // the C ABI shims
pub mod ffi;
pub mod file_serializer;
pub mod lexer;
pub mod path_extension;
pub mod pipe;
pub mod platform;
pub mod platform_file;
pub mod platform_filename;
pub mod platform_mkdirs;
pub mod platform_number;
pub mod platform_time;
pub mod profiler;
pub mod profiler_snapshot;
pub mod task;
pub mod text_lookup;
pub mod threading;
pub mod utf8;
pub mod util_uint64;

#[cfg(test)]
extern crate self as obs_util;

#[cfg(test)]
#[allow(unsafe_code)] // Existing layout and C-oracle tests retain their FFI boundary.
#[path = "../tests/suite.rs"]
mod tests;

#[cfg(test)]
#[path = "../tests/common/mod.rs"]
mod common;

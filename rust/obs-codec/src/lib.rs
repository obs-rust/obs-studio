//! Rust ports of the libobs codec bitstream helpers: `obs-nal.c`,
//! `obs-hevc.c` and `obs-av1.c`, and later `obs-avc.c`.
//!
//! Safe cores live at the crate root; the C ABI shims that replace the
//! original `EXPORT` symbols live in [`ffi`]. See
//! `docs/rust-port/testing-policy.md`.

#[cfg(test)]
use obs_c_oracle as _; // links the test bmalloc/bfree (oracle/test_bmem.c)

pub mod av1;
pub mod avc;
pub mod ffi;
pub mod hevc;
pub mod nal;

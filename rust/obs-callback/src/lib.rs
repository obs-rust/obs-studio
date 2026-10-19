//! Rust port of `libobs/callback/calldata.c.h` and `libobs/callback/decl.c.h`.
//!
//! [`calldata`] and [`decl`] are the safe cores. The C ABI shims live in
//! [`ffi`].

#![deny(unsafe_code)]

pub mod calldata;
pub mod decl;

#[allow(unsafe_code)]
pub mod ffi;

pub use calldata::{Calldata, FIXED_OVERFLOW_MESSAGE, StringGet};
pub use decl::{Decl, DeclParam, DeclParse, ParamType, parse_decl};

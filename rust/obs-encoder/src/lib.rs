//! Encoder state ownership with a thin libobs ABI boundary.
#![deny(unsafe_code)]
pub mod encoder_module;
#[allow(unsafe_code)]
pub mod ffi;
pub mod get_name;
pub mod last_error;
pub mod media_getters;
pub mod packet_ownership;
pub mod type_info;
pub mod video_tex_active;

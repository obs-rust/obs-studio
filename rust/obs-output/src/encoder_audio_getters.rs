//! Public audio getters: Tier 1 RED stubs.
use core::ffi::CStr;
pub fn sample_rate(_fields: Option<(i32,bool,u32)>, _media: impl FnOnce()->u32, _log: impl FnOnce(i32,&'static CStr)) -> u32 { todo!("public audio getter port") }
pub fn frame_size(_fields: Option<(i32,usize)>, _log: impl FnOnce(i32,&'static CStr)) -> usize { todo!("public audio getter port") }
pub fn mixer_index(_fields: Option<(i32,usize)>, _log: impl FnOnce(i32,&'static CStr)) -> usize { todo!("public audio getter port") }

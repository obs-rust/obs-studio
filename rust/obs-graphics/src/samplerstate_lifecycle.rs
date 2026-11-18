//! Sampler lifecycle decisions; objects remain opaque and backend-owned.
use std::ffi::CStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    Create,
    Destroy,
}
impl Entry {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Create => "gs_samplerstate_create",
            Self::Destroy => "gs_samplerstate_destroy",
        }
    }
    pub const fn c_name(self) -> &'static CStr {
        match self {
            Self::Create => c"gs_samplerstate_create",
            Self::Destroy => c"gs_samplerstate_destroy",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rejection {
    Context,
    Info,
}
impl Rejection {
    pub const fn message(self) -> &'static CStr {
        match self {
            Self::Context => c"called while not in a graphics context",
            Self::Info => c"Null 'info' parameter",
        }
    }
}
/// Dispatch methods are reached only after the Rust context/argument guards.
pub trait Host {
    type Info: Copy;
    type Sampler: Copy;
    fn has_context(&self) -> bool;
    fn debug(&mut self, entry: Entry, reason: Rejection);
    fn create(&mut self, info: Self::Info) -> Option<Self::Sampler>;
    fn destroy(&mut self, sampler: Self::Sampler);
}
pub fn create<H: Host>(_host: &mut H, _info: Option<H::Info>) -> Option<H::Sampler> {
    todo!()
}
pub fn destroy<H: Host>(_host: &mut H, _sampler: Option<H::Sampler>) {
    todo!()
}

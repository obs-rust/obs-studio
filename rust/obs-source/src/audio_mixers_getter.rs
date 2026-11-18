//! Source audio mixer query over borrowed private fields.
use core::ffi::CStr;

/// Individual reads preserve the C getter's short-circuit order.
pub trait Source {
    fn owns_info_id(&self) -> bool;
    fn output_flags(&self) -> u32;
    fn audio_mixers(&self) -> u32;
}

/// Read the stored mask only for audio-capable sources or owned source IDs.
pub fn get(source: Option<&impl Source>, log: impl FnOnce(i32, &'static CStr)) -> u32 {
    let _ = (source, log);
    todo!()
}

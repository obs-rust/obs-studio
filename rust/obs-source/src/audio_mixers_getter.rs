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
    let Some(source) = source else {
        log(400, c"obs_source_get_audio_mixers: Null 'source' parameter");
        return 0;
    };
    if !source.owns_info_id() && !crate::monitoring::has_audio(source.output_flags()) {
        return 0;
    }
    source.audio_mixers()
}

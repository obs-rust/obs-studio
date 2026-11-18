//! The source layout stays behind the existing properties field adapter.
use crate::configurable::{Source, configurable};
use crate::ffi::source_properties::Fields;
use core::ffi::{CStr, c_char, c_int, c_void};

unsafe extern "C" {
    fn obs_source_properties_instance_fields(source: *const c_void) -> Fields;
    fn blog(level: c_int, format: *const c_char, ...);
}

/// # Safety
/// A non-null source must be live under the original libobs ownership contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_configurable(source: *const c_void) -> bool {
    let source = if source.is_null() {
        None
    } else {
        // SAFETY: NULL was rejected before the private C layout accessor.
        let fields = unsafe { obs_source_properties_instance_fields(source) };
        Some(Source {
            data: !fields.data.is_null(),
            get_properties: fields.legacy.is_some(),
            get_properties2: fields.modern.is_some(),
        })
    };
    configurable(source, |name: &'static CStr| {
        // SAFETY: static strings match the two %s arguments of the format.
        unsafe {
            blog(
                400,
                c"%s: Null '%s' parameter".as_ptr(),
                name.as_ptr(),
                c"source".as_ptr(),
            );
        }
    })
}

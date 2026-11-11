//! Opaque snapshots and callback conversion; private C layouts stay in C.
use crate::output_properties::{self as properties_core, Host};
use core::ffi::{c_char, c_int, c_void};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Fields {
    pub defaults: Option<unsafe extern "C" fn(*mut c_void)>,
    pub properties: Option<unsafe extern "C" fn(*mut c_void) -> *mut c_void>,
    pub data: *mut c_void,
    pub settings: *mut c_void,
}
unsafe extern "C" {
    fn obs_output_properties_type_fields(info: *const c_void) -> Fields;
    fn obs_output_properties_instance_fields(output: *const c_void) -> Fields;
    fn obs_data_create() -> *mut c_void;
    fn obs_data_release(data: *mut c_void);
    fn obs_properties_apply_settings(props: *mut c_void, data: *mut c_void);
    fn blog(level: c_int, format: *const c_char, ...);
}
struct CHost;
impl Host for CHost {
    type Info = Fields;
    type Data = *mut c_void;
    type Props = *mut c_void;
    fn create(&self) -> Self::Data {
        // SAFETY: libobs returns a new owned settings handle.
        unsafe { obs_data_create() }
    }
    fn defaults(&self, info: Fields, data: Self::Data) {
        if let Some(callback) = info.defaults {
            // SAFETY: callback and handle originate in the live C registration.
            unsafe { callback(data) }
        }
    }
    fn has_properties(&self, info: Fields) -> bool {
        info.properties.is_some()
    }
    fn properties(&self, info: Fields, data: Option<Self::Data>) -> Self::Props {
        // SAFETY: safe orchestration calls this only after has_properties; plugin
        // data is borrowed from the live instance, or null for type queries.
        unsafe { info.properties.unwrap()(data.unwrap_or_default()) }
    }
    fn apply(&self, props: &Self::Props, data: Self::Data) {
        // SAFETY: libobs permits a null properties result and borrows settings.
        unsafe { obs_properties_apply_settings(*props, data) }
    }
    fn release(&self, data: Self::Data) {
        // SAFETY: release the temporary settings reference created above.
        unsafe { obs_data_release(data) }
    }
    fn null_output(&self) {
        // SAFETY: fixed format string with no variadic arguments.
        unsafe {
            blog(
                400,
                c"obs_output_properties: Null 'output' parameter".as_ptr(),
            )
        }
    }
}
unsafe fn find(id: *const c_char) -> Option<Fields> {
    // SAFETY: find_output accepts null and otherwise a readable C string.
    unsafe { super::output_type_lookup::find(id) }.map(|info| {
        // SAFETY: registration remains live during this query.
        unsafe { obs_output_properties_type_fields(info.as_ptr()) }
    })
}
/// # Safety
/// `id` is null or a readable NUL-terminated output identifier.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_defaults(id: *const c_char) -> *mut c_void {
    // SAFETY: forwards the caller's readable identifier.
    properties_core::defaults(&CHost, unsafe { find(id) }).unwrap_or_default()
}
/// # Safety
/// `id` is null or a readable NUL-terminated output identifier.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_output_properties(id: *const c_char) -> *mut c_void {
    // SAFETY: forwards the caller's readable identifier.
    properties_core::type_properties(&CHost, unsafe { find(id) }).unwrap_or_default()
}
/// # Safety
/// `output` is null or a live output with live plugin data and settings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_properties(output: *const c_void) -> *mut c_void {
    let fields = if output.is_null() {
        None
    } else {
        // SAFETY: null is rejected before accessing any private C members.
        let fields = unsafe { obs_output_properties_instance_fields(output) };
        Some((fields, fields.data, fields.settings))
    };
    properties_core::properties(&CHost, fields).unwrap_or_default()
}

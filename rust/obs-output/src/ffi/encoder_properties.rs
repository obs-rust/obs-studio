//! C ABI boundary for the encoder property constructors.
use crate::encoder_properties::{Host, construct};
use core::ffi::{c_char, c_void};

type PropertiesFn = unsafe extern "C" fn(*mut c_void) -> *mut c_void;
type Properties2Fn = unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void;
type DefaultsFn = unsafe extern "C" fn(*mut c_void);
type Defaults2Fn = unsafe extern "C" fn(*mut c_void, *mut c_void);

#[repr(C)]
pub struct PropertiesFields {
    pub properties: Option<PropertiesFn>,
    pub properties2: Option<Properties2Fn>,
    pub defaults: Option<DefaultsFn>,
    pub defaults2: Option<Defaults2Fn>,
    pub type_data: *mut c_void,
    pub data: *mut c_void,
    pub settings: *mut c_void,
}

unsafe extern "C" {
    fn find_encoder(id: *const c_char) -> *const c_void;
    fn obs_encoder_properties_type_fields(info: *const c_void) -> PropertiesFields;
    fn obs_encoder_properties_instance_fields(encoder: *const c_void) -> PropertiesFields;
    fn obs_data_create() -> *mut c_void;
    fn obs_data_release(settings: *mut c_void);
    fn obs_properties_apply_settings(props: *mut c_void, settings: *mut c_void);
    fn blog(level: i32, format: *const c_char, ...);
}

// Private borrowed host; only the unsafe constructors below can create it.
struct CProperties(PropertiesFields);
impl Host for CProperties {
    type Settings = *mut c_void;
    type Properties = *mut c_void;
    fn null() -> *mut c_void {
        core::ptr::null_mut()
    }
    fn has_properties(&self) -> bool {
        self.0.properties.is_some()
    }
    fn has_properties2(&self) -> bool {
        self.0.properties2.is_some()
    }
    fn create_settings(&self) -> *mut c_void {
        // SAFETY: returns a new libobs-owned settings reference.
        unsafe { obs_data_create() }
    }
    fn defaults(&self, settings: *mut c_void) {
        if let Some(callback) = self.0.defaults {
            // SAFETY: callback and settings remain live during this synchronous call.
            unsafe { callback(settings) };
        }
    }
    fn defaults2(&self, settings: *mut c_void) {
        if let Some(callback) = self.0.defaults2 {
            // SAFETY: the type-data pointer belongs to this callback table.
            unsafe { callback(settings, self.0.type_data) };
        }
    }
    fn settings(&self) -> *mut c_void {
        self.0.settings
    }
    fn properties(&self, instance: bool) -> *mut c_void {
        let data = if instance {
            self.0.data
        } else {
            core::ptr::null_mut()
        };
        // SAFETY: core dispatch checks presence and the host borrows live plugin data.
        unsafe { self.0.properties.expect("selected callback")(data) }
    }
    fn properties2(&self, instance: bool) -> *mut c_void {
        let data = if instance {
            self.0.data
        } else {
            core::ptr::null_mut()
        };
        // SAFETY: core dispatch checks presence and these pointers belong to the table.
        unsafe { self.0.properties2.expect("selected callback")(data, self.0.type_data) }
    }
    fn apply(&self, props: &*mut c_void, settings: *mut c_void) {
        // SAFETY: callback result and settings obey the libobs contract, including NULL.
        unsafe { obs_properties_apply_settings(*props, settings) };
    }
    fn release(&self, settings: *mut c_void) {
        // SAFETY: the safe core releases only its newly created defaults reference.
        unsafe { obs_data_release(settings) };
    }
}

/// # Safety
/// `id` must be a live NUL-terminated encoder identifier, as required by libobs.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_encoder_properties(id: *const c_char) -> *mut c_void {
    // SAFETY: caller supplies a valid ID and the registered info remains live.
    let info = unsafe { find_encoder(id) };
    let host = if info.is_null() {
        None
    } else {
        // SAFETY: lookup returned a live info table; the adapter only projects fields.
        Some(CProperties(unsafe {
            obs_encoder_properties_type_fields(info)
        }))
    };
    construct(host.as_ref(), false)
}

/// # Safety
/// A non-NULL encoder must remain live throughout callback execution.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_properties(encoder: *const c_void) -> *mut c_void {
    let host = if encoder.is_null() {
        // SAFETY: fixed format string has no variadic arguments.
        unsafe {
            blog(
                400,
                c"obs_encoder_properties: Null 'encoder' parameter".as_ptr(),
            )
        };
        None
    } else {
        // SAFETY: caller guarantees a live encoder; NULL was checked before field access.
        Some(CProperties(unsafe {
            obs_encoder_properties_instance_fields(encoder)
        }))
    };
    construct(host.as_ref(), true)
}

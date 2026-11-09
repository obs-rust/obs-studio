//! Convert the private C layout and callbacks into the safe construction host.
use crate::properties::{self, Host};
use core::ffi::{c_char, c_int, c_void};

type Legacy = unsafe extern "C" fn(*mut c_void) -> *mut c_void;
type Modern = unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void;
type Defaults = unsafe extern "C" fn(*mut c_void);
type Defaults2 = unsafe extern "C" fn(*mut c_void, *mut c_void);

#[repr(C)]
pub struct Fields {
    pub legacy: Option<Legacy>,
    pub modern: Option<Modern>,
    pub defaults: Option<Defaults>,
    pub defaults2: Option<Defaults2>,
    pub type_data: *mut c_void,
    pub data: *mut c_void,
    pub settings: *const *mut c_void,
}

unsafe extern "C" {
    fn obs_source_properties_info_fields(info: *const c_void) -> Fields;
    fn obs_source_properties_instance_fields(source: *const c_void) -> Fields;
    fn get_source_info(id: *const c_char) -> *const c_void;
    fn obs_data_create() -> *mut c_void;
    fn obs_data_release(settings: *mut c_void);
    fn obs_properties_apply_settings(props: *mut c_void, settings: *mut c_void);
    fn blog(level: c_int, format: *const c_char, ...);
}

struct Callbacks(Fields);

impl Host for Callbacks {
    type Properties = *mut c_void;
    type Settings = *mut c_void;
    fn modern(&self) -> bool {
        self.0.modern.is_some()
    }
    fn legacy(&self) -> bool {
        self.0.legacy.is_some()
    }
    fn defaults(&mut self) -> Self::Settings {
        // SAFETY: callbacks and type data originate in the live C source info.
        unsafe {
            let settings = obs_data_create();
            if let Some(callback) = self.0.defaults2 {
                callback(self.0.type_data, settings);
            } else if let Some(callback) = self.0.defaults {
                callback(settings);
            }
            settings
        }
    }
    fn construct(&mut self, modern: bool) -> Self::Properties {
        // SAFETY: the safe core selects only a present callback; C owns its arguments.
        unsafe {
            if modern {
                self.0
                    .modern
                    .map_or(core::ptr::null_mut(), |f| f(self.0.data, self.0.type_data))
            } else {
                self.0
                    .legacy
                    .map_or(core::ptr::null_mut(), |f| f(self.0.data))
            }
        }
    }
    fn apply(&mut self, props: &Self::Properties, defaults: Option<&Self::Settings>) {
        // SAFETY: libobs accepts null properties; settings are borrowed for this call.
        unsafe {
            obs_properties_apply_settings(
                *props,
                defaults.copied().unwrap_or_else(|| *self.0.settings),
            );
        }
    }
    fn release(&mut self, settings: Self::Settings) {
        // SAFETY: this is the reference returned by defaults(), released exactly once.
        unsafe {
            obs_data_release(settings);
        }
    }
}

/// # Safety
/// `id` follows the existing C lookup contract and remains valid during the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_source_properties(id: *const c_char) -> *mut c_void {
    // SAFETY: the caller provides the original lookup contract.
    let info = unsafe { get_source_info(id) };
    if info.is_null() {
        return core::ptr::null_mut();
    }
    // SAFETY: null was rejected before C member access; registry owns the info.
    properties::construct(
        &mut Callbacks(unsafe { obs_source_properties_info_fields(info) }),
        true,
    )
}

/// # Safety
/// A non-null source must be live under the original libobs ownership contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_properties(source: *const c_void) -> *mut c_void {
    if source.is_null() {
        // SAFETY: both strings are static and the variadic format matches its arguments.
        unsafe {
            blog(
                400,
                c"%s: Null '%s' parameter".as_ptr(),
                c"obs_source_properties".as_ptr(),
                c"source".as_ptr(),
            );
        }
        return core::ptr::null_mut();
    }
    // SAFETY: null was rejected before taking C member addresses.
    let fields = unsafe { obs_source_properties_instance_fields(source) };
    if fields.data.is_null() {
        return core::ptr::null_mut();
    }
    properties::construct(&mut Callbacks(fields), false)
}

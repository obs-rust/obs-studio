//! Private field-pointer traversal; all decisions remain in the safe core.
use crate::core_texture_activity::activity;
use std::ffi::c_void;
use std::ptr::NonNull;

#[repr(C)]
pub struct Fields {
    pub nv12: *const bool,
    pub p010: *const bool,
}

unsafe extern "C" {
    fn obs_core_texture_activity_core() -> *const c_void;
    fn obs_core_texture_activity_canvas(core: *const c_void) -> *const *mut c_void;
    fn obs_core_texture_activity_mix(canvas: *const c_void) -> *const *mut c_void;
    fn obs_core_texture_activity_fields(mix: *const c_void) -> Fields;
}

unsafe fn snapshot() -> (bool, bool) {
    // SAFETY: obtaining the global pointer does not dereference it. Each
    // subsequent accessor receives a non-null live owner, and only then forms
    // member addresses. The caller excludes startup/shutdown/reconfiguration.
    let flags = unsafe {
        NonNull::new(obs_core_texture_activity_core().cast_mut()).and_then(|core| {
            NonNull::new(*obs_core_texture_activity_canvas(core.as_ptr())).and_then(|canvas| {
                NonNull::new(*obs_core_texture_activity_mix(canvas.as_ptr())).map(|mix| {
                    let fields = obs_core_texture_activity_fields(mix.as_ptr());
                    (&*fields.nv12, &*fields.p010)
                })
            })
        })
    };
    activity(flags)
}

/// Report the main video mix's stored NV12 texture flag.
///
/// # Safety
/// Startup, shutdown and video reconfiguration must not race this read.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_nv12_tex_active() -> bool {
    // SAFETY: forwarded lifecycle preconditions keep the private fields live.
    unsafe { snapshot() }.0
}

/// Report the main video mix's stored P010 texture flag.
///
/// # Safety
/// Startup, shutdown and video reconfiguration must not race this read.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_p010_tex_active() -> bool {
    // SAFETY: forwarded lifecycle preconditions keep the private fields live.
    unsafe { snapshot() }.1
}

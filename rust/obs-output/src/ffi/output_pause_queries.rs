//! Private field conversion, atomic load and the original C mutex boundary.
use core::ffi::{CStr, c_char, c_int, c_void};
use core::sync::atomic::{AtomicBool, Ordering};

#[repr(C)]
pub struct Fields {
    pub paused: *const bool,
    pub offset: *const u64,
    pub mutex: *mut c_void,
}

unsafe extern "C" {
    fn obs_output_pause_queries_fields(output: *mut c_void) -> Fields;
    fn obs_output_pause_queries_lock(mutex: *mut c_void);
    fn obs_output_pause_queries_unlock(mutex: *mut c_void);
    fn blog(level: c_int, format: *const c_char, ...);
}

fn log(name: &'static CStr) {
    // SAFETY: literal format and static, terminated function name.
    unsafe { blog(400, c"%s: Null 'output' parameter".as_ptr(), name.as_ptr()) }
}

unsafe fn paused_value(output: *const c_void) -> Option<bool> {
    if output.is_null() {
        return None;
    }
    // SAFETY: NULL is rejected before taking member addresses. The caller
    // retains the output and C accesses its paused field atomically too.
    unsafe {
        let fields = obs_output_pause_queries_fields(output.cast_mut());
        Some(AtomicBool::from_ptr(fields.paused.cast_mut()).load(Ordering::SeqCst))
    }
}

unsafe fn offset_value(output: *mut c_void) -> Option<u64> {
    if output.is_null() {
        return None;
    }
    // SAFETY: the caller retains a live output and initialized pause mutex.
    // The adapter only takes member addresses; read under the same mutex as C.
    unsafe {
        let fields = obs_output_pause_queries_fields(output);
        obs_output_pause_queries_lock(fields.mutex);
        let value = *fields.offset;
        obs_output_pause_queries_unlock(fields.mutex);
        Some(value)
    }
}

/// # Safety
/// A non-null output must be live with its atomic paused flag readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_paused(output: *const c_void) -> bool {
    // SAFETY: caller retains the output; paused_value rejects NULL first.
    crate::output_pause_queries::paused(unsafe { paused_value(output) }, log)
}

/// # Safety
/// A non-null output must remain live with an initialized pause mutex.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_get_pause_offset(output: *mut c_void) -> u64 {
    // SAFETY: caller retains the output; offset_value rejects NULL first.
    crate::output_pause_queries::offset(unsafe { offset_value(output) }, log)
}

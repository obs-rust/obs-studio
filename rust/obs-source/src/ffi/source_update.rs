//! `obs_source_update` over the C-owned source: opaque-pointer conversion
//! only; the decisions and their order live in the safe core.
use crate::update::{self, Source};
use core::ffi::{c_char, c_int, c_void};
use obs_util::platform::CLongAtomic;

unsafe extern "C" {
    fn obs_source_update_settings(source: *const c_void) -> *mut c_void;
    fn obs_source_update_output_flags(source: *const c_void) -> u32;
    fn obs_source_update_defer_count(source: *mut c_void) -> *const CLongAtomic;
    fn obs_source_update_can_update(source: *const c_void) -> bool;
    fn obs_source_update_call(source: *mut c_void);
    fn obs_source_update_signal(source: *mut c_void);
    fn obs_data_apply(target: *mut c_void, apply_data: *mut c_void);
    fn blog(level: c_int, format: *const c_char, ...);
}

/// A non-NULL live source and the caller's non-NULL incoming settings.
struct Borrowed {
    source: *mut c_void,
    settings: *mut c_void,
}

impl Source for Borrowed {
    fn apply_settings(&self) {
        // SAFETY: the core calls this only for a non-NULL source and
        // non-NULL incoming settings; the stored object may be NULL, which
        // obs_data_apply() accepts.
        unsafe { obs_data_apply(obs_source_update_settings(self.source), self.settings) }
    }

    fn output_flags(&self) -> u32 {
        // SAFETY: a non-NULL live source.
        unsafe { obs_source_update_output_flags(self.source) }
    }

    fn defer_update_count(&self) -> &CLongAtomic {
        // SAFETY: a non-NULL live source keeps its aligned native long
        // alive for the call; C accesses it atomically too.
        unsafe { &*obs_source_update_defer_count(self.source) }
    }

    fn can_update(&self) -> bool {
        // SAFETY: a non-NULL live source.
        unsafe { obs_source_update_can_update(self.source) }
    }

    fn call_update(&self) {
        // SAFETY: only after can_update(); nothing of the source is held
        // across the plugin's code, which may reenter.
        unsafe { obs_source_update_call(self.source) }
    }

    fn signal_update(&self) {
        // SAFETY: a non-NULL live source and its signal handlers; the
        // handlers run synchronously and may reenter.
        unsafe { obs_source_update_signal(self.source) }
    }
}

/// # Safety
/// A non-NULL source must be live, with the settings object, data and
/// update callback its type gave it; `settings` is NULL or a live data
/// object.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_update(source: *mut c_void, settings: *mut c_void) {
    let borrowed = Borrowed { source, settings };
    update::update(
        (!source.is_null()).then_some(&borrowed),
        !settings.is_null(),
        |name| {
            // SAFETY: both strings are static and NUL terminated.
            unsafe { blog(400, c"%s: Null 'source' parameter".as_ptr(), name.as_ptr()) }
        },
    );
}

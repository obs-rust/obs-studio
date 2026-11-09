//! Private source context ABI; all layout access stays in the C adapter.
use core::ffi::{c_char, c_void};
use obs_source::init_context::initialize;

unsafe extern "C" {
    fn obs_source_context_initialize(
        source: *mut c_void,
        settings: *mut c_void,
        name: *const c_char,
        uuid: *const c_char,
        hotkeys: *mut c_void,
        private: bool,
    ) -> bool;
    fn obs_source_context_register(source: *mut c_void, signals: *const *const c_char) -> bool;
}

/// Initialize a live source context, preserving borrowed input identities.
///
/// # Safety
/// `source` must be a writable source and other inputs must satisfy
/// `obs_context_data_init`'s C contract. No input ownership is transferred here.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_init_context(
    source: *mut c_void,
    settings: *mut c_void,
    name: *const c_char,
    uuid: *const c_char,
    hotkeys: *mut c_void,
    private: bool,
) -> bool {
    if source.is_null() {
        return false;
    }
    initialize(
        (settings, name, uuid, hotkeys, private),
        |(settings, name, uuid, hotkeys, private)| {
            // SAFETY: nonnull source and borrowed arguments satisfy the caller's contract.
            unsafe { obs_source_context_initialize(source, settings, name, uuid, hotkeys, private) }
        },
        |signals| {
            let mut array = [core::ptr::null(); obs_source::init_context::SIGNALS.len() + 1];
            for (slot, signal) in array.iter_mut().zip(signals) {
                *slot = signal.as_ptr();
            }
            // SAFETY: live source; 40 static declarations and a null terminator remain
            // valid for the synchronous registration call, which does not retain the array.
            unsafe { obs_source_context_register(source, array.as_ptr()) }
        },
    )
}

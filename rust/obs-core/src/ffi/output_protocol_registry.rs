//! Borrow the private C registry without mirroring its layout or owning strings.
use crate::output_protocol_registry;
use std::ffi::{CStr, c_char};

unsafe extern "C" {
    fn obs_core_output_protocol_registry_fields(count: *mut usize) -> *const *mut c_char;
}

unsafe fn protocols<'a>() -> &'a [*mut c_char] {
    let mut count = 0;
    // SAFETY: callers guarantee an initialized core and synchronized live registry.
    let array = unsafe { obs_core_output_protocol_registry_fields(&mut count) };
    if count == 0 {
        &[]
    } else {
        // SAFETY: the C darray contains count initialized, live pointer entries.
        unsafe { std::slice::from_raw_parts(array, count) }
    }
}

/// Test whether a whole protocol name is registered.
///
/// # Safety
/// A non-NULL input is a terminated string and requires an initialized core.
/// The registry and its strings must remain live and unmodified during the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_is_output_protocol_registered(protocol: *const c_char) -> bool {
    if protocol.is_null() {
        return false;
    }
    // SAFETY: input and registry strings are terminated and live by the contract.
    unsafe {
        output_protocol_registry::registered(
            Some(CStr::from_ptr(protocol).to_bytes()),
            protocols()
                .iter()
                .map(|entry| CStr::from_ptr(*entry).to_bytes()),
        )
    }
}

/// Return a borrowed registry string at `index`, preserving output on failure.
///
/// # Safety
/// A non-NULL output points to a writable pointer slot and requires an initialized
/// core whose registry remains live and unmodified during the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_enum_output_protocols(
    index: usize,
    protocol: *mut *mut c_char,
) -> bool {
    if protocol.is_null() {
        return false;
    }
    // SAFETY: the registry is initialized and synchronized by the caller.
    let Some(entry) = output_protocol_registry::enumerate(unsafe { protocols() }, index) else {
        return false;
    };
    // SAFETY: the caller provides a writable output slot. Ownership stays with C.
    unsafe { protocol.write(entry) };
    true
}

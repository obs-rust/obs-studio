//! Opaque context/device boundary; duplicated memory uses the libobs allocator.
use crate::indexbuffer_create::{self, Backend};
use obs_util::ffi::bmem::bmemdup;
use std::ffi::{c_int, c_void};
use std::ptr::NonNull;

unsafe extern "C" {
    fn obs_graphics_indexbuffer_context() -> *mut c_void;
    fn obs_graphics_indexbuffer_dispatch(
        graphics: *mut c_void,
        kind: c_int,
        indices: *mut c_void,
        count: usize,
        flags: u32,
    ) -> *mut c_void;
}

struct Device(NonNull<c_void>);

impl Backend for Device {
    type Input = NonNull<c_void>;
    type Buffer = *mut c_void;

    fn duplicate(&mut self, input: Self::Input, bytes: usize) -> Self::Input {
        // SAFETY: gs_indexbuffer_create's caller supplies readable input for
        // the C size_t product, exactly as required by the original bmemdup.
        // bmemdup aborts on allocation failure and never returns null.
        unsafe { NonNull::new_unchecked(bmemdup(input.as_ptr(), bytes)) }
    }

    fn create(
        &mut self,
        kind: i32,
        input: Option<Self::Input>,
        count: usize,
        flags: u32,
    ) -> Self::Buffer {
        // SAFETY: the context was validated and remains live for this call.
        // The backend consumes this allocation even when returning null.
        unsafe {
            obs_graphics_indexbuffer_dispatch(
                self.0.as_ptr(),
                kind,
                input.map_or(std::ptr::null_mut(), NonNull::as_ptr),
                count,
                flags,
            )
        }
    }
}

/// Create an index buffer through the current thread's graphics device.
///
/// # Safety
/// The context must remain live. Non-null indices must satisfy the backend's
/// input/ownership contract and be readable for count times the index width
/// when duplication is requested. Transferred memory must use libobs allocation.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_indexbuffer_create(
    kind: c_int,
    indices: *mut c_void,
    count: usize,
    flags: u32,
) -> *mut c_void {
    // SAFETY: the C adapter owns validation/logging and exposes no private layout.
    let mut device = NonNull::new(unsafe { obs_graphics_indexbuffer_context() }).map(Device);
    indexbuffer_create::create(device.as_mut(), kind, NonNull::new(indices), count, flags)
        .unwrap_or(std::ptr::null_mut())
}

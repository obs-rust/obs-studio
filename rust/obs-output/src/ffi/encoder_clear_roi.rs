//! Real private field addresses and the existing shared C ROI mutex.
use crate::encoder_clear_roi::{self, RoiState};
use core::ffi::{c_char, c_int, c_void};

#[repr(C)]
pub struct ClearRoiFields {
    pub count: *mut usize,
    pub increment: *mut u32,
    pub mutex: *mut c_void,
    pub lock: unsafe extern "C" fn(*mut c_void),
    pub unlock: unsafe extern "C" fn(*mut c_void),
}
unsafe extern "C" {
    fn obs_encoder_clear_roi_fields(encoder: *mut c_void) -> ClearRoiFields;
    fn blog(level: c_int, format: *const c_char, ...);
}
struct Borrowed(ClearRoiFields);
impl RoiState for Borrowed {
    fn count(&self) -> usize {
        // SAFETY: the caller provides the same lifetime/synchronization as C's pre-lock read.
        unsafe { self.0.count.read() }
    }
    fn with_locked_fields(&mut self, update: impl FnOnce(&mut usize, &mut u32)) {
        // SAFETY: real C callbacks lock the same live mutex used by add/enum.
        unsafe { (self.0.lock)(self.0.mutex) };
        struct Unlock(*mut c_void, unsafe extern "C" fn(*mut c_void));
        impl Drop for Unlock {
            fn drop(&mut self) {
                // SAFETY: the scope owns this mutex lock and drops borrowed fields first.
                unsafe { (self.1)(self.0) };
            }
        }
        let _guard = Unlock(self.0.mutex, self.0.unlock);
        // SAFETY: distinct fields of the live encoder are exclusively borrowed under its ROI mutex.
        unsafe { update(&mut *self.0.count, &mut *self.0.increment) };
    }
}
/// # Safety
/// A non-null encoder must be live, with its pre-lock count readable under the
/// same synchronization required by the original C API. ROI mutations must use
/// its shared ROI mutex. The call must not occur within an ROI enum callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_clear_roi(encoder: *mut c_void) {
    // Check NULL before even borrowing field addresses from the private adapter.
    let mut borrowed = if encoder.is_null() {
        None
    } else {
        // SAFETY: caller guarantees a live encoder; NULL was checked above.
        Some(Borrowed(unsafe { obs_encoder_clear_roi_fields(encoder) }))
    };
    encoder_clear_roi::clear_roi(borrowed.as_mut(), || {
        // SAFETY: static NUL-terminated format with no conversion arguments.
        unsafe {
            blog(
                400,
                c"obs_encoder_clear_roi: Null 'encoder' parameter".as_ptr(),
            )
        };
    });
}

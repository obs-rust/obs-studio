//! Opaque C display and borrowed field pointers; no display or mutex layout.
use crate::display_size::{self, SizeAccess};
use core::ffi::c_void;

/// Private C field-pointer seam, measured against the real header in Tier 3.
#[repr(C)]
pub struct SizeFields {
    pub cx: *mut u32,
    pub cy: *mut u32,
}
unsafe extern "C" {
    fn obs_display_size_fields(display: *mut c_void) -> SizeFields;
    fn obs_display_size_lock(display: *mut c_void);
    fn obs_display_size_unlock(display: *mut c_void);
}
struct Access {
    display: *mut c_void,
    width: *mut u32,
    height: *mut u32,
    fields: SizeFields,
}
impl SizeAccess for Access {
    fn set_width(&mut self, value: u32) {
        // SAFETY: the caller guarantees writable output storage. Raw stores
        // preserve C aliasing (including aliasing a field of the display).
        unsafe { self.width.write(value) };
    }
    fn set_height(&mut self, value: u32) {
        // SAFETY: same contract as width; no Rust reference aliases this store.
        unsafe { self.height.write(value) };
    }
    fn has_display(&self) -> bool {
        !self.display.is_null()
    }
    fn lock(&mut self) {
        // SAFETY: called only for a live non-NULL C display. C takes the exact
        // existing draw_info_mutex; fields are projected only after acquiring it.
        unsafe {
            obs_display_size_lock(self.display);
            self.fields = obs_display_size_fields(self.display);
        }
    }
    fn width(&self) -> u32 {
        // SAFETY: size calls this only while the C draw_info_mutex is held.
        unsafe { self.fields.cx.read() }
    }
    fn height(&self) -> u32 {
        // SAFETY: size calls this only while the C draw_info_mutex is held.
        unsafe { self.fields.cy.read() }
    }
    fn unlock(&mut self) {
        // SAFETY: the same display's mutex was acquired by lock on this thread.
        unsafe { obs_display_size_unlock(self.display) };
    }
}
/// Query current display dimensions, zeroing both outputs for NULL.
///
/// # Safety
/// Outputs must be writable u32 locations; they may alias each other or the
/// display fields as in C. A non-NULL display must be a live C display with an
/// initialized draw_info_mutex. Its lifetime must cover this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_display_size(display: *mut c_void, width: *mut u32, height: *mut u32) {
    display_size::size(&mut Access {
        display,
        width,
        height,
        fields: SizeFields {
            cx: core::ptr::null_mut(),
            cy: core::ptr::null_mut(),
        },
    });
}

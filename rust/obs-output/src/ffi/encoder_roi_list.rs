//! Borrow real C field addresses; all list policy lives in the safe core.
use crate::encoder_roi::Roi;
use crate::encoder_roi_list::{self, Entries, RoiList};
use core::ffi::{c_char, c_int, c_void};
use obs_util::ffi::darray::{darray, darray_push_back};

#[repr(C)]
pub struct RoiListFields {
    pub rois: *mut darray,
    pub increment: *mut u32,
    pub caps: *const u32,
    pub scaled_width: *const u32,
    pub scaled_height: *const u32,
    pub media: *const *mut c_void,
    pub mutex: *mut c_void,
    pub lock: unsafe extern "C" fn(*mut c_void),
    pub unlock: unsafe extern "C" fn(*mut c_void),
    pub video_size: unsafe extern "C" fn(*mut c_void, *mut u32, *mut u32),
}
unsafe extern "C" {
    fn obs_encoder_roi_list_fields(encoder: *mut c_void) -> RoiListFields;
    fn blog(level: c_int, format: *const c_char, ...);
}
struct Borrowed(RoiListFields);
struct Locked<'a>(&'a mut Borrowed);
impl Entries for Locked<'_> {
    fn append(&mut self, roi: &Roi) {
        // SAFETY: the live encoder owns this bmem darray, exclusively under its ROI mutex.
        unsafe {
            darray_push_back(
                size_of::<Roi>(),
                self.0.0.rois,
                core::ptr::from_ref(roi).cast(),
            )
        };
    }
    fn increment(&mut self) -> &mut u32 {
        // SAFETY: the encoder's distinct increment field is exclusively borrowed under its mutex.
        unsafe { &mut *self.0.0.increment }
    }
    fn count(&self) -> usize {
        // SAFETY: live encoder storage, read only while locked.
        unsafe { (*self.0.0.rois).num }
    }
    fn visit(&mut self, index: usize, callback: &mut dyn FnMut(&mut Roi)) {
        // SAFETY: the core visits only indices below count under the shared mutex.
        // No slice or encoder-wide reference survives the callback.
        unsafe { callback(&mut *(*self.0.0.rois).array.cast::<Roi>().add(index)) };
    }
}
impl RoiList for Borrowed {
    fn caps(&self) -> u32 {
        // SAFETY: caller guarantees the original C API's live encoder and synchronization.
        unsafe { self.0.caps.read() }
    }
    fn scaled_size(&self) -> (u32, u32) {
        // SAFETY: same live fields and pre-lock synchronization as C.
        unsafe { (self.0.scaled_width.read(), self.0.scaled_height.read()) }
    }
    fn video_size(&self) -> (u32, u32) {
        let (mut width, mut height) = (0, 0);
        // SAFETY: the live media field is read only here, with C's synchronization;
        // real C video getters accept its current value, including NULL.
        unsafe { (self.0.video_size)(self.0.media.read(), &mut width, &mut height) };
        (width, height)
    }
    fn with_locked(&mut self, action: impl FnOnce(&mut dyn Entries)) {
        // SAFETY: callback locks this live encoder's existing shared ROI mutex.
        unsafe { (self.0.lock)(self.0.mutex) };
        struct Unlock(*mut c_void, unsafe extern "C" fn(*mut c_void));
        impl Drop for Unlock {
            fn drop(&mut self) {
                // SAFETY: scope owns the lock and drops all field borrows first.
                unsafe { (self.1)(self.0) };
            }
        }
        let _guard = Unlock(self.0.mutex, self.0.unlock);
        action(&mut Locked(self));
    }
}
/// # Safety
/// A non-null encoder must be live with C's pre-lock synchronization. When
/// encoder is non-null, a non-null ROI must be readable for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_add_roi(encoder: *mut c_void, roi: *const Roi) -> bool {
    let mut fields = if encoder.is_null() {
        None
    } else {
        // SAFETY: caller guarantees a live encoder.
        Some(Borrowed(unsafe { obs_encoder_roi_list_fields(encoder) }))
    };
    let roi = if fields.is_none() {
        None
    } else {
        // SAFETY: non-null ROI is readable by contract; NULL remains None.
        unsafe { roi.as_ref() }
    };
    encoder_roi_list::add_roi(fields.as_mut(), roi, || {
        // SAFETY: static terminated format has no conversion arguments.
        unsafe {
            blog(
                400,
                c"obs_encoder_add_roi: Null 'encoder' parameter".as_ptr(),
            )
        };
    })
}
/// # Safety
/// A non-null encoder must be live with C's synchronization. A callback must
/// be callable for every visited ROI and must not recursively lock its encoder's
/// ROI mutex. It may mutate the supplied ROI; a scaled ROI expires on return.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_enum_roi(
    encoder: *mut c_void,
    callback: Option<unsafe extern "C" fn(*mut c_void, *mut Roi)>,
    param: *mut c_void,
) {
    let mut fields = if encoder.is_null() {
        None
    } else {
        // SAFETY: caller guarantees a live encoder.
        Some(Borrowed(unsafe { obs_encoder_roi_list_fields(encoder) }))
    };
    encoder_roi_list::enum_roi(
        fields.as_mut(),
        |roi| {
            if let Some(callback) = callback {
                // SAFETY: caller's callback and context are valid; the core supplies
                // either a locked live entry or its temporary scaled copy.
                unsafe { callback(param, roi) };
            }
        },
        || {
            // SAFETY: static terminated format has no conversion arguments.
            unsafe {
                blog(
                    400,
                    c"obs_encoder_enum_roi: Null 'encoder' parameter".as_ptr(),
                )
            };
        },
    );
}

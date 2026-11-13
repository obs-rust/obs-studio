//! Preserve the canvas enumerator, callback and parameter without ownership changes.
use core::ffi::c_void;

/// Original C callback, nullable when the downstream enumerator never calls it.
pub type EnumProc = Option<unsafe extern "C" fn(*mut c_void, *mut c_void) -> bool>;

/// Private C field layout, measured against the real adapter by the oracle.
#[repr(C)]
pub struct Fields {
    pub canvas: *mut c_void,
    pub enumerate: unsafe extern "C" fn(*mut c_void, EnumProc, *mut c_void),
}

unsafe extern "C" {
    fn obs_enum_scenes_fields() -> Fields;
}

/// # Safety
/// The core and main canvas must be live. Callback/parameter and synchronization
/// must satisfy the unchanged obs_canvas_enum_scenes contract. A callback must
/// not unwind across the C boundary.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_enum_scenes(callback: EnumProc, param: *mut c_void) {
    // SAFETY: caller retains the initialized core and current main canvas.
    let fields = unsafe { obs_enum_scenes_fields() };
    crate::enum_scenes::enum_scenes(fields.canvas, callback, param, |c, cb, p| {
        // SAFETY: forward exactly the borrowed canvas and original arguments;
        // the existing C enumerator owns locking and temporary source references.
        unsafe { (fields.enumerate)(c, cb, p) };
    });
}

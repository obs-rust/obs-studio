//! Private-layout bridge; the safe core owns all setter gates.
use crate::output_mixer_setters::{Decision, decide};
use core::ffi::{c_char, c_int, c_void};
use core::sync::atomic::{AtomicBool, Ordering};
#[repr(C)]
pub struct Fields {
    pub mask: *mut usize,
    pub active: *mut bool,
    pub name: *const c_char,
    pub flags: u32,
}
unsafe extern "C" {
    fn obs_output_mixer_setters_fields(output: *mut c_void) -> Fields;
    fn blog(level: c_int, format: *const c_char, ...);
}
unsafe fn set(output: *mut c_void, value: usize, single: bool) {
    let name = if single {
        c"obs_output_set_mixer"
    } else {
        c"obs_output_set_mixers"
    };
    // SAFETY: reject NULL before forming any private member addresses.
    let fields = if output.is_null() {
        None
    } else {
        Some(unsafe { obs_output_mixer_setters_fields(output) })
    };
    let decision = decide(
        fields.is_some(),
        fields
            .as_ref()
            .is_some_and(|f| f.flags & crate::video_checks::OBS_OUTPUT_ENCODED != 0),
        single.then_some(value),
        || {
            fields.as_ref().is_some_and(|f| {
                // SAFETY: live C bool accessed with the original sequentially consistent atomic semantics.
                unsafe { AtomicBool::from_ptr(f.active).load(Ordering::SeqCst) }
            })
        },
        value,
    );
    // SAFETY: literals have matching argument types; non-null output fields remain live throughout the call.
    unsafe {
        match (decision, fields) {
            (Decision::Null, _) => {
                blog(400, c"%s: Null 'output' parameter".as_ptr(), name.as_ptr())
            }
            (Decision::Encoded, Some(f)) => blog(
                200,
                c"Output '%s': Tried to use %s on an encoded output".as_ptr(),
                f.name,
                name.as_ptr(),
            ),
            (Decision::InvalidIndex, Some(f)) => blog(
                200,
                c"output '%s': Ignoring mixer index %zu, it must be below %d".as_ptr(),
                f.name,
                value,
                6_i32,
            ),
            (Decision::Set(mask), Some(f)) => *f.mask = mask,
            _ => {}
        }
    }
}
/// # Safety
/// Non-null output must be live, with exclusive access to its mixer mask.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_set_mixer(output: *mut c_void, mixer_idx: usize) {
    // SAFETY: caller supplies the original setter contract.
    unsafe { set(output, mixer_idx, true) }
}
/// # Safety
/// Non-null output must be live, with exclusive access to its mixer mask.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_set_mixers(output: *mut c_void, mixers: usize) {
    // SAFETY: caller supplies the original setter contract.
    unsafe { set(output, mixers, false) }
}

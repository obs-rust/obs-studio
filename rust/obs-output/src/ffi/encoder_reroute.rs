//! Private encoder layout and plugin callback boundary.
use crate::encoder_reroute::{self, Environment};
use core::ffi::{c_char, c_int, c_void};

#[repr(C)]
pub struct RerouteFields {
    pub active: *mut c_void,
    pub replacement: *const c_void,
    pub size: usize,
    pub original_type: c_int,
    pub replacement_type: c_int,
    pub original_codec: *const c_char,
    pub replacement_codec: *const c_char,
    pub settings: *mut c_void,
    pub create: unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void,
}
unsafe extern "C" {
    fn obs_encoder_reroute_fields(encoder: *mut c_void, info: *const c_void) -> RerouteFields;
    fn obs_encoder_reroute_allowed() -> bool;
    fn find_encoder(id: *const c_char) -> *const c_void;
    fn astrcmpi(a: *const c_char, b: *const c_char) -> c_int;
    fn blog(level: c_int, format: *const c_char, ...);
}
struct Boundary {
    encoder: *mut c_void,
    id: *const c_char,
}
impl Environment for Boundary {
    type Candidate = RerouteFields;
    type Output = *mut c_void;
    fn encoder_present(&self) -> bool {
        !self.encoder.is_null()
    }
    fn id_present(&self) -> bool {
        !self.id.is_null()
    }
    fn null_parameter(&mut self, encoder: bool) {
        // SAFETY: static NUL-terminated strings and matching variadic format.
        unsafe {
            blog(
                400,
                c"%s: Null '%s' parameter".as_ptr(),
                c"obs_encoder_reroute".as_ptr(),
                if encoder {
                    c"encoder".as_ptr()
                } else {
                    c"reroute_id".as_ptr()
                },
            );
        }
    }
    fn allowed(&self) -> bool {
        // SAFETY: reads the calling thread's C create-only permission.
        unsafe { obs_encoder_reroute_allowed() }
    }
    fn lookup(&self) -> Option<RerouteFields> {
        // SAFETY: core validated ID; caller retains its string and registry.
        let info = unsafe { find_encoder(self.id) };
        if info.is_null() {
            return None;
        }
        // SAFETY: core validated encoder before any field accessor; registered info is live.
        Some(unsafe { obs_encoder_reroute_fields(self.encoder, info) })
    }
    fn kinds(&self, fields: &RerouteFields) -> (i32, i32) {
        (fields.original_type, fields.replacement_type)
    }
    fn codecs_equal(&self, fields: &RerouteFields) -> bool {
        // SAFETY: registered codecs are live C strings (or NULL, supported by astrcmpi).
        unsafe { astrcmpi(fields.original_codec, fields.replacement_codec) == 0 }
    }
    fn install_create(&mut self, fields: RerouteFields) -> *mut c_void {
        // SAFETY: C adapter supplies distinct complete info objects of the same size.
        // The caller owns the mutable encoder for this create callback. Copying bytes
        // preserves all callback fields and padding without mirroring the C table.
        unsafe {
            core::ptr::copy_nonoverlapping(
                fields.replacement.cast::<u8>(),
                fields.active.cast::<u8>(),
                fields.size,
            );
        }
        // SAFETY: a registered encoder has a valid create callback; invoke only after
        // installing the table, with the current settings and the original encoder.
        unsafe { (fields.create)(fields.settings, self.encoder) }
    }
}
/// # Safety
/// Non-NULL arguments must be live encoder/string handles. The registry and
/// encoder must remain valid and externally synchronized through plugin create.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_create_rerouted(
    encoder: *mut c_void,
    reroute_id: *const c_char,
) -> *mut c_void {
    encoder_reroute::create(&mut Boundary {
        encoder,
        id: reroute_id,
    })
    .unwrap_or(core::ptr::null_mut())
}

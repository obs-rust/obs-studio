//! Private encoder layout and plugin callback boundary. The registry lookup
//! and the initializer's create-only permission stay in libobs.
use crate::encoder_reroute::{self, Environment};
use core::ffi::{c_char, c_int, c_void};
use obs_util::ffi::dstr::astrcmpi;
use obs_util::ffi::encoder_type_lookup::find_encoder;

/// An encoder's `create` callback.
pub type Create = unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void;

/// Borrowed view of `struct obs_encoder_reroute_fields` (obs-internal.h).
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
    pub create: Option<Create>,
}

unsafe extern "C" {
    fn obs_encoder_reroute_fields(encoder: *mut c_void, info: *const c_void) -> RerouteFields;
    fn obs_encoder_reroute_allowed() -> bool;
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
        // SAFETY: static NUL-terminated strings and a matching variadic format.
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
        // SAFETY: reads the calling thread's create-only permission.
        unsafe { obs_encoder_reroute_allowed() }
    }
    fn lookup(&self) -> Option<RerouteFields> {
        // SAFETY: the core checked the ID is non-NULL; the caller keeps the
        // string and the registry live, as for the original lookup.
        let info = unsafe { find_encoder(self.id) };
        if info.is_null() {
            return None;
        }
        // SAFETY: the core checked the encoder is non-NULL; a registered info is live.
        Some(unsafe { obs_encoder_reroute_fields(self.encoder, info) })
    }
    fn kinds(&self, fields: &RerouteFields) -> (i32, i32) {
        (fields.original_type, fields.replacement_type)
    }
    fn codecs_equal(&self, fields: &RerouteFields) -> bool {
        // SAFETY: registered codecs are live C strings or NULL, which astrcmpi takes.
        unsafe { astrcmpi(fields.replacement_codec, fields.original_codec) == 0 }
    }
    fn install_create(&mut self, fields: RerouteFields) -> *mut c_void {
        // SAFETY: the adapter gives two distinct, complete info tables of
        // `size` bytes; the caller owns the encoder during its create callback.
        // A byte copy keeps every callback field without mirroring the table.
        unsafe {
            core::ptr::copy_nonoverlapping(
                fields.replacement.cast::<u8>(),
                fields.active.cast::<u8>(),
                fields.size,
            );
        }
        // obs_register_encoder refuses an info without create, so a
        // registered one always has it; NULL is only the type's other value.
        let Some(create) = fields.create else {
            return core::ptr::null_mut();
        };
        // SAFETY: called after the whole table is installed, with the current
        // settings and the original encoder, as the C does.
        unsafe { create(fields.settings, self.encoder) }
    }
}

/// # Safety
/// Non-NULL arguments must be a live encoder and a NUL-terminated ID. The
/// registry and the encoder must stay valid and externally synchronized
/// through the plugin's create callback.
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

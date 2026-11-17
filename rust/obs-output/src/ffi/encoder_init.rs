//! Thin conversion of the private C operation table to the safe sequencer.
use crate::encoder_init::{Boundary, initialize};
use core::ffi::{c_char, c_void};

/// Matches obs-encoder-init-fields.h; no encoder or pthread layout is mirrored.
#[repr(C)]
pub struct Fields {
    pub sentinel: unsafe extern "C" fn(*mut c_void, u32),
    pub context: unsafe extern "C" fn(*mut c_void, *const c_char, *mut c_void, *mut c_void) -> bool,
    pub mutex: unsafe extern "C" fn(*mut c_void, u32, bool) -> bool,
    pub defaults: unsafe extern "C" fn(*mut c_void),
    pub defaults2: unsafe extern "C" fn(*mut c_void),
}

struct Encoder<'a> {
    fields: &'a Fields,
    encoder: *mut c_void,
    name: *const c_char,
    settings: *mut c_void,
    hotkeys: *mut c_void,
}

impl Boundary for Encoder<'_> {
    fn sentinel(&mut self, index: u32) {
        // SAFETY: the synchronous C caller retains the encoder and its table.
        unsafe { (self.fields.sentinel)(self.encoder, index) }
    }
    fn context(&mut self) -> bool {
        // SAFETY: arguments are the original caller's live C objects.
        unsafe { (self.fields.context)(self.encoder, self.name, self.settings, self.hotkeys) }
    }
    fn mutex(&mut self, index: u32, recursive: bool) -> bool {
        // SAFETY: the core supplies only valid indices after sentinel setup.
        unsafe { (self.fields.mutex)(self.encoder, index, recursive) }
    }
    fn defaults(&mut self) {
        // SAFETY: the core reaches callbacks only after successful initialization.
        unsafe { (self.fields.defaults)(self.encoder) }
    }
    fn defaults2(&mut self) {
        // SAFETY: as above; C reads the current callback and type_data.
        unsafe { (self.fields.defaults2)(self.encoder) }
    }
}

/// # Safety
/// The caller supplies a live encoder, optional settings/hotkeys, a valid name
/// and a non-null table of synchronous callbacks valid for the entire call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_init_steps(
    encoder: *mut c_void,
    name: *const c_char,
    settings: *mut c_void,
    hotkeys: *mut c_void,
    fields: *const Fields,
) -> bool {
    // SAFETY: the private C caller places this table on its live stack.
    let fields = unsafe { &*fields };
    initialize(&mut Encoder {
        fields,
        encoder,
        name,
        settings,
        hotkeys,
    })
}

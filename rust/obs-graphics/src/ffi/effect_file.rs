//! Pointer conversion and private libobs collaborators for effect-file creation.
use super::effect_lookups::gs_effect;
use crate::effect_file::{self as safe, Diagnostic, Model};
use core::ffi::{CStr, c_char, c_void};
use core::ptr;

unsafe extern "C" {
    fn obs_graphics_effect_file_context() -> bool;
    fn obs_graphics_effect_file_first() -> *mut gs_effect;
    fn obs_graphics_effect_file_read(file: *const c_char) -> *mut c_char;
    fn obs_graphics_effect_file_create(
        text: *const c_char,
        file: *const c_char,
        error: *mut *mut c_char,
    ) -> *mut gs_effect;
    fn obs_graphics_effect_file_free(text: *mut c_void);
    fn obs_graphics_effect_file_log(which: i32, file: *const c_char);
}

#[derive(Clone, Copy)]
struct File(*const c_char);
#[derive(Clone, Copy)]
struct Effect(*mut gs_effect);
#[derive(Clone, Copy)]
struct Text(*mut c_char);
struct Boundary {
    error: *mut *mut c_char,
}
impl Model for Boundary {
    type File = File;
    type Effect = Effect;
    type Text = Text;
    fn context_present(&mut self) -> bool {
        // SAFETY: the current thread may query its graphics context.
        unsafe { obs_graphics_effect_file_context() }
    }
    fn first(&mut self) -> Option<Effect> {
        // SAFETY: the core checks context presence before reading its head.
        let effect = unsafe { obs_graphics_effect_file_first() };
        (!effect.is_null()).then_some(Effect(effect))
    }
    fn next(&mut self, effect: Effect) -> Option<Effect> {
        // SAFETY: only non-null live list entries become Effect handles.
        let next = unsafe { (*effect.0).next };
        (!next.is_null()).then_some(Effect(next))
    }
    fn file_path(&self, file: File) -> &[u8] {
        // SAFETY: the original non-null argument is a terminated C string.
        unsafe { CStr::from_ptr(file.0) }.to_bytes()
    }
    fn effect_path(&self, effect: Effect) -> &[u8] {
        // SAFETY: cache entries have a live terminated effect_path, as strcmp requires.
        unsafe { CStr::from_ptr((*effect.0).effect_path) }.to_bytes()
    }
    fn read(&mut self, file: File) -> Option<Text> {
        // SAFETY: file is the caller's original valid file name.
        let text = unsafe { obs_graphics_effect_file_read(file.0) };
        (!text.is_null()).then_some(Text(text))
    }
    fn create(&mut self, text: Text, file: File) -> Option<Effect> {
        // SAFETY: the read buffer remains live; error is forwarded untouched.
        let effect = unsafe { obs_graphics_effect_file_create(text.0, file.0, self.error) };
        (!effect.is_null()).then_some(Effect(effect))
    }
    fn free(&mut self, text: Text) {
        // SAFETY: each successful read is freed exactly once after construction.
        unsafe { obs_graphics_effect_file_free(text.0.cast()) }
    }
    fn log(&mut self, diagnostic: Diagnostic, file: Option<File>) {
        let which = match diagnostic {
            Diagnostic::Context => 0,
            Diagnostic::File => 1,
            Diagnostic::Read => 2,
        };
        // SAFETY: the read diagnostic has a valid file; other formats ignore it.
        unsafe { obs_graphics_effect_file_log(which, file.map_or(ptr::null(), |f| f.0)) }
    }
}
/// Create an effect from a UTF-8 file, or return the first cached path match.
///
/// # Safety
/// `file` is null or a terminated string and `error_string` is null or writable.
/// The caller holds the current graphics context while accessing its cache.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_effect_create_from_file(
    file: *const c_char,
    error_string: *mut *mut c_char,
) -> *mut gs_effect {
    let mut boundary = Boundary {
        error: error_string,
    };
    safe::create_from_file(&mut boundary, (!file.is_null()).then_some(File(file)))
        .map_or(ptr::null_mut(), |effect| effect.0)
}

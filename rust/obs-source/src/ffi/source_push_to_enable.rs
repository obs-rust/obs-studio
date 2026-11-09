//! C ABI borrowing the real source fields and original signal builder.
use crate::push_to_enable::{self, Kind, Source};
use core::ffi::{c_char, c_int, c_void};

#[repr(C)]
pub struct Fields {
    pub audio_mutex: *mut c_void,
    pub mute_enabled: *mut bool,
    pub talk_enabled: *mut bool,
    pub output_flags: *const u32,
    pub name: *const *mut c_char,
    pub signal_changed: unsafe extern "C" fn(*mut c_void, *const c_char, bool),
}
unsafe extern "C" {
    fn obs_source_push_to_enable_fields(source: *mut c_void) -> Fields;
    fn pthread_mutex_lock(mutex: *mut c_void) -> c_int;
    fn pthread_mutex_unlock(mutex: *mut c_void) -> c_int;
    fn blog(level: c_int, format: *const c_char, ...);
}
struct Borrowed {
    source: *mut c_void,
    fields: Fields,
}
impl Borrowed {
    unsafe fn new(source: *mut c_void) -> Option<Self> {
        if source.is_null() {
            None
        } else {
            Some(Self {
                source,
                // SAFETY: NULL was checked before forming field addresses; caller supplies a live source.
                fields: unsafe { obs_source_push_to_enable_fields(source) },
            })
        }
    }
    fn bit(&self, kind: Kind) -> *mut bool {
        match kind {
            Kind::Mute => self.fields.mute_enabled,
            Kind::Talk => self.fields.talk_enabled,
        }
    }
}
impl Source for Borrowed {
    fn lock(&mut self) {
        // SAFETY: the source owns an initialized mutex for the duration of this call.
        unsafe { pthread_mutex_lock(self.fields.audio_mutex) };
    }
    fn unlock(&mut self) {
        // SAFETY: the core releases the mutex it acquired on this thread.
        unsafe { pthread_mutex_unlock(self.fields.audio_mutex) };
    }
    fn enabled(&self, kind: Kind) -> bool {
        // SAFETY: the core reads a live enable bit while holding audio_mutex.
        unsafe { *self.bit(kind) }
    }
    fn output_flags(&self) -> u32 {
        // SAFETY: the source's flags remain live; read occurs at the original point under audio_mutex.
        unsafe { *self.fields.output_flags }
    }
    fn log_change(&mut self, kind: Kind, enabled: bool) {
        let format = match kind {
            Kind::Mute => c"source '%s' %s push-to-mute",
            Kind::Talk => c"source '%s' %s push-to-talk",
        };
        let action = if enabled { c"enabled" } else { c"disabled" };
        // SAFETY: source name is the original C string, borrowed under audio_mutex; other strings are static.
        unsafe { blog(300, format.as_ptr(), *self.fields.name, action.as_ptr()) };
    }
    fn store(&mut self, kind: Kind, enabled: bool) {
        // SAFETY: the core writes the live field only while holding audio_mutex.
        unsafe { *self.bit(kind) = enabled };
    }
    fn signal(&mut self, kind: Kind, enabled: bool) {
        let signal = match kind {
            Kind::Mute => c"push_to_mute_changed",
            Kind::Talk => c"push_to_talk_changed",
        };
        // SAFETY: the real C seam synchronously borrows this source, with the original mutex still held.
        unsafe { (self.fields.signal_changed)(self.source, signal.as_ptr(), enabled) };
    }
}
fn null_log(name: &'static str) {
    let function = match name {
        "obs_source_enable_push_to_mute" => c"obs_source_enable_push_to_mute",
        _ => c"obs_source_enable_push_to_talk",
    };
    // SAFETY: format and argument are static NUL-terminated strings.
    unsafe {
        blog(
            400,
            c"%s: Null 'source' parameter".as_ptr(),
            function.as_ptr(),
        )
    };
}
/// # Safety
/// Non-NULL source must remain live and have an initialized audio mutex.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_enable_push_to_mute(source: *mut c_void, enabled: bool) {
    // SAFETY: the public contract provides a live source or NULL.
    let mut source = unsafe { Borrowed::new(source) };
    push_to_enable::enable(source.as_mut(), Kind::Mute, enabled, null_log);
}
/// # Safety
/// Non-NULL source must remain live and have an initialized audio mutex.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_enable_push_to_talk(source: *mut c_void, enabled: bool) {
    // SAFETY: the public contract provides a live source or NULL.
    let mut source = unsafe { Borrowed::new(source) };
    push_to_enable::enable(source.as_mut(), Kind::Talk, enabled, null_log);
}

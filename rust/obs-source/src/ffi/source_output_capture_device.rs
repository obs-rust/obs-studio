//! C ABI of `obs_source_audio_output_capture_device_changed()` (`obs.h`,
//! `EXPORT`, in `libobs/cmake/rust-exports.d`) and of its `activate` signal
//! handler `obs_source_audio_output_capture_device_activated()` (no
//! `EXPORT`, hidden in `libobs/cmake/rust-hidden.d`). Every read and call
//! stays behind the adapters of `obs-source-output-capture-device-fields.c.h`.
use crate::output_capture_device::{Device, device_activated, device_changed};
use core::ffi::{c_char, c_int, c_void};
use core::ptr::{NonNull, null_mut};

/// `signal_callback_t`.
pub type SignalCallback = unsafe extern "C" fn(*mut c_void, *mut c_void);

unsafe extern "C" {
    fn obs_source_output_capture_device_monitoring_set() -> bool;
    fn obs_source_output_capture_device_flags(source: *const c_void) -> u32;
    fn obs_source_output_capture_device_monitoring_id() -> *const c_char;
    fn obs_source_output_capture_device_host_default(
        device_id: *const c_char,
        def_id: *mut *mut c_char,
    ) -> bool;
    fn obs_source_output_capture_device_match(id1: *const c_char, id2: *const c_char) -> bool;
    fn obs_source_output_capture_device_free(id: *mut c_char);
    fn obs_source_output_capture_device_signal(source: *mut c_void);
    fn obs_source_output_capture_device_connect(source: *mut c_void, handler: SignalCallback);
    fn obs_source_output_capture_device_disconnect(source: *mut c_void, handler: SignalCallback);
    fn obs_source_output_capture_device_name(source: *const c_void) -> *const c_char;
    fn obs_source_output_capture_device_duplicating() -> *mut c_void;
    fn obs_source_output_capture_device_calldata_source(cd: *mut c_void) -> *mut c_void;
    fn obs_source_output_capture_device_settings(source: *mut c_void) -> *mut c_void;
    fn obs_source_output_capture_device_settings_id(settings: *mut c_void) -> *const c_char;
    fn obs_source_output_capture_device_release_settings(settings: *mut c_void);
    fn blog(level: c_int, format: *const c_char, ...);
}

/// `LOG_INFO`.
const LOG_INFO: c_int = 300;

/// The C's started line, with the source's name as its one `%s`.
pub const STARTED_FORMAT: &core::ffi::CStr = c"Device for 'Audio Output Capture' source %s is also used for audio monitoring.\nDeduplication logic is being applied to all monitored sources.";
/// The C's stopped line.
pub const STOPPED_LINE: &core::ffi::CStr = c"Deduplication logic stopped.";

/// A device id as the C passes it: NULL is `None`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Id(NonNull<c_char>);

impl Id {
    /// A non-NULL C id.
    pub fn from_raw(ptr: NonNull<c_char>) -> Self {
        Self(ptr)
    }

    /// The C pointer.
    pub fn as_ptr(self) -> *const c_char {
        self.0.as_ptr().cast_const()
    }
}

fn id(ptr: *const c_char) -> Option<Id> {
    NonNull::new(ptr.cast_mut()).map(Id)
}

fn raw(id: Option<Id>) -> *const c_char {
    id.map_or(core::ptr::null(), Id::as_ptr)
}

/// A C pointer (a source, a calldata or a settings object) as the C passes
/// it, possibly NULL; only the adapters dereference it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ptr(pub *mut c_void);

/// libobs, with `handler` the `activate` handler connected and disconnected
/// (the Rust one in production; the parity test passes the original's).
#[derive(Clone, Copy, Debug)]
pub struct Libobs {
    /// The handler given to the `activate` connection.
    pub handler: SignalCallback,
}

impl Default for Libobs {
    fn default() -> Self {
        Self {
            handler: obs_source_audio_output_capture_device_activated,
        }
    }
}

impl Device for Libobs {
    type Source = Ptr;
    type Id = Id;
    type Calldata = Ptr;
    type Settings = Ptr;

    fn monitoring_device_set(&mut self) -> bool {
        // SAFETY: reads one field of the started core.
        unsafe { obs_source_output_capture_device_monitoring_set() }
    }
    fn output_flags(&mut self, source: Ptr) -> u32 {
        // SAFETY: the caller's source, read as the C read it (a NULL one
        // faults here, as it did in the C).
        unsafe { obs_source_output_capture_device_flags(source.0) }
    }
    fn monitoring_id(&mut self) -> Option<Id> {
        // SAFETY: reads one field of the started core.
        id(unsafe { obs_source_output_capture_device_monitoring_id() })
    }
    fn host_default(&mut self, device_id: Option<Id>) -> Option<Option<Id>> {
        let mut def_id = null_mut();
        // SAFETY: the caller's id (or NULL) and a local out pointer.
        let answered =
            unsafe { obs_source_output_capture_device_host_default(raw(device_id), &mut def_id) };
        answered.then(|| id(def_id))
    }
    fn devices_match(&mut self, id1: Option<Id>, id2: Option<Id>) -> bool {
        // SAFETY: ids from the caller, the host or the core, or NULL.
        unsafe { obs_source_output_capture_device_match(raw(id1), raw(id2)) }
    }
    fn free_default(&mut self, id: Id) {
        // SAFETY: the bmem string the host default gave, freed once.
        unsafe { obs_source_output_capture_device_free(id.0.as_ptr()) }
    }
    fn signal_deduplication(&mut self, source: Option<Ptr>) {
        // SAFETY: the caller's source or NULL, passed as the signal's data.
        unsafe {
            obs_source_output_capture_device_signal(source.map_or(null_mut(), |source| source.0))
        }
    }
    fn connect_activate(&mut self, source: Ptr) {
        // SAFETY: the caller's live source.
        unsafe { obs_source_output_capture_device_connect(source.0, self.handler) }
    }
    fn disconnect_activate(&mut self, source: Ptr) {
        // SAFETY: the caller's live source.
        unsafe { obs_source_output_capture_device_disconnect(source.0, self.handler) }
    }
    fn log_started(&mut self, source: Ptr) {
        // SAFETY: the caller's live source; its name matches the one %s.
        unsafe {
            let name = obs_source_output_capture_device_name(source.0);
            blog(LOG_INFO, STARTED_FORMAT.as_ptr(), name);
        }
    }
    fn log_stopped(&mut self) {
        // SAFETY: a static line with no conversion.
        unsafe { blog(LOG_INFO, STOPPED_LINE.as_ptr()) }
    }
    fn duplicating_source(&mut self) -> Option<Ptr> {
        // SAFETY: reads one field of the started core.
        let source = unsafe { obs_source_output_capture_device_duplicating() };
        (!source.is_null()).then_some(Ptr(source))
    }
    fn calldata_source(&mut self, cd: Ptr) -> Option<Ptr> {
        // SAFETY: the signal's calldata.
        let source = unsafe { obs_source_output_capture_device_calldata_source(cd.0) };
        (!source.is_null()).then_some(Ptr(source))
    }
    fn settings(&mut self, source: Ptr) -> Ptr {
        // SAFETY: the live source the signal named.
        Ptr(unsafe { obs_source_output_capture_device_settings(source.0) })
    }
    fn settings_device_id(&mut self, settings: Ptr) -> Option<Id> {
        // SAFETY: the settings just taken.
        id(unsafe { obs_source_output_capture_device_settings_id(settings.0) })
    }
    fn release_settings(&mut self, settings: Ptr) {
        // SAFETY: the reference taken by `settings`, released once.
        unsafe { obs_source_output_capture_device_release_settings(settings.0) }
    }
}

/// `obs_source_audio_output_capture_device_changed()`.
///
/// # Safety
/// `source` is a live source (the original dereferences it whenever a
/// monitoring device is set); `device_id` is NULL or a C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_audio_output_capture_device_changed(
    source: *mut c_void,
    device_id: *const c_char,
) {
    device_changed(&mut Libobs::default(), Ptr(source), id(device_id));
}

/// `obs_source_audio_output_capture_device_activated()` (hidden): the
/// source's `activate` handler.
///
/// # Safety
/// `cd` is the signal's calldata; its `"source"`, when set, is live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_audio_output_capture_device_activated(
    _data: *mut c_void,
    cd: *mut c_void,
) {
    device_activated(&mut Libobs::default(), Ptr(cd));
}

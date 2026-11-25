//! The source's fields, its `media_actions` queue and its signal handler
//! stay behind the adapters of `obs-source-media-controls-fields.c.h`: the
//! queue adapters build the `struct media_action` with the C's designated
//! initializers and append it under `media_actions_mutex`, and the signal
//! adapter goes through `obs_source_dosignal(source, NULL, signal)`.
use crate::media_admission::Call;
use crate::media_controls::{self, Action, Source};
use core::ffi::{CStr, c_char, c_int, c_void};

unsafe extern "C" {
    fn obs_source_media_controls_data(source: *const c_void) -> *mut c_void;
    fn obs_source_media_controls_output_flags(source: *const c_void) -> u32;
    fn obs_source_media_controls_has_callback(source: *const c_void, call: c_int) -> bool;
    fn obs_source_media_controls_queue(source: *mut c_void, action_type: c_int);
    fn obs_source_media_controls_queue_play_pause(source: *mut c_void, pause: bool);
    fn obs_source_media_controls_queue_set_time(source: *mut c_void, ms: i64);
    fn obs_source_media_controls_signal(source: *mut c_void, signal: *const c_char);
    fn blog(level: c_int, format: *const c_char, ...);
}

/// A live, non-NULL source, read, queued to and signalled through the
/// adapters (public for the parity test's mutants).
pub struct Live(*mut c_void);

impl Live {
    /// # Safety
    /// `source` is a live, non-NULL source that stays live while this value
    /// is used, under the original C calls' contract.
    #[must_use]
    pub unsafe fn new(source: *mut c_void) -> Self {
        Self(source)
    }
}

impl Source for Live {
    fn has_data(&self) -> bool {
        // SAFETY: a live source, read under the C calls' contract.
        !unsafe { obs_source_media_controls_data(self.0) }.is_null()
    }
    fn output_flags(&self) -> u32 {
        // SAFETY: a live source.
        unsafe { obs_source_media_controls_output_flags(self.0) }
    }
    fn has_callback(&self, call: Call) -> bool {
        // SAFETY: a live source; the adapter reads only the callback the
        // call names.
        unsafe { obs_source_media_controls_has_callback(self.0, call.value()) }
    }
    fn queue(&self, action: Action) {
        // SAFETY: a live source whose queue and mutex the adapters own, as
        // the C calls did.
        unsafe {
            match action {
                Action::PlayPause(pause) => {
                    obs_source_media_controls_queue_play_pause(self.0, pause)
                }
                Action::SetTime(ms) => obs_source_media_controls_queue_set_time(self.0, ms),
                Action::Restart | Action::Stop | Action::Next | Action::Previous => {
                    obs_source_media_controls_queue(self.0, action.action_type());
                }
            }
        }
    }
    fn signal(&self, signal: &'static CStr) {
        // SAFETY: a live source and a static signal name; the dispatch is
        // synchronous and no reference to the source spans it.
        unsafe { obs_source_media_controls_signal(self.0, signal.as_ptr()) }
    }
}

fn null_log(function: &'static CStr) {
    // SAFETY: the literal format and the static name match its one %s.
    unsafe {
        blog(
            400,
            c"%s: Null 'source' parameter".as_ptr(),
            function.as_ptr(),
        );
    }
}

/// No adapter runs until NULL has been rejected.
fn live(source: *mut c_void) -> Option<Live> {
    (!source.is_null()).then_some(Live(source))
}

/// # Safety
/// A non-NULL source must stay live through the call, under the original C
/// function's ownership and synchronization rules.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_media_play_pause(source: *mut c_void, pause: bool) {
    media_controls::queue(live(source).as_ref(), Action::PlayPause(pause), null_log);
}

/// # Safety
/// As [`obs_source_media_play_pause`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_media_restart(source: *mut c_void) {
    media_controls::queue(live(source).as_ref(), Action::Restart, null_log);
}

/// # Safety
/// As [`obs_source_media_play_pause`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_media_stop(source: *mut c_void) {
    media_controls::queue(live(source).as_ref(), Action::Stop, null_log);
}

/// # Safety
/// As [`obs_source_media_play_pause`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_media_next(source: *mut c_void) {
    media_controls::queue(live(source).as_ref(), Action::Next, null_log);
}

/// # Safety
/// As [`obs_source_media_play_pause`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_media_previous(source: *mut c_void) {
    media_controls::queue(live(source).as_ref(), Action::Previous, null_log);
}

/// # Safety
/// As [`obs_source_media_play_pause`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_media_set_time(source: *mut c_void, ms: i64) {
    media_controls::queue(live(source).as_ref(), Action::SetTime(ms), null_log);
}

/// # Safety
/// A non-NULL source must stay live through the call, including its
/// synchronous `media_started` handlers, under the original C function's
/// ownership and synchronization rules.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_media_started(source: *mut c_void) {
    media_controls::started(live(source).as_ref(), null_log);
}

/// # Safety
/// As [`obs_source_media_started`], for `media_ended`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_media_ended(source: *mut c_void) {
    media_controls::ended(live(source).as_ref(), null_log);
}

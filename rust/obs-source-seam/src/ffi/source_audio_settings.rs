//! C ABI for `libobs/obs-source-audio-settings.c.h`, the source audio
//! settings: the volume, sync offset, balance, mute and monitoring setters and
//! getters, all `EXPORT` through `obs.h` (`rust-exports.d`).
//!
//! The functions are the safe core's (`obs_source::audio_settings`), run on
//! [`Seam`], which implements its `Source` over
//! `libobs/obs-source-audio-settings-seam.h`, still C in
//! `libobs.core-source`: one seam call per method. `struct obs_source` is
//! never mirrored, and the fields stay in C, so ThreadSanitizer keeps seeing
//! their reads and writes. The locking is the C's: no signal goes out with a
//! lock held.
//!
//! `enum obs_monitoring_type` crosses as `int`, as libobs passes it (checked
//! in `obs-source-audio-settings-seam.c.h`).

use std::ffi::{CStr, c_char, c_int, c_void};
use std::ptr;

use obs_source::audio_settings::{self as core, Source};

unsafe extern "C" {
    fn os_gettime_ns() -> u64;

    fn obs_source_audio_settings_seam_valid(source: *const c_void, func: *const c_char) -> bool;
    fn obs_source_audio_settings_seam_warn(message: *const c_char);
    fn obs_source_audio_settings_seam_is_private(source: *const c_void) -> bool;

    fn obs_source_audio_settings_seam_lock_audio_buf(source: *const c_void);
    fn obs_source_audio_settings_seam_unlock_audio_buf(source: *const c_void);
    fn obs_source_audio_settings_seam_lock_actions(source: *const c_void);
    fn obs_source_audio_settings_seam_unlock_actions(source: *const c_void);

    fn obs_source_audio_settings_seam_get_user_volume(source: *const c_void) -> f32;
    fn obs_source_audio_settings_seam_set_user_volume(source: *mut c_void, value: f32);
    fn obs_source_audio_settings_seam_get_sync_offset(source: *const c_void) -> i64;
    fn obs_source_audio_settings_seam_set_sync_offset(source: *mut c_void, value: i64);
    fn obs_source_audio_settings_seam_get_balance(source: *const c_void) -> f32;
    fn obs_source_audio_settings_seam_set_balance(source: *mut c_void, value: f32);
    fn obs_source_audio_settings_seam_set_monitoring_type(source: *mut c_void, value: c_int);

    fn obs_source_audio_settings_seam_get_user_muted(source: *const c_void) -> bool;
    fn obs_source_audio_settings_seam_set_user_muted(source: *mut c_void, value: bool);
    fn obs_source_audio_settings_seam_get_monitor_only_muted(source: *const c_void) -> bool;
    fn obs_source_audio_settings_seam_set_monitor_only_muted(source: *mut c_void, value: bool);
    fn obs_source_audio_settings_seam_get_monitoring_enabled(source: *const c_void) -> bool;
    fn obs_source_audio_settings_seam_set_monitoring_enabled(source: *mut c_void, value: bool);

    fn obs_source_audio_settings_seam_push_volume_action(
        source: *mut c_void,
        timestamp: u64,
        volume: f32,
    );
    fn obs_source_audio_settings_seam_push_mute_action(
        source: *mut c_void,
        timestamp: u64,
        muted: bool,
    );

    fn obs_source_audio_settings_seam_signal_volume(
        source: *mut c_void,
        volume: f64,
        also_core: bool,
    ) -> f64;
    fn obs_source_audio_settings_seam_signal_sync_offset(source: *mut c_void, offset: i64) -> i64;
    fn obs_source_audio_settings_seam_signal_balance(source: *mut c_void, balance: f64) -> f64;
    fn obs_source_audio_settings_seam_signal_muted(source: *mut c_void, muted: bool);
    fn obs_source_audio_settings_seam_signal_monitor(source: *mut c_void, enabled: bool);
    fn obs_source_audio_settings_seam_signal_audio_monitoring(
        source: *mut c_void,
        monitoring_type: c_int,
    );

    fn obs_source_audio_settings_seam_monitor_create(source: *mut c_void);
    fn obs_source_audio_settings_seam_monitor_destroy(source: *mut c_void);
}

/// A live, non-NULL `obs_source_t` a shim was handed. NULL never becomes a
/// `Seam`: [`source`] gives `None` for it.
pub struct Seam(*mut c_void);

// SAFETY (every method): `self.0` is the live source the C caller passed, and
// each call is the one seam function that names it.
impl Source for Seam {
    fn valid(source: Option<&Self>, func: &'static CStr) -> bool {
        let ptr = source.map_or(ptr::null(), |s| s.0.cast_const());
        // SAFETY: NULL or the caller's live source, and a static name.
        unsafe { obs_source_audio_settings_seam_valid(ptr, func.as_ptr()) }
    }
    fn now() -> u64 {
        // SAFETY: no arguments.
        unsafe { os_gettime_ns() }
    }
    fn warn(message: &'static CStr) {
        // SAFETY: a static NUL-terminated message.
        unsafe { obs_source_audio_settings_seam_warn(message.as_ptr()) }
    }
    fn is_private(&self) -> bool {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_settings_seam_is_private(self.0) }
    }
    fn lock_audio_buf(&self) {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_settings_seam_lock_audio_buf(self.0) }
    }
    fn unlock_audio_buf(&self) {
        // SAFETY: see the impl; the core locked it.
        unsafe { obs_source_audio_settings_seam_unlock_audio_buf(self.0) }
    }
    fn lock_actions(&self) {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_settings_seam_lock_actions(self.0) }
    }
    fn unlock_actions(&self) {
        // SAFETY: see the impl; the core locked it.
        unsafe { obs_source_audio_settings_seam_unlock_actions(self.0) }
    }
    fn user_volume(&self) -> f32 {
        // SAFETY: see the impl; the core holds audio_buf_mutex.
        unsafe { obs_source_audio_settings_seam_get_user_volume(self.0) }
    }
    fn set_user_volume(&self, value: f32) {
        // SAFETY: see the impl; the core holds audio_buf_mutex.
        unsafe { obs_source_audio_settings_seam_set_user_volume(self.0, value) }
    }
    fn sync_offset(&self) -> i64 {
        // SAFETY: see the impl; the core holds audio_buf_mutex.
        unsafe { obs_source_audio_settings_seam_get_sync_offset(self.0) }
    }
    fn set_sync_offset(&self, value: i64) {
        // SAFETY: see the impl; the core holds audio_buf_mutex.
        unsafe { obs_source_audio_settings_seam_set_sync_offset(self.0, value) }
    }
    fn balance(&self) -> f32 {
        // SAFETY: see the impl; the core holds audio_buf_mutex.
        unsafe { obs_source_audio_settings_seam_get_balance(self.0) }
    }
    fn set_balance(&self, value: f32) {
        // SAFETY: see the impl; the core holds audio_buf_mutex.
        unsafe { obs_source_audio_settings_seam_set_balance(self.0, value) }
    }
    fn set_monitoring_type(&self, value: i32) {
        // SAFETY: see the impl; the core holds audio_buf_mutex.
        unsafe { obs_source_audio_settings_seam_set_monitoring_type(self.0, value) }
    }
    fn user_muted(&self) -> bool {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_settings_seam_get_user_muted(self.0) }
    }
    fn set_user_muted(&self, value: bool) {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_settings_seam_set_user_muted(self.0, value) }
    }
    fn monitor_only_muted(&self) -> bool {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_settings_seam_get_monitor_only_muted(self.0) }
    }
    fn set_monitor_only_muted(&self, value: bool) {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_settings_seam_set_monitor_only_muted(self.0, value) }
    }
    fn monitoring_enabled(&self) -> bool {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_settings_seam_get_monitoring_enabled(self.0) }
    }
    fn set_monitoring_enabled(&self, value: bool) {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_settings_seam_set_monitoring_enabled(self.0, value) }
    }
    fn push_volume_action(&self, timestamp: u64, volume: f32) {
        // SAFETY: see the impl; the core holds audio_actions_mutex.
        unsafe { obs_source_audio_settings_seam_push_volume_action(self.0, timestamp, volume) }
    }
    fn push_mute_action(&self, timestamp: u64, muted: bool) {
        // SAFETY: see the impl; the core holds audio_actions_mutex.
        unsafe { obs_source_audio_settings_seam_push_mute_action(self.0, timestamp, muted) }
    }
    fn signal_volume(&self, volume: f64, also_core: bool) -> f64 {
        // SAFETY: see the impl; the core holds no lock.
        unsafe { obs_source_audio_settings_seam_signal_volume(self.0, volume, also_core) }
    }
    fn signal_sync_offset(&self, offset: i64) -> i64 {
        // SAFETY: see the impl; the core holds no lock.
        unsafe { obs_source_audio_settings_seam_signal_sync_offset(self.0, offset) }
    }
    fn signal_balance(&self, balance: f64) -> f64 {
        // SAFETY: see the impl; the core holds no lock.
        unsafe { obs_source_audio_settings_seam_signal_balance(self.0, balance) }
    }
    fn signal_muted(&self, muted: bool) {
        // SAFETY: see the impl; the core holds no lock.
        unsafe { obs_source_audio_settings_seam_signal_muted(self.0, muted) }
    }
    fn signal_monitor(&self, enabled: bool) {
        // SAFETY: see the impl; the core holds no lock.
        unsafe { obs_source_audio_settings_seam_signal_monitor(self.0, enabled) }
    }
    fn signal_audio_monitoring(&self, monitoring_type: i32) {
        // SAFETY: see the impl; the core holds no lock.
        unsafe { obs_source_audio_settings_seam_signal_audio_monitoring(self.0, monitoring_type) }
    }
    fn monitor_create(&self) {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_settings_seam_monitor_create(self.0) }
    }
    fn monitor_destroy(&self) {
        // SAFETY: see the impl; the core only does this when monitoring was on.
        unsafe { obs_source_audio_settings_seam_monitor_destroy(self.0) }
    }
}

/// `Some` for a non-NULL source pointer.
fn source(source: *const c_void) -> Option<Seam> {
    (!source.is_null()).then(|| Seam(source.cast_mut()))
}

/// `void obs_source_set_volume(obs_source_t *source, float volume)`.
///
/// # Safety
/// `obj` is NULL or a live source.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_set_volume(obj: *mut c_void, volume: f32) {
    core::set_volume(source(obj).as_ref(), volume);
}

/// `float obs_source_get_volume(const obs_source_t *source)`.
///
/// # Safety
/// `obj` is NULL or a live source.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_volume(obj: *const c_void) -> f32 {
    core::get_volume(source(obj).as_ref())
}

/// `void obs_source_set_sync_offset(obs_source_t *source, int64_t offset)`.
///
/// # Safety
/// `obj` is NULL or a live source.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_set_sync_offset(obj: *mut c_void, offset: i64) {
    core::set_sync_offset(source(obj).as_ref(), offset);
}

/// `int64_t obs_source_get_sync_offset(const obs_source_t *source)`.
///
/// # Safety
/// `obj` is NULL or a live source.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_sync_offset(obj: *const c_void) -> i64 {
    core::get_sync_offset(source(obj).as_ref())
}

/// `bool obs_source_muted(const obs_source_t *source)`.
///
/// # Safety
/// `obj` is NULL or a live source.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_muted(obj: *const c_void) -> bool {
    core::muted(source(obj).as_ref())
}

/// `void obs_source_set_muted(obs_source_t *source, bool muted)`.
///
/// # Safety
/// `obj` is NULL or a live source.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_set_muted(obj: *mut c_void, muted: bool) {
    core::set_muted(source(obj).as_ref(), muted);
}

/// `void obs_source_set_monitoring_enabled(obs_source_t *source, bool
/// enabled)`.
///
/// # Safety
/// `obj` is NULL or a live source.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_set_monitoring_enabled(obj: *mut c_void, enabled: bool) {
    core::set_monitoring_enabled(source(obj).as_ref(), enabled);
}

/// `bool obs_source_get_monitoring_enabled(const obs_source_t *source)`.
///
/// # Safety
/// `obj` is NULL or a live source.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_monitoring_enabled(obj: *const c_void) -> bool {
    core::get_monitoring_enabled(source(obj).as_ref())
}

/// `void obs_source_set_monitoring_type(obs_source_t *source, enum
/// obs_monitoring_type type)`.
///
/// # Safety
/// `obj` is NULL or a live source.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_set_monitoring_type(obj: *mut c_void, monitoring_type: c_int) {
    core::set_monitoring_type(source(obj).as_ref(), monitoring_type);
}

/// `enum obs_monitoring_type obs_source_get_monitoring_type(const obs_source_t
/// *source)`.
///
/// # Safety
/// `obj` is NULL or a live source.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_monitoring_type(obj: *const c_void) -> c_int {
    core::get_monitoring_type(source(obj).as_ref())
}

/// `void obs_source_set_balance_value(obs_source_t *source, float balance)`.
///
/// # Safety
/// `obj` is NULL or a live source.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_set_balance_value(obj: *mut c_void, balance: f32) {
    core::set_balance_value(source(obj).as_ref(), balance);
}

/// `float obs_source_get_balance_value(const obs_source_t *source)`.
///
/// # Safety
/// `obj` is NULL or a live source.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_balance_value(obj: *const c_void) -> f32 {
    core::get_balance_value(source(obj).as_ref())
}

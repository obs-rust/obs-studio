//! C ABI for `libobs/obs-source-muted-volume.c.h`: `source_muted()` (the
//! audio output path's mute check) and `get_source_volume()` (the volume
//! the audio mix thread applies), declared in `obs-internal.h` without
//! `EXPORT`, so the shims stay hidden (`rust-hidden.d`).
//!
//! Runs the safe core ([`crate::muted_volume`]) over the C-owned source:
//! every field is one read or write through the adapters of
//! `obs-source-muted-volume-fields.c.h`, so the pressed flags stay the plain
//! reads they were and `user_muted` the C's `os_atomic_load_bool()`. Like
//! the C, nothing checks the source: the callers pass a live one.
//!
//! No intentional differences from C.

use core::ffi::{c_int, c_void};

use crate::muted_volume::{self, Flags, Host, Key};

unsafe extern "C" {
    fn obs_source_muted_volume_key_enabled(source: *const c_void, key: c_int) -> bool;
    fn obs_source_muted_volume_user_pressed(source: *const c_void, key: c_int) -> bool;
    fn obs_source_muted_volume_pressed(source: *const c_void, key: c_int) -> bool;
    fn obs_source_muted_volume_delay(source: *const c_void, key: c_int) -> u64;
    fn obs_source_muted_volume_stop_time(source: *const c_void, key: c_int) -> u64;
    fn obs_source_muted_volume_set_stop_time(source: *mut c_void, key: c_int, stop_time: u64);
    fn obs_source_muted_volume_enabled(source: *const c_void) -> bool;
    fn obs_source_muted_volume_user_muted(source: *const c_void) -> bool;
    fn obs_source_muted_volume_muted(source: *const c_void) -> bool;
    fn obs_source_muted_volume_volume(source: *const c_void) -> f32;
}

/// The adapters' key number.
const fn key_number(key: Key) -> c_int {
    match key {
        Key::Mute => 0,
        Key::Talk => 1,
    }
}

/// The live libobs source the caller hands over.
struct Libobs;

// SAFETY (every method): `source` is the caller's live, non-null source; the
// key number is one the adapters know.
impl Host for Libobs {
    type Source = *mut c_void;
    fn key_enabled(&self, source: *mut c_void, key: Key) -> bool {
        // SAFETY: a live source.
        unsafe { obs_source_muted_volume_key_enabled(source, key_number(key)) }
    }
    fn pressed(&self, source: *mut c_void, flags: Flags, key: Key) -> bool {
        // SAFETY: a live source.
        unsafe {
            match flags {
                Flags::User => obs_source_muted_volume_user_pressed(source, key_number(key)),
                Flags::Mix => obs_source_muted_volume_pressed(source, key_number(key)),
            }
        }
    }
    fn delay(&self, source: *mut c_void, key: Key) -> u64 {
        // SAFETY: a live source.
        unsafe { obs_source_muted_volume_delay(source, key_number(key)) }
    }
    fn stop_time(&self, source: *mut c_void, key: Key) -> u64 {
        // SAFETY: a live source.
        unsafe { obs_source_muted_volume_stop_time(source, key_number(key)) }
    }
    fn set_stop_time(&self, source: *mut c_void, key: Key, stop_time: u64) {
        // SAFETY: a live source; the C writes the field without a lock too.
        unsafe { obs_source_muted_volume_set_stop_time(source, key_number(key), stop_time) }
    }
    fn enabled(&self, source: *mut c_void) -> bool {
        // SAFETY: a live source.
        unsafe { obs_source_muted_volume_enabled(source) }
    }
    fn muted(&self, source: *mut c_void, flags: Flags) -> bool {
        // SAFETY: a live source.
        unsafe {
            match flags {
                Flags::User => obs_source_muted_volume_user_muted(source),
                Flags::Mix => obs_source_muted_volume_muted(source),
            }
        }
    }
    fn volume(&self, source: *mut c_void) -> f32 {
        // SAFETY: a live source.
        unsafe { obs_source_muted_volume_volume(source) }
    }
}

/// `source_muted(source, os_time)`.
///
/// # Safety
/// `source` is a live source.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn source_muted(source: *mut c_void, os_time: u64) -> bool {
    muted_volume::source_muted(&Libobs, source, os_time)
}

/// `get_source_volume(source, os_time)`.
///
/// # Safety
/// `source` is a live source.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn get_source_volume(source: *mut c_void, os_time: u64) -> f32 {
    muted_volume::get_source_volume(&Libobs, source, os_time)
}

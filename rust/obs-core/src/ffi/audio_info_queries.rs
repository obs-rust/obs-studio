//! `obs_get_audio_info()` and `obs_get_audio_info2()` over the real core,
//! reached through the private adapters of
//! `libobs/obs-audio-info-queries-fields.c.h` (#5363).
//!
//! The core is checked for NULL before any field is read; each field is read
//! when the safe core asks for it, and the output's info comes from the real
//! `audio_output_get_info()`. The caller's struct is written field by field,
//! only on success, so its padding and any untouched bytes stay as they were.
use core::ffi::{c_int, c_void};
use core::ptr::{addr_of, addr_of_mut};

use obs_media_io::ffi::audio_io::audio_output_info;

use crate::audio_info_queries::{AudioCore, AudioInfo, get_audio_info, get_audio_info2};
use crate::core_setup_rules::buffer_ms;

/// Mirrors `struct obs_audio_info` (`obs.h`).
#[repr(C)]
#[allow(non_camel_case_types)]
pub struct obs_audio_info {
    pub samples_per_sec: u32,
    pub speakers: c_int,
}

/// Mirrors `struct obs_audio_info2` (`obs.h`).
#[repr(C)]
#[allow(non_camel_case_types)]
pub struct obs_audio_info2 {
    pub samples_per_sec: u32,
    pub speakers: c_int,
    pub max_buffering_ms: u32,
    pub fixed_buffering: bool,
}

unsafe extern "C" {
    fn obs_audio_info_queries_core() -> *const c_void;
    fn obs_audio_info_queries_output(core: *const c_void) -> *mut c_void;
    fn obs_audio_info_queries_output_info(audio: *const c_void) -> *const audio_output_info;
    fn obs_audio_info_queries_fixed_buffer(core: *const c_void) -> bool;
    fn obs_audio_info_queries_max_buffering_ticks(core: *const c_void) -> c_int;
}

/// The live, non-NULL current core.
struct Core(*const c_void);

impl AudioCore for Core {
    fn has_output(&mut self) -> bool {
        // SAFETY: `self.0` is the live current core (see `current`).
        !unsafe { obs_audio_info_queries_output(self.0) }.is_null()
    }
    fn output_info(&mut self) -> AudioInfo {
        // SAFETY: as in `has_output`; the handle was just found non-NULL.
        let audio = unsafe { obs_audio_info_queries_output(self.0) };
        // SAFETY: a live audio output's info, valid while the output lives,
        // as the original's read of it requires.
        let info = unsafe { obs_audio_info_queries_output_info(audio) };
        // SAFETY: `info` points to the output's live `audio_output_info`.
        unsafe {
            AudioInfo {
                samples_per_sec: addr_of!((*info).samples_per_sec).read(),
                speakers: addr_of!((*info).speakers).read(),
            }
        }
    }
    fn fixed_buffer(&mut self) -> bool {
        // SAFETY: as in `has_output`.
        unsafe { obs_audio_info_queries_fixed_buffer(self.0) }
    }
    fn max_buffering_ticks(&mut self) -> c_int {
        // SAFETY: as in `has_output`.
        unsafe { obs_audio_info_queries_max_buffering_ticks(self.0) }
    }
    fn buffer_ms(&mut self, ticks: c_int, samples_per_sec: u32) -> c_int {
        buffer_ms(ticks, samples_per_sec)
    }
}

/// The current core, `None` when it is NULL.
fn current() -> Option<Core> {
    // SAFETY: the adapter only returns the current global core pointer.
    let core = unsafe { obs_audio_info_queries_core() };
    (!core.is_null()).then_some(Core(core))
}

/// # Safety
/// `oai` is NULL or writable for a `struct obs_audio_info`; the current
/// core, when not NULL, is live with a live audio output or none.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_audio_info(oai: *mut obs_audio_info) -> bool {
    let Some(info) = get_audio_info(current().as_mut(), !oai.is_null()) else {
        return false;
    };
    // SAFETY: `oai` is non-NULL (checked by the safe core) and writable.
    unsafe {
        addr_of_mut!((*oai).samples_per_sec).write(info.samples_per_sec);
        addr_of_mut!((*oai).speakers).write(info.speakers);
    }
    true
}

/// # Safety
/// `oai2` is NULL or writable for a `struct obs_audio_info2`; the current
/// core, when not NULL, is live with a live audio output or none.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_audio_info2(oai2: *mut obs_audio_info2) -> bool {
    let Some(info) = get_audio_info2(current().as_mut(), !oai2.is_null()) else {
        return false;
    };
    // SAFETY: `oai2` is non-NULL (checked by the safe core) and writable.
    unsafe {
        addr_of_mut!((*oai2).samples_per_sec).write(info.samples_per_sec);
        addr_of_mut!((*oai2).speakers).write(info.speakers);
        addr_of_mut!((*oai2).fixed_buffering).write(info.fixed_buffering);
        addr_of_mut!((*oai2).max_buffering_ms).write(info.max_buffering_ms);
    }
    true
}

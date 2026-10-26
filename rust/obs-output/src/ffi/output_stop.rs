//! C ABI for `libobs/obs-output-stop.c.h`, the output stop and end-capture
//! decisions of `obs-output.c`, declared in `libobs/obs-output-stop.h`
//! without `EXPORT`.
//!
//! `obs-output.c.h` (still C) checks the output (`obs_output_valid()`),
//! tests `context.data`, `raw_audio2`, `signal` and `force`, loads every
//! atomic (`active`, `data_active`, `reconnecting`, `starting`,
//! `delay_active`, `delay_capturing`, `delay_restart_refs` and the
//! end-capture thread's flags) and tests the stopping event where it always
//! did, in their `&&` order, calls `info.stop`, the delayed stop, the
//! encoder stops and shutdowns, the raw disconnects, the service
//! deactivation, the delay cleanup, the frame log, the joins,
//! `pthread_create()` and every signal, and passes `info.flags`,
//! `active_delay_ns`, the loaded restart count and the thread result here.
//! So `struct obs_output` is never mirrored. These shims take no lock, call
//! nothing in libobs and dereference no pointer. They stay hidden
//! (`rust-hidden.txt`). The interleave and mix tests ask the
//! `obs_output_raw_audio_interleave` and `obs_output_raw_audio_mix_on` shims
//! in [`super::output_raw_audio`], and the first mixer the
//! `obs_output_first_mixer` shim in [`super::output_video_checks`].
//!
//! No intentional differences.

use std::ffi::{c_int, c_long};

use crate::output_stop as core;

/// `bool obs_output_stop_encoded(uint32_t flags)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_stop_encoded(flags: u32) -> bool {
    core::encoded(flags)
}

/// `bool obs_output_stop_delay_on(uint64_t active_delay_ns)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_stop_delay_on(active_delay_ns: u64) -> bool {
    core::delay_on(active_delay_ns)
}

/// `bool obs_output_stop_has_video(uint32_t flags)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_stop_has_video(flags: u32) -> bool {
    core::has_video(flags)
}

/// `bool obs_output_stop_has_audio(uint32_t flags)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_stop_has_audio(flags: u32) -> bool {
    core::has_audio(flags)
}

/// `bool obs_output_stop_service(uint32_t flags)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_stop_service(flags: u32) -> bool {
    core::service(flags)
}

/// `bool obs_output_stop_end_delay(long delay_restart_refs)`.
// `long` is `i64` on Linux and macOS and `i32` on Windows.
#[allow(clippy::useless_conversion)]
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_stop_end_delay(delay_restart_refs: c_long) -> bool {
    core::end_delay(i64::from(delay_restart_refs))
}

/// `bool obs_output_stop_thread_failed(int ret)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_stop_thread_failed(ret: c_int) -> bool {
    core::thread_failed(ret)
}

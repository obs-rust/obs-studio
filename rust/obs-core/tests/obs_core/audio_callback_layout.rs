//! Tier 2 layout of the C types the audio tick's shim
//! (`obs_core::ffi::audio_callback`) shares with libobs and copies itself:
//! `struct obs_task_info`, which it pops from the audio task queue, and one
//! row of a source's `audio_output_buf` (`obs_audio_seam_output_row_t`),
//! which the seam hands it. Each size, alignment and field offset is checked
//! against the values the oracle measures from the real headers
//! (`oracle_tick_*`). The other structs it passes (`struct obs_audio_input`,
//! `struct obs_audio_buffering`, `struct ts_info`, `struct
//! audio_output_data` and the step results) are their ports' and are checked
//! in `audio_discard_parity` and `audio_mix_parity`.

use std::mem::{align_of, offset_of, size_of};

use obs_c_oracle::audio_callback as c;
use obs_core::ffi::audio_callback::{OutputRow, obs_task_info};

#[test]
fn obs_task_info_matches_c() {
    // SAFETY: the oracle's sizeof/offsetof functions take nothing.
    unsafe {
        assert_eq!(size_of::<obs_task_info>(), c::oracle_tick_task_info_size());
        assert_eq!(
            align_of::<obs_task_info>(),
            c::oracle_tick_task_info_align()
        );
        assert_eq!(
            offset_of!(obs_task_info, task),
            c::oracle_tick_task_info_offset_task()
        );
        assert_eq!(
            offset_of!(obs_task_info, param),
            c::oracle_tick_task_info_offset_param()
        );
    }
}

#[test]
fn output_row_matches_c() {
    // SAFETY: as above.
    unsafe {
        assert_eq!(size_of::<OutputRow>(), c::oracle_tick_output_row_size());
        assert_eq!(align_of::<OutputRow>(), c::oracle_tick_output_row_align());
    }
}

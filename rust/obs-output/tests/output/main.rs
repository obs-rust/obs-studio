//! obs-output integration tests: a module of the core category binary
//! (`obs-tests-core`, rust/obs-data/tests/core.rs), not a binary of its own.

mod caption_text;
mod caption_text_parity;
mod delay;
mod delay_parity;
mod encoder_audio_getters;
mod encoder_audio_getters_parity;
mod encoder_audio_start;
mod encoder_audio_start_frames;
mod encoder_audio_start_frames_parity;
mod encoder_audio_start_parity;
mod encoder_bind;
mod encoder_bind_parity;
mod encoder_counters;
mod encoder_counters_parity;
mod encoder_dimensions;
mod encoder_dimensions_parity;
mod encoder_frame_rate_divisor;
mod encoder_frame_rate_divisor_parity;
mod encoder_gpu_gates;
mod encoder_gpu_gates_parity;
mod encoder_gpu_rescale;
mod encoder_gpu_rescale_parity;
mod encoder_gpu_scaling_getters;
mod encoder_gpu_scaling_getters_parity;
mod encoder_group_reconfig;
mod encoder_group_reconfig_parity;
mod encoder_last_error;
mod encoder_last_error_parity;
mod encoder_lifecycle;
mod encoder_lifecycle_parity;
mod encoder_metadata;
mod encoder_metadata_parity;
mod encoder_packet_ts;
mod encoder_packet_ts_parity;
mod encoder_pause;
mod encoder_pause_parity;
mod encoder_roi;
mod encoder_roi_parity;
mod encoder_roi_state;
mod encoder_roi_state_parity;
mod encoder_scaling_enabled;
mod encoder_scaling_enabled_parity;
mod encoder_size;
mod encoder_size_parity;
mod encoder_state_queries;
mod encoder_state_queries_parity;
mod interleave;
mod interleave_parity;
mod output_begin;
mod output_begin_parity;
mod output_can_pause;
mod output_can_pause_parity;
mod output_caption;
mod output_caption_parity;
mod output_flags;
mod output_flags_parity;
mod output_handler_getters;
mod output_handler_getters_parity;
mod output_identity;
mod output_identity_parity;
mod output_mixer_getters;
mod output_mixer_getters_parity;
mod output_pause;
mod output_pause_parity;
mod output_pause_queries;
mod output_pause_queries_parity;
mod output_reconnect;
mod output_reconnect_parity;
mod output_service_getter;
mod output_service_getter_parity;
mod output_slots;
mod output_slots_parity;
mod output_start;
mod output_start_parity;
mod output_stop;
mod output_stop_parity;
mod output_supported_codecs;
mod output_supported_codecs_parity;
mod protocols;
mod protocols_parity;
mod raw_audio;
mod raw_audio_parity;
mod raw_audio_trim;
mod raw_audio_trim_parity;
mod service;
mod service_lifecycle;
mod service_lifecycle_parity;
mod service_module_lookup;
mod service_module_lookup_parity;
mod service_parity;
mod service_settings;
mod service_settings_parity;
mod service_state;
mod service_state_parity;
mod service_weak_acquire;
mod service_weak_acquire_parity;
mod service_weak_release;
mod service_weak_release_parity;
mod stop_stats;
mod stop_stats_parity;
mod video_checks;
mod video_checks_parity;

use std::process::Command;
use std::sync::Mutex;

/// The oracle's process-wide service state: the `find_service()` table,
/// the module lists and the lifecycle trace hook, shared by the service
/// parity tests on both sides.
pub(crate) static SERVICE_GLOBALS: Mutex<()> = Mutex::new(());

const MARKER_ENV: &str = "OBS_OUTPUT_ISOLATED_TEST";

/// Runs `test` (its `module_path!()`-based name) alone in a child process
/// of this executable, for tests that swap the process-wide log handler.
/// Returns true in the child, where the test body runs; the parent checks
/// that the child ran exactly that test and passed.
pub(crate) fn isolated(test: &str) -> bool {
    let test = test
        .strip_prefix(concat!(env!("CARGO_CRATE_NAME"), "::"))
        .unwrap_or(test);
    if std::env::var(MARKER_ENV).as_deref() == Ok(test) {
        return true;
    }
    let exe = std::env::current_exe().expect("path of the test binary");
    let out = Command::new(exe)
        .args([test, "--exact", "--test-threads=1", "--nocapture"])
        .env(MARKER_ENV, test)
        .output()
        .expect("start the isolated test process");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "isolated {test} failed:\n{stdout}\n{stderr}"
    );
    assert!(
        stdout.contains("1 passed"),
        "isolated {test} did not run exactly one test:\n{stdout}"
    );
    false
}

mod weak_encoder_identity;

mod weak_encoder_identity_parity;

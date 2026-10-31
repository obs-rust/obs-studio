//! C ABI shims replacing the original libobs `EXPORT` symbols.

pub mod audio_io;
pub mod audio_resampler_drift;
// The audio-io.c engine is built where `with_audio_thread_port!` expands.
crate::platform::with_audio_thread_port! {
    pub mod audio_output;
}
pub mod format_conversion;
pub mod frame_rate;
pub mod media_remux;
pub mod video_fourcc;
pub mod video_frame;
pub mod video_io;
pub mod video_matrices;

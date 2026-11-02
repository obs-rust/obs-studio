//! Brings libobs up like the application does: video, audio, modules and the
//! record and stream outputs (#3317).
//!
//! Every step reports failure as text instead of panicking or aborting; the
//! host keeps going and the MCP tools that need the missing piece return the
//! text as their error.

use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;

use obs_sys::ffi as sys;
use serde_json::json;

use super::{Data, Output, c_string};
use crate::host::{RECORD_OUTPUT, STREAM_OUTPUT, StartOptions};
use crate::startup::Note;

/// What [`configure`] set up. Dropping it releases the outputs, which must
/// happen before `obs_shutdown`.
pub struct Pipeline {
    _outputs: Vec<Output>,
    /// The silent mixer the audio encoder reads when libobs has none; declared
    /// after the outputs so it closes after they release the encoder.
    _null_audio: Option<NullAudio>,
    video_error: Option<String>,
    output_error: Option<String>,
    notes: Vec<Note>,
}

impl Pipeline {
    /// Why there is no video subsystem, if there is none.
    pub fn video_error(&self) -> Option<&str> {
        self.video_error.as_deref()
    }

    /// Why the record or stream output could not be created, if it could not.
    pub fn output_error(&self) -> Option<&str> {
        self.output_error.as_deref()
    }

    /// Every problem found, in order, for the log.
    pub fn notes(&self) -> &[Note] {
        &self.notes
    }
}

/// An owned reference to an `obs_encoder_t`.
struct Encoder(NonNull<sys::obs_encoder_t>);

impl Drop for Encoder {
    fn drop(&mut self) {
        // SAFETY: owns one reference; outputs keep their own.
        unsafe { sys::obs_encoder_release(self.0.as_ptr()) };
    }
}

/// An owned reference to an `obs_service_t`.
struct Service(NonNull<sys::obs_service_t>);

impl Drop for Service {
    fn drop(&mut self) {
        // SAFETY: owns one reference; the output keeps its own.
        unsafe { sys::obs_service_release(self.0.as_ptr()) };
    }
}

/// Resets video and audio, loads the modules and creates the outputs.
///
/// Never fails: a step that cannot complete is recorded in the result.
pub fn configure(options: &StartOptions) -> Pipeline {
    let mut notes = Vec::new();
    let mut pipeline = Pipeline {
        _outputs: Vec::new(),
        _null_audio: None,
        video_error: None,
        output_error: None,
        notes: Vec::new(),
    };

    add_paths(options, &mut notes);
    if let Err(error) = reset_video(options) {
        pipeline.video_error = Some(error.clone());
        notes.push(error);
    }
    // No audio is not a startup failure: the run continues with video and
    // the note goes to the OBS log, not stderr (see `crate::startup`).
    let mut audio_error = reset_audio().err();
    let mut audio_warning = audio_error
        .as_ref()
        .map(|error| format!("{error}; continuing without audio"));
    load_modules(options, &mut notes);
    // SAFETY: libobs is started.
    unsafe { sys::obs_post_load_modules() };

    if pipeline.video_error.is_some() {
        pipeline.output_error = Some("the video subsystem is not available".to_owned());
    } else {
        let media = match audio_error.take() {
            // SAFETY: libobs is started.
            None => Ok(unsafe { sys::obs_get_audio() }),
            // libobs's mixer is down: feed the audio encoder silence at the
            // same rate and layout, so recording and streaming still start
            // with video (#3491).
            Some(error) => NullAudio::open().map(|null| {
                audio_warning = Some(format!(
                    "{error}; continuing with a silent audio track"
                ));
                pipeline._null_audio.insert(null).0.as_ptr()
            }),
        };
        match media.and_then(|media| create_outputs(&options.record_dir, media)) {
            Ok((outputs, problems)) => {
                pipeline._outputs = outputs;
                if !problems.is_empty() {
                    pipeline.output_error = Some(problems.join("; "));
                    notes.extend(problems);
                }
            }
            Err(error) => {
                pipeline.output_error = Some(error.clone());
                notes.push(error);
            }
        }
    }
    pipeline.notes = notes
        .into_iter()
        .map(Note::error)
        .chain(audio_warning.map(Note::warning))
        .collect();
    pipeline
}

/// Registers the effect-file and plugin directories.
fn add_paths(options: &StartOptions, notes: &mut Vec<String>) {
    if let Some(path) = &options.data_path {
        match path_c(&with_separator(path)) {
            // libobs appends the file name to the path as is, so end it with a
            // separator. SAFETY: NUL-terminated; libobs copies the string.
            Ok(path) => unsafe { sys::obs_add_data_path(path.as_ptr()) },
            Err(error) => notes.push(error),
        }
    }
}

/// Loads the plugins in the module directory. `obs_load_all_modules` is a
/// deprecated no-op; the plugin loader takes the paths directly.
fn load_modules(options: &StartOptions, notes: &mut Vec<String>) {
    let Some((bin, data)) = &options.module_path else {
        return;
    };
    let (bin, data) = match (path_c(bin), path_c(data)) {
        (Ok(bin), Ok(data)) => (bin, data),
        (Err(error), _) | (_, Err(error)) => return notes.push(error),
    };
    let mut info = sys::obs_runtime_module_info {
        path_info: sys::obs_runtime_module_path {
            binary: bin.as_ptr(),
            data: data.as_ptr(),
        },
        type_: sys::OBS_MODULE_TYPE_PLUGIN,
        name: std::ptr::null(),
    };
    let mut failures = sys::obs_module_failure_info {
        failed_modules: std::ptr::null_mut(),
        count: 0,
    };
    // SAFETY: both structs are fully initialized and the strings outlive the
    // call; libobs copies what it keeps.
    unsafe { sys::obs_load_plugins(&raw mut info, &raw mut failures) };
    if failures.count > 0 {
        notes.push(format!("{} module(s) failed to load", failures.count));
    }
    // SAFETY: filled in by `obs_load_plugins` above.
    unsafe { sys::obs_module_failure_info_free(&raw mut failures) };
}

/// `path` ending in a separator.
fn with_separator(path: &Path) -> PathBuf {
    let mut text = path.as_os_str().to_owned();
    if !path.to_string_lossy().ends_with(std::path::MAIN_SEPARATOR) {
        text.push(std::path::MAIN_SEPARATOR_STR);
    }
    PathBuf::from(text)
}

fn path_c(path: &Path) -> Result<CString, String> {
    c_string(&path.to_string_lossy(), "path")
}

fn reset_video(options: &StartOptions) -> Result<(), String> {
    let module = c_string(&options.graphics_module, "graphics module")?;
    let mut info = sys::obs_video_info {
        graphics_module: module.as_ptr(),
        fps_num: options.fps,
        fps_den: 1,
        base_width: options.size.0,
        base_height: options.size.1,
        output_width: options.size.0,
        output_height: options.size.1,
        output_format: sys::VIDEO_FORMAT_NV12,
        adapter: 0,
        gpu_conversion: true,
        colorspace: sys::VIDEO_CS_709,
        range: sys::VIDEO_RANGE_PARTIAL,
        scale_type: sys::OBS_SCALE_BICUBIC,
    };
    // SAFETY: `info` is fully initialized and `module` outlives the call;
    // libobs copies what it keeps.
    let code = unsafe { sys::obs_reset_video(&raw mut info) };
    if code == sys::OBS_VIDEO_SUCCESS {
        return Ok(());
    }
    let why = match code {
        sys::OBS_VIDEO_MODULE_NOT_FOUND => "the graphics module was not found",
        sys::OBS_VIDEO_NOT_SUPPORTED => "the graphics adapter lacks required capabilities",
        sys::OBS_VIDEO_INVALID_PARAM => "invalid video parameters",
        sys::OBS_VIDEO_CURRENTLY_ACTIVE => "video is already active",
        _ => "the graphics context could not be created (is there a display?)",
    };
    Err(format!(
        "cannot start video with graphics module '{}': {why} (code {code})",
        options.graphics_module
    ))
}

/// Set by [`fail_audio_mixer`]; consumed by the next [`reset_audio`].
#[cfg(feature = "fault-injection")]
static FAIL_AUDIO_MIXER: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Test-only fault injection: the next [`configure`] skips `obs_reset_audio`
/// and reports it failed, as when libobs cannot open its mixer.
#[cfg(feature = "fault-injection")]
pub fn fail_audio_mixer() {
    FAIL_AUDIO_MIXER.store(true, std::sync::atomic::Ordering::SeqCst);
}

fn reset_audio() -> Result<(), String> {
    #[cfg(feature = "fault-injection")]
    if FAIL_AUDIO_MIXER.swap(false, std::sync::atomic::Ordering::SeqCst) {
        return Err("cannot start audio at 48 kHz stereo (injected)".to_owned());
    }
    let info = sys::obs_audio_info {
        samples_per_sec: AUDIO_RATE,
        speakers: sys::SPEAKERS_STEREO,
    };
    // SAFETY: `info` is fully initialized; libobs copies it.
    if unsafe { sys::obs_reset_audio(&raw const info) } {
        Ok(())
    } else {
        Err("cannot start audio at 48 kHz stereo".to_owned())
    }
}

fn video_encoder() -> Result<Encoder, String> {
    let settings = Data::from_json(&json!({
        "rate_control": "CBR", "bitrate": 2500, "keyint_sec": 2, "preset": "veryfast",
    }))?;
    // SAFETY: strings are static NUL-terminated; `settings` is live; the
    // result is an owned reference or null.
    let encoder = unsafe {
        sys::obs_video_encoder_create(
            c"obs_x264".as_ptr(),
            c"simple_h264_mcp".as_ptr(),
            settings.0.as_ptr(),
            std::ptr::null_mut(),
        )
    };
    let encoder = NonNull::new(encoder)
        .map(Encoder)
        .ok_or("no video encoder 'obs_x264' (is the obs-x264 plugin loaded?)")?;
    // SAFETY: the encoder is live and video is up; libobs keeps what it needs.
    unsafe { sys::obs_encoder_set_video(encoder.0.as_ptr(), sys::obs_get_video()) };
    Ok(encoder)
}

/// Sample rate and layout of the main mix, and of the null audio path.
const AUDIO_RATE: u32 = 48_000;

/// A standalone `audio_t` that outputs silence, standing in for libobs's
/// mixer when `obs_reset_audio` fails. It only exists on that path.
struct NullAudio(NonNull<sys::audio_t>);

impl NullAudio {
    fn open() -> Result<Self, String> {
        /// The mixes arrive zeroed (`input_and_output` in audio-io), so
        /// leaving them alone is silence; the block starts at `start_ts`.
        unsafe extern "C" fn silence(
            _param: *mut std::ffi::c_void,
            start_ts: u64,
            _end_ts: u64,
            new_ts: *mut u64,
            _active_mixers: u32,
            _mixes: *mut sys::audio_output_data,
        ) -> bool {
            // SAFETY: audio-io passes a valid out pointer.
            unsafe { *new_ts = start_ts };
            true
        }
        let mut info = sys::audio_output_info {
            name: c"null_audio_mcp".as_ptr(),
            samples_per_sec: AUDIO_RATE,
            format: sys::AUDIO_FORMAT_FLOAT_PLANAR,
            speakers: sys::SPEAKERS_STEREO,
            input_callback: Some(silence),
            input_param: std::ptr::null_mut(),
        };
        let mut audio = std::ptr::null_mut();
        // SAFETY: `info` is fully initialized and the name is static; on
        // success `audio` is an owned output closed by `Drop`.
        let code = unsafe { sys::audio_output_open(&raw mut audio, &raw mut info) };
        match NonNull::new(audio) {
            Some(audio) if code == sys::AUDIO_OUTPUT_SUCCESS => Ok(Self(audio)),
            _ => Err(format!("cannot open a silent audio output either ({code})")),
        }
    }
}

impl Drop for NullAudio {
    fn drop(&mut self) {
        // SAFETY: owned; the outputs and encoders that read it are released.
        unsafe { sys::audio_output_close(self.0.as_ptr()) };
    }
}

fn audio_encoder(media: *mut sys::audio_t) -> Result<Encoder, String> {
    let settings = Data::from_json(&json!({"bitrate": 160}))?;
    // SAFETY: as in `video_encoder`; mixer 0 is the main mix.
    let encoder = unsafe {
        sys::obs_audio_encoder_create(
            c"ffmpeg_aac".as_ptr(),
            c"simple_aac_mcp".as_ptr(),
            settings.0.as_ptr(),
            0,
            std::ptr::null_mut(),
        )
    };
    let encoder = NonNull::new(encoder)
        .map(Encoder)
        .ok_or("no audio encoder 'ffmpeg_aac' (is the obs-ffmpeg plugin loaded?)")?;
    // SAFETY: the encoder is live; `media` is libobs's mixer or the live
    // null audio output.
    unsafe { sys::obs_encoder_set_audio(encoder.0.as_ptr(), media) };
    Ok(encoder)
}

fn attach(output: &Output, video: &Encoder, audio: &Encoder) {
    // SAFETY: all three are live; the output takes its own references.
    unsafe {
        sys::obs_output_set_video_encoder(output.0.as_ptr(), video.0.as_ptr());
        sys::obs_output_set_audio_encoder(output.0.as_ptr(), audio.0.as_ptr(), 0);
    }
}

/// Creates the record output and the stream output, sharing the encoders.
/// Returns what was created and a message for each part that was not.
fn create_outputs(
    record_dir: &Path,
    audio_media: *mut sys::audio_t,
) -> Result<(Vec<Output>, Vec<String>), String> {
    let video = video_encoder()?;
    let audio = audio_encoder(audio_media)?;
    let mut outputs = Vec::new();
    let mut problems = Vec::new();

    let path = record_dir.join("obs-mcp.mkv");
    match Output::create(
        "ffmpeg_muxer",
        RECORD_OUTPUT,
        &json!({"path": path.to_string_lossy()}),
    ) {
        Ok(output) => {
            attach(&output, &video, &audio);
            outputs.push(output);
        }
        Err(error) => problems.push(format!("record output: {error}")),
    }

    match stream_output(&video, &audio) {
        Ok(output) => outputs.push(output),
        Err(error) => problems.push(format!("stream output: {error}")),
    }
    Ok((outputs, problems))
}

fn stream_output(video: &Encoder, audio: &Encoder) -> Result<Output, String> {
    let output = Output::create("rtmp_output", STREAM_OUTPUT, &json!({}))?;
    let settings = Data::from_json(&json!({
        "server": "rtmp://localhost/live", "key": "",
    }))?;
    // SAFETY: strings are static NUL-terminated; `settings` is live; the
    // result is an owned reference or null.
    let service = unsafe {
        sys::obs_service_create(
            c"rtmp_custom".as_ptr(),
            c"simple_stream_service".as_ptr(),
            settings.0.as_ptr(),
            std::ptr::null_mut(),
        )
    };
    let service = NonNull::new(service)
        .map(Service)
        .ok_or("no service 'rtmp_custom' (is the rtmp-services plugin loaded?)")?;
    // SAFETY: both are live; the output takes its own reference to the service.
    unsafe { sys::obs_output_set_service(output.0.as_ptr(), service.0.as_ptr()) };
    attach(&output, video, audio);
    Ok(output)
}

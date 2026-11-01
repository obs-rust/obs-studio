//! Brings libobs up like the application does: video, audio, modules and the
//! record and stream outputs (#3317).
//!
//! Every step reports failure as text instead of panicking or aborting; the
//! host keeps going and the MCP tools that need the missing piece return the
//! text as their error.

use std::ffi::CString;
use std::path::Path;
use std::ptr::NonNull;

use obs_sys::ffi as sys;
use serde_json::json;

use super::{Data, Output, c_string};
use crate::host::{RECORD_OUTPUT, STREAM_OUTPUT, StartOptions};

/// What [`configure`] set up. Dropping it releases the outputs, which must
/// happen before `obs_shutdown`.
pub struct Pipeline {
    _outputs: Vec<Output>,
    video_error: Option<String>,
    output_error: Option<String>,
    notes: Vec<String>,
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
    pub fn notes(&self) -> &[String] {
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
        video_error: None,
        output_error: None,
        notes: Vec::new(),
    };

    add_paths(options, &mut notes);
    if let Err(error) = reset_video(options) {
        pipeline.video_error = Some(error.clone());
        notes.push(error);
    }
    if let Err(error) = reset_audio() {
        notes.push(error);
    }
    // SAFETY: libobs is started; the paths were registered above.
    unsafe {
        sys::obs_load_all_modules();
        sys::obs_post_load_modules();
    }

    if pipeline.video_error.is_some() {
        pipeline.output_error = Some("the video subsystem is not available".to_owned());
    } else {
        match create_outputs(&options.record_dir) {
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
    pipeline.notes = notes;
    pipeline
}

/// Registers the effect-file and plugin directories.
fn add_paths(options: &StartOptions, notes: &mut Vec<String>) {
    if let Some(path) = &options.data_path {
        match path_c(path) {
            // SAFETY: NUL-terminated; libobs copies the string.
            Ok(path) => unsafe { sys::obs_add_data_path(path.as_ptr()) },
            Err(error) => notes.push(error),
        }
    }
    if let Some((bin, data)) = &options.module_path {
        match (path_c(bin), path_c(data)) {
            // SAFETY: both NUL-terminated; libobs copies the strings.
            (Ok(bin), Ok(data)) => unsafe { sys::obs_add_module_path(bin.as_ptr(), data.as_ptr()) },
            (Err(error), _) | (_, Err(error)) => notes.push(error),
        }
    }
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

fn reset_audio() -> Result<(), String> {
    let info = sys::obs_audio_info {
        samples_per_sec: 48_000,
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

fn audio_encoder() -> Result<Encoder, String> {
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
    // SAFETY: the encoder is live; audio is up or the pointer is null, which
    // libobs tolerates until the output starts.
    unsafe { sys::obs_encoder_set_audio(encoder.0.as_ptr(), sys::obs_get_audio()) };
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
fn create_outputs(record_dir: &Path) -> Result<(Vec<Output>, Vec<String>), String> {
    let video = video_encoder()?;
    let audio = audio_encoder()?;
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

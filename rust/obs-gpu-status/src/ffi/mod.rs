//! C ABI for the frontend (`frontend/utility/obs-gpu-status.h`). The
//! frontend links it through the staticlib rust/obs-gpu-status-frontend,
//! like obs-nvenc links obs-nvenc-plugin, so libobs' exported symbol set
//! does not change.

use std::ffi::{CStr, CString, c_char, c_int};
use std::sync::{Mutex, Once};
use std::time::{Duration, Instant};

use crate::report::Report;
use crate::usage::{Sampler, UsageSample};
use crate::{FrameWindow, LogCapture, RenderAdapter, RenderHint};

/// The graphics log lines [`obs_gpu_status_log_line`] kept.
static LOG: Mutex<LogCapture> = Mutex::new(LogCapture::new());

/// Offer one OBS log line; the ones about graphics are quoted in the
/// dialog (#944). Null is ignored.
///
/// # Safety
/// `line` is null or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_gpu_status_log_line(line: *const c_char) {
    if line.is_null() {
        return;
    }
    // SAFETY: the caller passes a NUL-terminated string.
    let line = unsafe { CStr::from_ptr(line) }.to_string_lossy();
    // Every log line comes through here: filter before taking the lock.
    if crate::is_graphics_log_line(&line) {
        LOG.lock().unwrap_or_else(|e| e.into_inner()).push(&line);
    }
}

/// The frame-time window [`obs_gpu_status_sample`] fills.
static FRAMES: Mutex<FrameWindow> = Mutex::new(FrameWindow::new());

/// Record libobs' one-second average frame time
/// (`obs_get_average_frame_time_ns`); call once a second. 0 is skipped.
#[unsafe(no_mangle)]
pub extern "C" fn obs_gpu_status_sample(frame_ns: u64) {
    FRAMES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(frame_ns);
}

/// The system's GPUs, probed once per process (`nvidia-smi` is too slow to
/// run on every refresh, or on the startup path: see
/// [`obs_gpu_status_prefetch`]).
static DEVICES: crate::DeviceCache = crate::DeviceCache::new();

/// Start probing the system's GPUs on a thread, so that the first
/// [`obs_gpu_status_report`] (after graphics init) finds them probed. Call
/// once, early; later calls do nothing.
#[unsafe(no_mangle)]
pub extern "C" fn obs_gpu_status_prefetch() {
    DEVICES.prefetch(crate::platform::system_devices);
}

/// Build the report for `adapter` (null when graphics init failed), the
/// system's GPUs, probed once per process (`nvidia-smi` is too slow to run
/// on every refresh), and the median of the sampled frame times
/// ([`FrameWindow::stats`]) against `interval_ns`, with the last
/// utilization sample ([`obs_gpu_status_usage_start`]). Writes
/// 0 (green), 1 (yellow) or 2 (red) to `state` when it is not null, and
/// returns the `gpuAcceleration` JSON, freed with [`obs_gpu_status_free`].
///
/// # Safety
/// `adapter` is null or a NUL-terminated string; `state` is null or
/// writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_gpu_status_report(
    adapter: *const c_char,
    interval_ns: u64,
    state: *mut c_int,
) -> *mut c_char {
    let adapter = if adapter.is_null() {
        None
    } else {
        // SAFETY: the caller passes a NUL-terminated string.
        Some(
            unsafe { CStr::from_ptr(adapter) }
                .to_string_lossy()
                .into_owned(),
        )
    };
    let frames = FRAMES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .stats(interval_ns);
    let devices = DEVICES.get(crate::platform::system_devices);
    let log_lines = LOG.lock().unwrap_or_else(|e| e.into_inner()).lines();
    let published = *USAGE.lock().unwrap_or_else(|e| e.into_inner());
    let (usage, stale) = published.map_or((None, false), |p| {
        let usage = crate::usage::current(p.sample, p.taken, p.interval, Instant::now());
        (usage, p.sample.is_some() && usage.is_none())
    });
    let report = Report::new(adapter.as_deref(), devices, frames)
        .with_log_lines(log_lines)
        .with_usage(usage)
        .with_usage_stale(stale);
    if !state.is_null() {
        // SAFETY: the caller passes a writable int.
        unsafe { *state = report.state as c_int };
    }
    // The JSON escapes control characters, so it has no interior NUL.
    CString::new(report.to_json()).map_or(std::ptr::null_mut(), CString::into_raw)
}

/// Free a string from [`obs_gpu_status_report`]; null is ignored.
///
/// # Safety
/// `json` is null or came from [`obs_gpu_status_report`] and is not used
/// again.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_gpu_status_free(json: *mut c_char) {
    if !json.is_null() {
        // SAFETY: allocated by CString::into_raw in obs_gpu_status_report.
        drop(unsafe { CString::from_raw(json) });
    }
}

/// What [`obs_gpu_status_usage_start`] last said about the render adapter,
/// and a generation that tells the sampling thread it changed.
struct UsageHint {
    render_node: Option<String>,
    adapter_index: Option<u32>,
    generation: u64,
}

static USAGE_HINT: Mutex<UsageHint> = Mutex::new(UsageHint {
    render_node: None,
    adapter_index: None,
    generation: 0,
});

/// The sampling thread's last reading (`None` without a source), when it
/// was taken, and the pause before the next one (#1022).
#[derive(Clone, Copy)]
struct Published {
    sample: Option<UsageSample>,
    taken: Instant,
    interval: Duration,
}

/// The last [`Published`] reading; `None` before the first sample.
static USAGE: Mutex<Option<Published>> = Mutex::new(None);

/// Lines for the OBS log from the sampling thread, until the frontend takes
/// them ([`obs_gpu_status_take_log`]).
static USAGE_LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());

static USAGE_THREAD: Once = Once::new();

/// Start sampling the render adapter's utilization on a thread, once a
/// second or less often after slow samples (#1025, #1022), so the report
/// never waits for a source. Call after each
/// video reset: a later call changes the adapter; it does not start a
/// second thread. `render_node` is the Linux EGL render node (adapter 0's
/// name in `gs_enum_adapters`), `adapter_index` the Windows
/// `obs_video_info.adapter`, negative when not known.
///
/// # Safety
/// `render_node` is null or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_gpu_status_usage_start(
    render_node: *const c_char,
    adapter_index: c_int,
) {
    let render_node = if render_node.is_null() {
        None
    } else {
        // SAFETY: the caller passes a NUL-terminated string.
        Some(
            unsafe { CStr::from_ptr(render_node) }
                .to_string_lossy()
                .into_owned(),
        )
    };
    {
        let mut hint = USAGE_HINT.lock().unwrap_or_else(|e| e.into_inner());
        hint.render_node = render_node;
        hint.adapter_index = u32::try_from(adapter_index).ok();
        hint.generation += 1;
    }
    USAGE_THREAD.call_once(|| {
        // Without the thread the label shows "GPU: n/a".
        let _ = std::thread::Builder::new()
            .name("obs-gpu-usage".to_owned())
            .spawn(sample_usage);
    });
}

/// The sampling thread: resolve the render adapter when the hint changes,
/// then sample, pausing [`crate::usage::next_interval`] after each sample.
fn sample_usage() {
    let mut sampler = Sampler::new(crate::platform::usage_sources());
    let mut generation = 0;
    let mut adapter = RenderAdapter::Unknown;
    loop {
        let changed = {
            let hint = USAGE_HINT.lock().unwrap_or_else(|e| e.into_inner());
            (hint.generation != generation).then(|| {
                generation = hint.generation;
                (hint.render_node.clone(), hint.adapter_index)
            })
        };
        if let Some((render_node, adapter_index)) = changed {
            adapter = crate::platform::render_adapter(RenderHint {
                render_node: render_node.as_deref(),
                adapter_index,
            });
        }
        let start = Instant::now();
        let sample = sampler.sample(&adapter);
        let interval = crate::usage::next_interval(start.elapsed());
        let demotions = sampler.take_demotions();
        if !demotions.is_empty() {
            let mut log = USAGE_LOG.lock().unwrap_or_else(|e| e.into_inner());
            log.extend(demotions.iter().map(demotion_line));
        }
        *USAGE.lock().unwrap_or_else(|e| e.into_inner()) = Some(Published {
            sample,
            taken: Instant::now(),
            interval,
        });
        std::thread::sleep(interval);
    }
}

/// The OBS log line for a source that is no longer asked.
fn demotion_line(d: &crate::usage::Demotion) -> String {
    let times: Vec<String> = d
        .times
        .iter()
        .map(|t| format!("{} ms", t.as_millis()))
        .collect();
    format!(
        "GPU usage: {} is too slow ({}); not asked again in this session",
        d.source.as_str(),
        times.join(", ")
    )
}

/// The OBS log lines from the sampling thread since the last call, one per
/// line, or null when there are none. Free the string with
/// [`obs_gpu_status_free`].
#[unsafe(no_mangle)]
pub extern "C" fn obs_gpu_status_take_log() -> *mut c_char {
    let lines = std::mem::take(&mut *USAGE_LOG.lock().unwrap_or_else(|e| e.into_inner()));
    if lines.is_empty() {
        return std::ptr::null_mut();
    }
    // Source names and times only: no interior NUL.
    CString::new(lines.join("\n")).map_or(std::ptr::null_mut(), CString::into_raw)
}

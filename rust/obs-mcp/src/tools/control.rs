//! Device, media and measured pipeline control (#4446).
use super::EmptyInput;
use crate::server::{ToolError, ToolRegistry};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::rc::Rc;

/// Bounded start options, extended by the wait domain.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StartInput {
    /// Wait until the output reports active.
    #[serde(default)]
    pub wait: bool,
    /// Enable the paired microphone before a stopped output starts.
    #[serde(default = "enabled")]
    pub audio_enabled: bool,
    /// One-based OBS audio mix (1 through 6).
    #[serde(default = "first_mix")]
    pub audio_track: u32,
}
fn enabled() -> bool {
    true
}
fn first_mix() -> u32 {
    1
}
impl Default for StartInput {
    fn default() -> Self {
        Self {
            wait: false,
            audio_enabled: true,
            audio_track: 1,
        }
    }
}
/// Virtual Camera and its paired audio source.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct VcamStatus {
    /// Whether the registered output exists.
    pub configured: bool,
    /// Video output is active.
    pub active: bool,
    /// Paired virtual microphone is active, when the plugin supports the query.
    pub vmic_active: Option<bool>,
}
/// Virtual microphone state.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct VmicStatus {
    /// Active according to virtualcam_audio; null means unsupported.
    pub active: Option<bool>,
    /// Runtime source name, when known.
    pub source_name: Option<String>,
}
/// A media source selection.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MediaInput {
    /// Name of an existing media source.
    pub name: String,
}
/// libobs media state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MediaState {
    /// Not a controllable media source.
    #[default]
    None,
    /// Playing.
    Playing,
    /// Opening.
    Opening,
    /// Buffering.
    Buffering,
    /// Paused.
    Paused,
    /// Stopped.
    Stopped,
    /// Playback reached the end.
    Ended,
    /// Playback failed.
    Error,
}
/// Media playback snapshot, in milliseconds.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct MediaStatus {
    /// Source name.
    pub name: String,
    /// Current media state.
    pub state: MediaState,
    /// Total duration, if known.
    pub duration_ms: Option<i64>,
    /// Playback position, if known.
    pub position_ms: Option<i64>,
}
/// One live pipeline stage; null values are unmeasured, never synthetic zeroes.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct StageStats {
    /// Currently running.
    pub live: bool,
    /// Counter generation, when the provider exposes one.
    pub generation: Option<u64>,
    /// Observed minimum stage residence time, if measured.
    pub latency_min_ns: Option<u64>,
    /// Observed maximum stage residence time, if measured.
    pub latency_max_ns: Option<u64>,
    /// Frames handled since the counter baseline.
    pub frames_total: Option<u64>,
    /// Dropped frames since baseline, if measured.
    pub frames_dropped: Option<u64>,
    /// Underruns/overruns since baseline, if measured.
    pub xruns: Option<u64>,
}
/// Render and device stages.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct Stages {
    /// OBS render stage.
    pub render: StageStats,
    /// Virtual Camera write stage.
    pub vcam: StageStats,
    /// Virtual microphone stage.
    pub vmic: StageStats,
}
/// End-to-end measurement provenance.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct SyncStats {
    /// Measured audio minus video offset; null until independently measured.
    pub av_offset_ms: Option<f64>,
    /// Measurement source; queue estimates are not end-to-end measurements.
    pub measured_from: String,
}
/// GetStats-style render counters.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct RenderStats {
    /// Actual render FPS.
    pub active_fps: f64,
    /// Average render duration, not end-to-end latency.
    pub average_frame_time_ns: u64,
    /// Frames rendered since baseline.
    pub render_total_frames: u64,
    /// Frames lagged since baseline.
    pub render_skipped_frames: u64,
}
/// Typed pipeline statistics.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct StatsSnapshot {
    /// Per-stage measured counters and bounds.
    pub stages: Stages,
    /// End-to-end sync, with explicit provenance.
    pub end_to_end: SyncStats,
    /// Render counters.
    pub render: RenderStats,
}
/// Graph snapshot useful for diagnosing missing sources and outputs.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct PipelineDump {
    /// Program source name.
    pub program: Option<String>,
    /// Source names in stable order.
    pub sources: Vec<String>,
    /// Virtual output state.
    pub vcam: VcamStatus,
    /// Current pipeline measurements.
    pub stats: StatsSnapshot,
}
/// Backend operations take typed requests and return typed responses.
pub trait ControlBackend {
    /// Starts or observes an already-running camera.
    /// # Errors
    /// Reports missing plugins, unavailable devices or processing failures.
    fn vcam_start(&self, request: StartInput) -> Result<VcamStatus, ToolError>;
    /// Stops or observes an already-stopped camera.
    /// # Errors
    /// Reports unavailable output configuration.
    fn vcam_stop(&self, request: EmptyInput) -> Result<VcamStatus, ToolError>;
    /// Camera status.
    /// # Errors
    /// Reports query failures.
    fn vcam_status(&self, request: EmptyInput) -> Result<VcamStatus, ToolError>;
    /// Microphone status.
    /// # Errors
    /// Reports query failures.
    fn vmic_status(&self, request: EmptyInput) -> Result<VmicStatus, ToolError>;
    /// Restarts a media source.
    /// # Errors
    /// Reports missing or non-media sources.
    fn media_restart(&self, request: MediaInput) -> Result<MediaStatus, ToolError>;
    /// Media status.
    /// # Errors
    /// Reports missing sources.
    fn media_status(&self, request: MediaInput) -> Result<MediaStatus, ToolError>;
    /// Statistics since the current baseline.
    /// # Errors
    /// Reports query failures.
    fn stats_get(&self, request: EmptyInput) -> Result<StatsSnapshot, ToolError>;
    /// Sets a new baseline and returns the resulting snapshot.
    /// # Errors
    /// Reports query failures.
    fn stats_reset(&self, request: EmptyInput) -> Result<StatsSnapshot, ToolError>;
    /// Graph and measurements.
    /// # Errors
    /// Reports query failures.
    fn pipeline_dump(&self, request: EmptyInput) -> Result<PipelineDump, ToolError>;
}
/// Difference from a baseline; a recreated counter begins a new generation.
#[must_use]
pub fn counter_delta(current: u64, baseline: u64) -> u64 {
    if current >= baseline {
        current - baseline
    } else {
        current
    }
}

typed_tool!(
    VcamStart,
    "vcam_start",
    ControlBackend,
    StartInput,
    VcamStatus,
    IDEMPOTENT,
    "vcam start: control and inspect OBS directly.",
    |backend, input| { backend.vcam_start(input) }
);

typed_tool!(
    VcamStop,
    "vcam_stop",
    ControlBackend,
    EmptyInput,
    VcamStatus,
    IDEMPOTENT,
    "vcam stop: control and inspect OBS directly.",
    |backend, input| { backend.vcam_stop(input) }
);

typed_tool!(
    VcamGet,
    "vcam_status",
    ControlBackend,
    EmptyInput,
    VcamStatus,
    READ_ONLY,
    "vcam status: control and inspect OBS directly.",
    |backend, input| { backend.vcam_status(input) }
);

typed_tool!(
    VmicGet,
    "vmic_status",
    ControlBackend,
    EmptyInput,
    VmicStatus,
    READ_ONLY,
    "vmic status: control and inspect OBS directly.",
    |backend, input| { backend.vmic_status(input) }
);

typed_tool!(
    MediaRestart,
    "media_restart",
    ControlBackend,
    MediaInput,
    MediaStatus,
    MUTATING,
    "media restart: control and inspect OBS directly.",
    |backend, input| { backend.media_restart(input) }
);

typed_tool!(
    MediaGet,
    "media_status",
    ControlBackend,
    MediaInput,
    MediaStatus,
    READ_ONLY,
    "media status: control and inspect OBS directly.",
    |backend, input| { backend.media_status(input) }
);

typed_tool!(
    StatsGet,
    "stats_get",
    ControlBackend,
    EmptyInput,
    StatsSnapshot,
    READ_ONLY,
    "stats get: control and inspect OBS directly.",
    |backend, input| { backend.stats_get(input) }
);

typed_tool!(
    StatsReset,
    "stats_reset",
    ControlBackend,
    EmptyInput,
    StatsSnapshot,
    MUTATING,
    "stats reset: control and inspect OBS directly.",
    |backend, input| { backend.stats_reset(input) }
);

typed_tool!(
    PipelineGet,
    "pipeline_dump",
    ControlBackend,
    EmptyInput,
    PipelineDump,
    READ_ONLY,
    "pipeline dump: control and inspect OBS directly.",
    |backend, input| { backend.pipeline_dump(input) }
);

/// Registers device, media and measurement tools.
pub fn register(registry: &mut ToolRegistry, backend: &Rc<dyn ControlBackend>) {
    registry.register(Box::new(VcamStart {
        backend: Rc::clone(backend),
    }));
    registry.register(Box::new(VcamStop {
        backend: Rc::clone(backend),
    }));
    registry.register(Box::new(VcamGet {
        backend: Rc::clone(backend),
    }));
    registry.register(Box::new(VmicGet {
        backend: Rc::clone(backend),
    }));
    registry.register(Box::new(MediaRestart {
        backend: Rc::clone(backend),
    }));
    registry.register(Box::new(MediaGet {
        backend: Rc::clone(backend),
    }));
    registry.register(Box::new(StatsGet {
        backend: Rc::clone(backend),
    }));
    registry.register(Box::new(StatsReset {
        backend: Rc::clone(backend),
    }));
    registry.register(Box::new(PipelineGet {
        backend: Rc::clone(backend),
    }));
}

/// Generated tool contracts.
#[must_use]
pub fn schemas() -> Vec<serde_json::Value> {
    vec![
        crate::server::tool_schema::<VcamStart>(),
        crate::server::tool_schema::<VcamStop>(),
        crate::server::tool_schema::<VcamGet>(),
        crate::server::tool_schema::<VmicGet>(),
        crate::server::tool_schema::<MediaRestart>(),
        crate::server::tool_schema::<MediaGet>(),
        crate::server::tool_schema::<StatsGet>(),
        crate::server::tool_schema::<StatsReset>(),
        crate::server::tool_schema::<PipelineGet>(),
    ]
}

impl StatsSnapshot {
    /// Subtract counter baselines while leaving latency gauges unchanged.
    #[must_use]
    pub fn relative_to(mut self, baseline: &Self) -> Self {
        fn stage(now: &mut StageStats, old: &StageStats) {
            let delta =
                |n: Option<u64>, b: Option<u64>| n.map(|v| counter_delta(v, b.unwrap_or(0)));
            now.frames_total = delta(now.frames_total, old.frames_total);
            now.frames_dropped = delta(now.frames_dropped, old.frames_dropped);
            now.xruns = delta(now.xruns, old.xruns);
        }
        stage(&mut self.stages.render, &baseline.stages.render);
        stage(&mut self.stages.vcam, &baseline.stages.vcam);
        stage(&mut self.stages.vmic, &baseline.stages.vmic);
        self.render.render_total_frames = counter_delta(
            self.render.render_total_frames,
            baseline.render.render_total_frames,
        );
        self.render.render_skipped_frames = counter_delta(
            self.render.render_skipped_frames,
            baseline.render.render_skipped_frames,
        );
        self
    }
}

/// Stateful baselines detect provider recreation even after raw counters exceed
/// the previous baseline. Latency bounds are gauges, reset by the provider.
#[derive(Clone, Debug)]
pub struct StatsBaseline {
    baseline: StatsSnapshot,
    previous: StatsSnapshot,
}
impl StatsBaseline {
    /// Establishes a baseline from the current raw snapshot.
    #[must_use]
    pub fn new(snapshot: StatsSnapshot) -> Self {
        Self {
            previous: snapshot.clone(),
            baseline: snapshot,
        }
    }
    /// Observe a raw snapshot, resetting rolled-back/recreated counter baselines.
    pub fn apply(&mut self, snapshot: StatsSnapshot) -> StatsSnapshot {
        fn stage(now: &StageStats, previous: &StageStats, baseline: &mut StageStats) {
            let recreated = now.generation.is_some() && now.generation != previous.generation;
            let fix = |current: Option<u64>, prior: Option<u64>, base: &mut Option<u64>| {
                if recreated || current.zip(prior).is_some_and(|(n, p)| n < p) {
                    *base = current.map(|_| 0);
                }
            };
            fix(
                now.frames_total,
                previous.frames_total,
                &mut baseline.frames_total,
            );
            fix(
                now.frames_dropped,
                previous.frames_dropped,
                &mut baseline.frames_dropped,
            );
            fix(now.xruns, previous.xruns, &mut baseline.xruns);
            baseline.generation = now.generation;
        }
        stage(
            &snapshot.stages.render,
            &self.previous.stages.render,
            &mut self.baseline.stages.render,
        );
        stage(
            &snapshot.stages.vcam,
            &self.previous.stages.vcam,
            &mut self.baseline.stages.vcam,
        );
        stage(
            &snapshot.stages.vmic,
            &self.previous.stages.vmic,
            &mut self.baseline.stages.vmic,
        );
        if snapshot.render.render_total_frames < self.previous.render.render_total_frames {
            self.baseline.render.render_total_frames = 0;
        }
        if snapshot.render.render_skipped_frames < self.previous.render.render_skipped_frames {
            self.baseline.render.render_skipped_frames = 0;
        }
        self.previous = snapshot.clone();
        snapshot.relative_to(&self.baseline)
    }
}
